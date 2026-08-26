use std::cell::RefCell;
use std::rc::Rc;

use shrimply_math_core::Fraction;
use shrimply_project::project::{Time, default_playback_speed};

pub type SharedPlayerState = Rc<RefCell<PlayerState>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub position: Time,
    pub duration: Time,
    pub playing: bool,
    pub playback_speed: Fraction,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProjectChange {
    pub duration: Option<Time>,
    pub audio: bool,
    pub audio_beats: bool,
    pub audio_waveforms: bool,
    pub video: bool,
    pub live_preview: bool,
    pub captions: bool,
    pub inspector: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerEvent {
    State,
    Project(ProjectChange),
}

pub struct PlayerState {
    position: Time,
    duration: Time,
    playing: bool,
    playback_speed: Fraction,
    revision: u64,
    listeners: Vec<ListenerEntry>,
}

type PlayerListener = Rc<dyn Fn(PlayerEvent)>;
type ListenerAlive = Rc<dyn Fn() -> bool>;

struct ListenerEntry {
    label: &'static str,
    listener: PlayerListener,
    alive: Option<ListenerAlive>,
}

pub fn new(duration: Time) -> SharedPlayerState {
    Rc::new(RefCell::new(PlayerState {
        position: Time::ZERO,
        duration,
        playing: false,
        playback_speed: default_playback_speed(),
        revision: 0,
        listeners: Vec::new(),
    }))
}

pub fn connect_named(
    state: &SharedPlayerState,
    label: &'static str,
    listener: impl Fn(PlayerEvent) + 'static,
) {
    state.borrow_mut().listeners.push(ListenerEntry {
        label,
        listener: Rc::new(listener),
        alive: None,
    });
}

pub fn connect_while_alive_named(
    state: &SharedPlayerState,
    label: &'static str,
    alive: impl Fn() -> bool + 'static,
    listener: impl Fn(PlayerEvent) + 'static,
) {
    // Use for UI owned by rebuildable trees. Plain connect leaks stale callbacks.
    state.borrow_mut().listeners.push(ListenerEntry {
        label,
        listener: Rc::new(listener),
        alive: Some(Rc::new(alive)),
    });
}

pub fn snapshot(state: &SharedPlayerState) -> Snapshot {
    let state = state.borrow();
    Snapshot {
        position: state.position,
        duration: state.duration,
        playing: state.playing,
        playback_speed: state.playback_speed,
        revision: state.revision,
    }
}

/// The authoritative playback time. UI code should read this instead of keeping a local copy
/// whose lifetime can diverge from the player clock.
pub fn current_time(state: &SharedPlayerState) -> Time {
    state.borrow().position
}

pub fn refresh_project(state: &SharedPlayerState, change: ProjectChange) {
    update(state, PlayerEvent::Project(change), |state| {
        if let Some(duration) = change.duration {
            state.duration = duration;
            state.position = state.position.clamp(Time::ZERO, duration);
        }
        state.revision = state.revision.wrapping_add(1);
        true
    });
}

pub fn set_position(state: &SharedPlayerState, position: Time) {
    update(state, PlayerEvent::State, |state| {
        let position = position.max(Time::ZERO);
        if state.position == position {
            return false;
        }
        if !state.playing {
            tracing::trace!(
                "Player position changed while paused: {} -> {}",
                state.position.as_label(),
                position.as_label()
            );
        }
        state.position = position;
        true
    });
}

pub fn set_duration(state: &SharedPlayerState, duration: Time) {
    update(state, PlayerEvent::State, |state| {
        if state.duration == duration {
            return false;
        }
        state.duration = duration;
        state.position = state.position.max(Time::ZERO);
        true
    });
}

pub fn set_playing(state: &SharedPlayerState, playing: bool) {
    update(state, PlayerEvent::State, |state| {
        if state.playing == playing {
            return false;
        }
        tracing::info!(
            "Player playing changed: {} -> {} at {}",
            state.playing,
            playing,
            state.position.as_label()
        );
        state.playback_speed = default_playback_speed();
        state.playing = playing;
        true
    });
}

pub fn set_playback_speed(state: &SharedPlayerState, playback_speed: Fraction) {
    update(state, PlayerEvent::State, |state| {
        if state.playback_speed == playback_speed {
            return false;
        }
        tracing::info!(
            "Player playback speed changed: {} -> {}",
            state.playback_speed,
            playback_speed
        );
        state.playback_speed = playback_speed;
        true
    });
}

pub fn step_playback_speed_forward(state: &SharedPlayerState) {
    let snapshot = snapshot(state);
    if !snapshot.playing {
        set_playback_speed(state, default_playback_speed());
        set_playing(state, true);
        return;
    }

    let next = match shrimply_project::project::fraction_numerator(snapshot.playback_speed) {
        value if value < 2 => 2,
        value if value < 4 => 4,
        _ => 8,
    };
    set_playback_speed(state, Fraction::new_raw(next, 1));
}

pub fn toggle_playing(state: &SharedPlayerState) {
    let snapshot = snapshot(state);
    if !snapshot.playing && snapshot.position >= snapshot.duration {
        set_position(state, Time::ZERO);
    }
    set_playing(state, !snapshot.playing);
}

fn update(
    state: &SharedPlayerState,
    event: PlayerEvent,
    change: impl FnOnce(&mut PlayerState) -> bool,
) {
    let listeners = {
        let mut state = state.borrow_mut();
        if !change(&mut state) {
            return;
        }
        state
            .listeners
            .retain(|entry| entry.alive.as_ref().is_none_or(|alive| alive()));
        state
            .listeners
            .iter()
            .map(|entry| (entry.label, entry.listener.clone(), entry.alive.clone()))
            .collect::<Vec<_>>()
    };

    let listener_count = listeners.len();
    let _span = tracing::debug_span!(
        "player.dispatch",
        event = ?event,
        listener_count,
    )
    .entered();
    let _measurement = shrimply_benchmarking::measure("Player / Dispatch");
    for (index, (label, listener, alive)) in listeners.into_iter().enumerate() {
        if alive.as_ref().is_some_and(|alive| !alive()) {
            continue;
        }
        let _listener_span = tracing::debug_span!("player.listener", label, index).entered();
        shrimply_support::crash::set_context(format!(
            "player listener begin {label} event={event:?}"
        ));
        listener(event);
        shrimply_support::crash::set_context(format!(
            "player listener end {label} event={event:?}"
        ));
    }
}
