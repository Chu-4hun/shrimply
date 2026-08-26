use super::InspectorContext;
use crate::player_state::{self, ProjectChange};
use gtk::prelude::*;
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect, mirror::MirrorModifier};
use uuid::Uuid;

pub fn add_rows(value: &MirrorModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let horizontal_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let horizontal_label = gtk::Label::new(Some("Horizontal"));
    horizontal_label.set_halign(gtk::Align::Start);
    horizontal_label.set_hexpand(true);
    let horizontal = gtk::Switch::builder()
        .active(value.horizontal)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    horizontal_row.append(&horizontal_label);
    horizontal_row.append(&horizontal);
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    horizontal.connect_active_notify(move |toggle| {
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
        effect.horizontal = toggle.is_active();
        shrimply_project::project::commit_edit(&project, "edit-mirror");
        drop(project);
        player_state::refresh_project(
            &player,
            ProjectChange {
                video: true,
                ..Default::default()
            },
        );
    });
    out.append(&horizontal_row);

    let vertical_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let vertical_label = gtk::Label::new(Some("Vertical"));
    vertical_label.set_halign(gtk::Align::Start);
    vertical_label.set_hexpand(true);
    let vertical = gtk::Switch::builder()
        .active(value.vertical)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    vertical_row.append(&vertical_label);
    vertical_row.append(&vertical);
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    vertical.connect_active_notify(move |toggle| {
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
        effect.vertical = toggle.is_active();
        shrimply_project::project::commit_edit(&project, "edit-mirror");
        drop(project);
        player_state::refresh_project(
            &player,
            ProjectChange {
                video: true,
                ..Default::default()
            },
        );
    });
    out.append(&vertical_row);
}
