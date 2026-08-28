use super::InspectorContext;
use crate::player_state::{self, ProjectChange};
use gtk::prelude::*;
use shrimply_ui_foundation::ui::switch_row;
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect, mirror::MirrorModifier};
use uuid::Uuid;

pub fn add_rows(value: &MirrorModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    out.append(&switch_row(
        "Horizontal",
        None,
        value.horizontal,
        move |active| {
        let Some(key) = key.clone() else { return };
        let mut project = project.borrow_mut();
        let Some(effect) = project
            .video_item_mut(&key)
            .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
            .and_then(|modifier| match &mut modifier.effect {
                ModifierEffect::Raster(effect) => match &mut **effect {
                    RasterModifierEffect::Mirror(effect) => Some(effect),
                    _ => None,
                },
                _ => None,
            })
        else {
            return;
        };
        effect.horizontal = active;
        shrimply_project::project::commit_edit(&project, "edit-mirror");
        drop(project);
        player_state::refresh_project(
            &player,
            ProjectChange {
                video: true,
                ..Default::default()
            },
        );
        },
    ));

    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    out.append(&switch_row("Vertical", None, value.vertical, move |active| {
        let Some(key) = key.clone() else { return };
        let mut project = project.borrow_mut();
        let Some(effect) = project
            .video_item_mut(&key)
            .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
            .and_then(|modifier| match &mut modifier.effect {
                ModifierEffect::Raster(effect) => match &mut **effect {
                    RasterModifierEffect::Mirror(effect) => Some(effect),
                    _ => None,
                },
                _ => None,
            })
        else {
            return;
        };
        effect.vertical = active;
        shrimply_project::project::commit_edit(&project, "edit-mirror");
        drop(project);
        player_state::refresh_project(
            &player,
            ProjectChange {
                video: true,
                ..Default::default()
            },
        );
    }));
}
