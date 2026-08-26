use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk::glib;
use shrimply_resource_pipeline::{Event, Subscription, TryNext};

const EVENTS_PER_TICK: usize = 8;

pub struct UiSubscription {
    source: Option<glib::SourceId>,
    active: Rc<Cell<bool>>,
}

impl UiSubscription {
    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    pub fn cancel(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        if self.active.replace(false)
            && let Some(source) = self.source.take()
        {
            source.remove();
        }
    }
}

impl Drop for UiSubscription {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn deliver<O, K, P, V>(
    owner: glib::WeakRef<O>,
    mut subscription: Subscription<K, P, V>,
    interval: Duration,
    mut on_event: impl FnMut(&O, Event<P, V>) + 'static,
) -> UiSubscription
where
    O: glib::object::ObjectType + 'static,
    K: 'static,
    P: 'static,
    V: 'static,
{
    let active = Rc::new(Cell::new(true));
    let source_active = active.clone();
    let source = glib::timeout_add_local(interval, move || {
        if !source_active.get() {
            return glib::ControlFlow::Break;
        }
        let Some(owner) = owner.upgrade() else {
            source_active.set(false);
            return glib::ControlFlow::Break;
        };
        for _ in 0..EVENTS_PER_TICK {
            match subscription.try_next() {
                TryNext::Event(event) => {
                    let terminal = event.is_terminal();
                    on_event(&owner, event);
                    if terminal {
                        source_active.set(false);
                        return glib::ControlFlow::Break;
                    }
                }
                TryNext::Empty => return glib::ControlFlow::Continue,
                TryNext::Closed => {
                    on_event(
                        &owner,
                        Event::Failed(Arc::from("resource job closed without a terminal event")),
                    );
                    source_active.set(false);
                    return glib::ControlFlow::Break;
                }
            }
        }
        glib::ControlFlow::Continue
    });
    UiSubscription {
        source: Some(source),
        active,
    }
}
