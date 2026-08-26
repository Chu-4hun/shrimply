use gtk::prelude::*;
use shrimply_ui_foundation::tr;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, scalar_row};
use crate::player_state::{self, ProjectChange};
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect, luma_key::LumaKeyModifier};

pub fn add_rows(value: &LumaKeyModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let options = ScalarOptions {
        minimum: Some(0.0),
        maximum: Some(1.0),
        unit: None,
        rotating: false,
    };
    out.append(&scalar_row(
        "Threshold",
        &value.threshold,
        id,
        options,
        context,
    ));
    out.append(&scalar_row(
        "Softness",
        &value.softness,
        id,
        options,
        context,
    ));

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let label = gtk::Label::new(Some(tr!("Invert").as_ref()));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    let invert = gtk::Switch::builder()
        .active(value.invert)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    row.append(&label);
    row.append(&invert);

    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    invert.connect_active_notify(move |toggle| {
        let Some(key) = key.clone() else { return };
        let mut project = project.borrow_mut();
        let Some(effect) = project
            .video_item_mut(&key)
            .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
            .and_then(|modifier| match &mut modifier.effect {
                ModifierEffect::Raster(effect) => match &mut **effect {
                    RasterModifierEffect::LumaKey(effect) => Some(effect),
                    _ => None,
                },
                _ => None,
            })
        else {
            return;
        };
        if effect.invert == toggle.is_active() {
            return;
        }
        effect.invert = toggle.is_active();
        shrimply_project::project::commit_edit(&project, "edit-luma-key-invert");
        drop(project);
        player_state::refresh_project(
            &player,
            ProjectChange {
                video: true,
                ..Default::default()
            },
        );
    });
    out.append(&row);
}
