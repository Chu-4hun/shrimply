use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::gdk::prelude::*;
use gtk::glib;
use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_pointer, wl_registry, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::pointer_constraints::zv1::client::{
    zwp_locked_pointer_v1, zwp_pointer_constraints_v1,
};
use wayland_protocols::wp::relative_pointer::zv1::client::{
    zwp_relative_pointer_manager_v1, zwp_relative_pointer_v1,
};

const SLOW_POINTER_LOCK_LOG_THRESHOLD: Duration = Duration::from_millis(16);

unsafe extern "C" {
    fn gdk_wayland_display_get_wl_display(display: *mut gdk::ffi::GdkDisplay) -> *mut c_void;
    fn gdk_wayland_surface_get_wl_surface(surface: *mut gdk::ffi::GdkSurface) -> *mut c_void;
    fn gdk_wayland_seat_get_wl_seat(seat: *mut gdk::ffi::GdkSeat) -> *mut c_void;
}

pub struct PointerLock {
    _inner: Rc<RefCell<PointerLockInner>>,
}

impl PointerLock {
    pub fn new(widget: &impl IsA<gtk::Widget>, on_delta: impl Fn(f64) + 'static) -> Option<Self> {
        Self::new_2d(widget, move |delta_x, _| on_delta(delta_x))
    }

    pub fn new_2d(
        widget: &impl IsA<gtk::Widget>,
        on_delta: impl Fn(f64, f64) + 'static,
    ) -> Option<Self> {
        std::env::var_os("WAYLAND_DISPLAY")?;

        let native = widget.as_ref().native()?;
        let gdk_surface = native.surface()?;
        let gdk_display = gdk_surface.display();
        let gdk_seat = gdk_display.default_seat()?;

        let wl_display =
            unsafe { gdk_wayland_display_get_wl_display(gdk_display.to_glib_none().0) };
        let wl_surface =
            unsafe { gdk_wayland_surface_get_wl_surface(gdk_surface.to_glib_none().0) };
        let wl_seat = unsafe { gdk_wayland_seat_get_wl_seat(gdk_seat.to_glib_none().0) };
        if wl_display.is_null() || wl_surface.is_null() || wl_seat.is_null() {
            return None;
        }

        let backend = unsafe { Backend::from_foreign_display(wl_display.cast()) };
        let conn = Connection::from_backend(backend);
        let (globals, mut event_queue) = registry_queue_init::<PointerLockState>(&conn).ok()?;
        let qh = event_queue.handle();
        let pointer_constraints: zwp_pointer_constraints_v1::ZwpPointerConstraintsV1 =
            globals.bind(&qh, 1..=1, ()).ok()?;
        let relative_manager: zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1 =
            globals.bind(&qh, 1..=1, ()).ok()?;

        let surface_id =
            unsafe { ObjectId::from_ptr(wl_surface::WlSurface::interface(), wl_surface.cast()) }
                .ok()?;
        let seat_id =
            unsafe { ObjectId::from_ptr(wl_seat::WlSeat::interface(), wl_seat.cast()) }.ok()?;
        let surface = wl_surface::WlSurface::from_id(&conn, surface_id).ok()?;
        let seat = wl_seat::WlSeat::from_id(&conn, seat_id).ok()?;
        let pointer = seat.get_pointer(&qh, ());
        let relative_pointer = relative_manager.get_relative_pointer(&pointer, &qh, ());
        let locked_pointer = pointer_constraints.lock_pointer(
            &surface,
            &pointer,
            None,
            zwp_pointer_constraints_v1::Lifetime::Persistent,
            &qh,
            (),
        );

        let mut state = PointerLockState {
            on_delta: Rc::new(on_delta),
            tick_motion_count: 0,
            tick_delta_x: 0.0,
            tick_delta_y: 0.0,
            tick_on_delta_elapsed_us: 0,
        };
        let _ = event_queue.flush();
        let _ = event_queue.dispatch_pending(&mut state);
        state.dispatch_delta();

        let inner = Rc::new(RefCell::new(PointerLockInner {
            event_queue,
            state,
            source: None,
            locked_pointer,
            relative_pointer,
            pointer,
            pointer_constraints,
            relative_manager,
            surface,
            _seat: seat,
            _gdk_display: gdk_display,
            _gdk_surface: gdk_surface,
            _gdk_seat: gdk_seat,
        }));
        let weak = Rc::downgrade(&inner);
        let source = widget.as_ref().add_tick_callback(move |_, _| {
            let Some(inner) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let mut inner = inner.borrow_mut();
            let PointerLockInner {
                event_queue, state, ..
            } = &mut *inner;
            state.begin_tick();
            let dispatch_started = Instant::now();
            let _ = event_queue.dispatch_pending(state);
            let dispatch_elapsed = dispatch_started.elapsed();
            // One dispatch can contain many raw motion events; apply one UI update.
            state.dispatch_delta();
            let flush_started = Instant::now();
            let _ = event_queue.flush();
            let flush_elapsed = flush_started.elapsed();
            state.log_tick(dispatch_elapsed, flush_elapsed);
            glib::ControlFlow::Continue
        });
        inner.borrow_mut().source = Some(source);

        Some(Self { _inner: inner })
    }

    pub fn restore_cursor_at(&self, x: f64, y: f64) {
        let inner = self._inner.borrow();
        inner.locked_pointer.set_cursor_position_hint(x, y);
        inner.surface.commit();
        let _ = inner.event_queue.flush();
    }
}

struct PointerLockInner {
    event_queue: EventQueue<PointerLockState>,
    state: PointerLockState,
    source: Option<gtk::TickCallbackId>,
    locked_pointer: zwp_locked_pointer_v1::ZwpLockedPointerV1,
    relative_pointer: zwp_relative_pointer_v1::ZwpRelativePointerV1,
    pointer: wl_pointer::WlPointer,
    pointer_constraints: zwp_pointer_constraints_v1::ZwpPointerConstraintsV1,
    relative_manager: zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
    surface: wl_surface::WlSurface,
    _seat: wl_seat::WlSeat,
    _gdk_display: gdk::Display,
    _gdk_surface: gdk::Surface,
    _gdk_seat: gdk::Seat,
}

impl Drop for PointerLockInner {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
        self.locked_pointer.destroy();
        self.relative_pointer.destroy();
        self.pointer.release();
        self.pointer_constraints.destroy();
        self.relative_manager.destroy();
        let _ = self.event_queue.flush();
    }
}

struct PointerLockState {
    on_delta: Rc<dyn Fn(f64, f64)>,
    tick_motion_count: usize,
    tick_delta_x: f64,
    tick_delta_y: f64,
    tick_on_delta_elapsed_us: u128,
}

impl PointerLockState {
    fn begin_tick(&mut self) {
        self.tick_motion_count = 0;
        self.tick_delta_x = 0.0;
        self.tick_delta_y = 0.0;
        self.tick_on_delta_elapsed_us = 0;
    }

    fn log_tick(&self, dispatch_elapsed: Duration, flush_elapsed: Duration) {
        if dispatch_elapsed < SLOW_POINTER_LOCK_LOG_THRESHOLD
            && flush_elapsed < SLOW_POINTER_LOCK_LOG_THRESHOLD
            && self.tick_on_delta_elapsed_us < SLOW_POINTER_LOCK_LOG_THRESHOLD.as_micros()
        {
            return;
        }
        tracing::debug!(
            "pointer_lock: dispatch_tick motions={} delta=({:.3}, {:.3}) dispatch_elapsed_us={} on_delta_elapsed_us={} flush_elapsed_us={}",
            self.tick_motion_count,
            self.tick_delta_x,
            self.tick_delta_y,
            dispatch_elapsed.as_micros(),
            self.tick_on_delta_elapsed_us,
            flush_elapsed.as_micros(),
        );
    }

    fn dispatch_delta(&mut self) {
        if self.tick_delta_x == 0.0 && self.tick_delta_y == 0.0 {
            return;
        }
        let delta_x = self.tick_delta_x;
        let delta_y = self.tick_delta_y;
        let started = Instant::now();
        (self.on_delta)(delta_x, delta_y);
        let elapsed = started.elapsed();
        self.tick_on_delta_elapsed_us += elapsed.as_micros();
        if elapsed >= SLOW_POINTER_LOCK_LOG_THRESHOLD {
            tracing::debug!(
                "pointer_lock: coalesced_delta motions={} delta=({delta_x:.3}, {delta_y:.3}) on_delta_elapsed_us={}",
                self.tick_motion_count,
                elapsed.as_micros(),
            );
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for PointerLockState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for PointerLockState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_pointer::WlPointer,
        _event: wl_pointer::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_pointer_constraints_v1::ZwpPointerConstraintsV1, ()> for PointerLockState {
    fn event(
        _state: &mut Self,
        _proxy: &zwp_pointer_constraints_v1::ZwpPointerConstraintsV1,
        _event: zwp_pointer_constraints_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_locked_pointer_v1::ZwpLockedPointerV1, ()> for PointerLockState {
    fn event(
        _state: &mut Self,
        _proxy: &zwp_locked_pointer_v1::ZwpLockedPointerV1,
        _event: zwp_locked_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1, ()>
    for PointerLockState
{
    fn event(
        _state: &mut Self,
        _proxy: &zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
        _event: zwp_relative_pointer_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_relative_pointer_v1::ZwpRelativePointerV1, ()> for PointerLockState {
    fn event(
        state: &mut Self,
        _proxy: &zwp_relative_pointer_v1::ZwpRelativePointerV1,
        event: zwp_relative_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let zwp_relative_pointer_v1::Event::RelativeMotion { dx, dy, .. } = event {
            state.tick_motion_count += 1;
            state.tick_delta_x += dx;
            state.tick_delta_y += dy;
        }
    }
}
