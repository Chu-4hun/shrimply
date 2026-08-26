use gtk::prelude::*;
use shrimply_project::project::Project;
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect, mask::MaskModifier};
use uuid::Uuid;

use crate::{
    InspectorContext,
    player_state::{self, ProjectChange},
    ui::control_row,
};

pub fn add_rows(value: &MaskModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let source = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let pick = gtk::Button::builder()
        .label(if value.item_id.is_some() {
            mask_item_label(
                &context.project.borrow(),
                context.selected_item.as_ref(),
                value.item_id,
            )
        } else {
            "Drag onto a visual clip…".to_string()
        })
        .tooltip_text("Drag onto a visual clip in the timeline")
        .hexpand(true)
        .build();
    let drag = gtk::DragSource::builder()
        .actions(gtk::gdk::DragAction::COPY)
        .build();
    drag.connect_prepare(move |_, _, _| {
        Some(gtk::gdk::ContentProvider::for_value(
            &gtk::glib::Bytes::from_owned(id.to_string().into_bytes()).to_value(),
        ))
    });
    pick.add_controller(drag);
    source.append(&pick);
    let clear = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text("Clear mask source")
        .sensitive(value.item_id.is_some())
        .build();
    clear.connect_clicked({
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        move |_| {
            let Some(key) = &key else {
                return;
            };
            update_mask(&project, key, id, |mask| mask.item_id = None);
            player_state::refresh_project(
                &player,
                ProjectChange {
                    video: true,
                    inspector: true,
                    ..Default::default()
                },
            );
        }
    });
    source.append(&clear);
    out.append(&control_row("Source", &source));

    out.append(&super::step_row(
        "Mode",
        &value.mode,
        id,
        context,
        "edit-mask-mode",
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &**effect {
                RasterModifierEffect::Mask(effect) => Some(&effect.mode),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::Mask(effect) => Some(&mut effect.mode),
                _ => None,
            },
            _ => None,
        },
    ));

    let invert = gtk::Switch::builder()
        .active(value.invert)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    invert.connect_active_notify({
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        move |invert| {
            let Some(key) = &key else {
                return;
            };
            update_mask(&project, key, id, |mask| mask.invert = invert.is_active());
            player_state::refresh_project(
                &player,
                ProjectChange {
                    video: true,
                    ..Default::default()
                },
            );
        }
    });
    out.append(&control_row("Invert", &invert));
}

fn update_mask(
    project: &std::rc::Rc<std::cell::RefCell<Project>>,
    key: &shrimply_project::project::ItemAddress,
    id: Uuid,
    update: impl FnOnce(&mut MaskModifier),
) {
    let mut project = project.borrow_mut();
    let Some(item) = project.video_item_mut(key) else {
        return;
    };
    let Some(mask) = item
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::Mask(mask) => Some(mask),
                _ => None,
            },
            _ => None,
        })
    else {
        return;
    };
    update(mask);
    shrimply_project::project::commit_edit(&project, "edit-mask");
}

fn mask_item_label(
    project: &Project,
    selected_item: Option<&shrimply_project::project::ItemAddress>,
    id: Option<Uuid>,
) -> String {
    let Some(id) = id else {
        return "Choose item…".to_string();
    };
    let Some(sequence_path) = selected_item.map(|item| item.sequence_path()) else {
        return "Missing item".to_string();
    };
    let Some(tracks) = project.video_tracks_for_path(sequence_path) else {
        return "Missing item".to_string();
    };
    tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track
                .items
                .iter()
                .position(|item| item.id == id)
                .map(|item_index| format!("Track {} · Item {}", track_index + 1, item_index + 1))
        })
        .unwrap_or_else(|| "Missing item".to_string())
}
