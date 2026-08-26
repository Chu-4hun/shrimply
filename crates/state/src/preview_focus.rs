use std::cell::RefCell;
use std::rc::Rc;

pub use shrimply_preview_core::{PreviewFacetKey, PreviewTarget};
use shrimply_project::project::ItemAddress;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusedPreview {
    pub item: ItemAddress,
    pub card_key: String,
    pub target: PreviewTarget,
}

pub type SharedPreviewFocus = Rc<RefCell<PreviewFocusState>>;

pub struct PreviewFocusState {
    focused: Option<FocusedPreview>,
    listeners: Vec<ListenerEntry>,
}

type Listener = Rc<dyn Fn()>;
type ListenerAlive = Rc<dyn Fn() -> bool>;

struct ListenerEntry {
    label: &'static str,
    listener: Listener,
    alive: Option<ListenerAlive>,
}

pub fn new() -> SharedPreviewFocus {
    Rc::new(RefCell::new(PreviewFocusState {
        focused: None,
        listeners: Vec::new(),
    }))
}

pub fn snapshot(state: &SharedPreviewFocus) -> Option<FocusedPreview> {
    state.borrow().focused.clone()
}

pub fn set(state: &SharedPreviewFocus, focused: FocusedPreview) {
    update(state, Some(focused));
}

pub fn clear(state: &SharedPreviewFocus) {
    update(state, None);
}

pub fn connect_named(
    state: &SharedPreviewFocus,
    label: &'static str,
    listener: impl Fn() + 'static,
) {
    state.borrow_mut().listeners.push(ListenerEntry {
        label,
        listener: Rc::new(listener),
        alive: None,
    });
}

pub fn connect_while_alive_named(
    state: &SharedPreviewFocus,
    label: &'static str,
    alive: impl Fn() -> bool + 'static,
    listener: impl Fn() + 'static,
) {
    state.borrow_mut().listeners.push(ListenerEntry {
        label,
        listener: Rc::new(listener),
        alive: Some(Rc::new(alive)),
    });
}

fn update(state: &SharedPreviewFocus, focused: Option<FocusedPreview>) {
    let listeners = {
        let mut state = state.borrow_mut();
        if state.focused == focused {
            return;
        }
        state.focused = focused;
        state
            .listeners
            .retain(|entry| entry.alive.as_ref().is_none_or(|alive| alive()));
        state
            .listeners
            .iter()
            .map(|entry| (entry.label, entry.listener.clone(), entry.alive.clone()))
            .collect::<Vec<_>>()
    };

    for (label, listener, alive) in listeners {
        if alive.as_ref().is_some_and(|alive| !alive()) {
            continue;
        }
        shrimply_support::crash::set_context(format!("preview focus listener begin {label}"));
        listener();
        shrimply_support::crash::set_context(format!("preview focus listener end {label}"));
    }
}
