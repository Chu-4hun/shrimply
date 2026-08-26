use gtk::prelude::*;
use shrimply_project::project::Project;
use shrimply_ui_foundation::ui::{control_row, enum_dropdown};
use shrimply_video_modifiers::{ModifierEffect, VectorModifierEffect, text_mask::TextMaskModifier};
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, scalar_row};
use crate::player_state::{self, ProjectChange};

pub fn add_rows(value: &TextMaskModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&scalar_row(
        "Amount",
        &value.amount,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(1.0),
            unit: None,
            rotating: false,
        },
        context,
    ));

    let mode = enum_dropdown(value.partial_mode, {
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        move |mode| {
            update(&project, key.as_ref(), id, |mask| mask.partial_mode = mode);
            refresh(&player);
        }
    });
    out.append(&control_row("Partial mode", &mode));

    let direction = enum_dropdown(value.direction, {
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        move |direction| {
            update(&project, key.as_ref(), id, |mask| {
                mask.direction = direction
            });
            refresh(&player);
        }
    });
    out.append(&control_row("Direction", &direction));
}

fn update(
    project: &std::rc::Rc<std::cell::RefCell<Project>>,
    key: Option<&shrimply_project::project::ItemAddress>,
    id: Uuid,
    update: impl FnOnce(&mut TextMaskModifier),
) {
    let Some(key) = key else { return };
    let mut project = project.borrow_mut();
    let Some(mask) = project
        .video_item_mut(key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Vector(effect) => match &mut **effect {
                VectorModifierEffect::TextMask(mask) => Some(mask),
                _ => None,
            },
            _ => None,
        })
    else {
        return;
    };
    update(mask);
    shrimply_project::project::commit_edit(&project, "edit-text-mask");
}

fn refresh(player: &crate::player_state::SharedPlayerState) {
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            ..Default::default()
        },
    );
}
