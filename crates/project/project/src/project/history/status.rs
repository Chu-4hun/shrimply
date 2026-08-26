use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum CommitStatus {
    InProgress(String),
    SavePending,
    Saving,
    SaveFailed(String),
    Idle,
}

#[derive(Clone)]
pub(super) enum SaveStatus {
    Saved,
    Pending,
    Saving,
    Failed(String),
}

type Listener = Rc<dyn Fn(CommitStatus)>;

struct Manager {
    listeners: Vec<Listener>,
    actions: Vec<String>,
    save_status: SaveStatus,
}

thread_local! {
    static MANAGER: RefCell<Manager> = const { RefCell::new(Manager {
        listeners: Vec::new(),
        actions: Vec::new(),
        save_status: SaveStatus::Saved,
    }) };
}

pub fn connect_commit_status(listener: impl Fn(CommitStatus) + 'static) {
    let listener = Rc::new(listener);
    let status = MANAGER.with(|manager| {
        let mut manager = manager.borrow_mut();
        manager.listeners.push(listener.clone());
        manager.current()
    });
    listener(status);
}

pub(super) fn begin(message: &str) {
    update(|manager| manager.actions.push(message.to_string()));
}

pub(super) fn finish() {
    update(|manager| {
        manager.actions.pop();
    });
}

pub(super) fn set_save(status: SaveStatus) {
    update(|manager| manager.save_status = status);
}

pub(super) fn request_save(status: SaveStatus) {
    glib::idle_add_once(move || set_save(status));
}

fn update(change: impl FnOnce(&mut Manager)) {
    let (status, listeners) = MANAGER.with(|manager| {
        let mut manager = manager.borrow_mut();
        change(&mut manager);
        (manager.current(), manager.listeners.clone())
    });
    for listener in listeners {
        listener(status.clone());
    }
}

impl Manager {
    fn current(&self) -> CommitStatus {
        if matches!(self.save_status, SaveStatus::Saving) {
            return CommitStatus::Saving;
        }
        if let Some(message) = self.actions.last() {
            return CommitStatus::InProgress(message.clone());
        }
        match &self.save_status {
            SaveStatus::Saved => CommitStatus::Idle,
            SaveStatus::Pending => CommitStatus::SavePending,
            SaveStatus::Saving => CommitStatus::Saving,
            SaveStatus::Failed(error) => CommitStatus::SaveFailed(error.clone()),
        }
    }
}
