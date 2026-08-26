use crate::player_state::{self, ProjectChange};
use shrimply_project::project::VideoItem;
pub(super) fn update_generated_live(
    project: &std::rc::Rc<std::cell::RefCell<shrimply_project::project::Project>>,
    player_state: &player_state::SharedPlayerState,
    key: crate::InspectedItem,
    update: impl FnOnce(&mut VideoItem) -> bool,
) {
    let mut project = project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    if !update(item) {
        return;
    }
    drop(project);
    player_state::refresh_project(
        player_state,
        ProjectChange {
            video: true,
            ..ProjectChange::default()
        },
    );
}

pub(super) fn resize_text_source(item: &mut VideoItem) {
    let _ = item;
}
