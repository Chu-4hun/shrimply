use std::cell::RefCell;

use crate::project::Project;

const HISTORY_LIMIT: usize = 1000;

thread_local! {
    static HISTORY: RefCell<Option<MemoryHistory>> = const { RefCell::new(None) };
}

struct MemoryHistory {
    snapshots: Vec<Project>,
    base_index: usize,
    index: usize,
    last_coalesce_group: Option<String>,
}

pub(super) fn seed(project: &Project, base_index: usize) {
    HISTORY.with(|slot| {
        *slot.borrow_mut() = Some(MemoryHistory {
            snapshots: vec![project.clone()],
            base_index,
            index: 0,
            last_coalesce_group: None,
        });
    });
}

pub(super) fn commit(project: &Project, coalesce_group: Option<&str>) {
    HISTORY.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(history) = slot.as_mut() else {
            *slot = Some(MemoryHistory {
                snapshots: vec![project.clone()],
                base_index: 0,
                index: 0,
                last_coalesce_group: None,
            });
            return;
        };
        history.commit(project.clone(), coalesce_group);
    });
}

pub(super) fn undo(project: &mut Project) -> Option<usize> {
    HISTORY.with(|slot| slot.borrow_mut().as_mut()?.undo(project))
}

pub(super) fn redo(project: &mut Project) -> Option<usize> {
    HISTORY.with(|slot| slot.borrow_mut().as_mut()?.redo(project))
}

impl MemoryHistory {
    fn stored_index(&self) -> usize {
        self.base_index + self.index
    }

    fn commit(&mut self, project: Project, coalesce_group: Option<&str>) {
        if let Some(group) = coalesce_group {
            if self.last_coalesce_group.as_deref() == Some(group)
                && self.index + 1 == self.snapshots.len()
                && !self.snapshots.is_empty()
            {
                self.snapshots[self.index] = project;
                return;
            }
            self.snapshots.truncate(self.index + 1);
            self.snapshots.push(project);
            self.index = self.snapshots.len() - 1;
            self.last_coalesce_group = Some(group.to_string());
            self.prune();
            return;
        }

        self.snapshots.truncate(self.index + 1);
        self.snapshots.push(project);
        self.index = self.snapshots.len() - 1;
        self.last_coalesce_group = None;
        self.prune();
    }

    fn undo(&mut self, project: &mut Project) -> Option<usize> {
        if self.index == 0 {
            return None;
        }
        self.snapshots[self.index] = project.clone();
        self.index -= 1;
        self.last_coalesce_group = None;
        let cursor_position = project.cursor_position;
        let timeline_zoom = project.timeline_zoom;
        let expanded_sequence_paths = project.expanded_sequence_paths.clone();
        *project = self.snapshots[self.index].clone();
        project.cursor_position = cursor_position;
        project.timeline_zoom = timeline_zoom;
        project.expanded_sequence_paths = expanded_sequence_paths;
        Some(self.stored_index())
    }

    fn redo(&mut self, project: &mut Project) -> Option<usize> {
        if self.index + 1 >= self.snapshots.len() {
            return None;
        }
        self.snapshots[self.index] = project.clone();
        self.index += 1;
        self.last_coalesce_group = None;
        let cursor_position = project.cursor_position;
        let timeline_zoom = project.timeline_zoom;
        let expanded_sequence_paths = project.expanded_sequence_paths.clone();
        *project = self.snapshots[self.index].clone();
        project.cursor_position = cursor_position;
        project.timeline_zoom = timeline_zoom;
        project.expanded_sequence_paths = expanded_sequence_paths;
        Some(self.stored_index())
    }

    fn prune(&mut self) {
        let remove_count = self.snapshots.len().saturating_sub(HISTORY_LIMIT);
        if remove_count == 0 {
            return;
        }
        self.snapshots.drain(0..remove_count);
        self.base_index += remove_count;
        self.index = self.index.saturating_sub(remove_count);
    }
}
