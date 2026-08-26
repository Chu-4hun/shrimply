use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, color_row, integer_scalar_row, scalar_row};
use crate::player_state::{self, ProjectChange};
use crate::timeline_value::*;
use shrimply_project::project::Color;
use shrimply_video_modifiers::{
    ModifierEffect, RasterModifierEffect,
    dithering::{DitheringColorMode, DitheringModifier},
};

pub fn add_rows(value: &DitheringModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&super::step_row(
        "Pattern",
        &value.pattern,
        id,
        context,
        "edit-dithering-pattern",
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &**effect {
                RasterModifierEffect::Dithering(effect) => Some(&effect.pattern),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::Dithering(effect) => Some(&mut effect.pattern),
                _ => None,
            },
            _ => None,
        },
    ));

    out.append(&super::step_row(
        "Color mode",
        &value.color_mode,
        id,
        context,
        "edit-dithering-color-mode",
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &**effect {
                RasterModifierEffect::Dithering(effect) => Some(&effect.color_mode),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::Dithering(effect) => Some(&mut effect.color_mode),
                _ => None,
            },
            _ => None,
        },
    ));

    out.append(&integer_scalar_row(
        "Levels",
        &value.levels,
        id,
        ScalarOptions {
            minimum: Some(2.0),
            maximum: Some(256.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
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
    let position = player_state::snapshot(&context.player_state).position;
    let local_time = context
        .selected_item
        .clone()
        .and_then(|key| crate::video::visual_local_time(&context.project.borrow(), key, position))
        .unwrap_or(shrimply_project::project::Time::ZERO);
    if value.color_mode.value_at(local_time) == DitheringColorMode::Palette {
        for color in &value.palette {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let color_control = color_row("Color", color, id, context);
            color_control.set_hexpand(true);
            row.append(&color_control);
            let remove = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text("Remove color")
                .css_classes(["flat"])
                .build();
            let key = context.selected_item.clone();
            let project = context.project.clone();
            let player = context.player_state.clone();
            let refresh = context.refresh.clone();
            let color_id = color.id;
            remove.connect_clicked(move |_| {
                let Some(key) = key.clone() else { return };
                let mut project = project.borrow_mut();
                let Some(effect) = project
                    .video_item_mut(&key)
                    .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
                    .and_then(|modifier| match &mut modifier.effect {
                        ModifierEffect::Raster(effect) => match &mut **effect {
                            RasterModifierEffect::Dithering(effect) => Some(effect),
                            _ => None,
                        },
                        _ => None,
                    })
                else {
                    return;
                };
                let Some(index) = effect.palette.iter().position(|color| color.id == color_id)
                else {
                    return;
                };
                effect.palette.remove(index);
                shrimply_project::project::commit_edit(&project, "remove-dithering-palette-color");
                drop(project);
                player_state::refresh_project(
                    &player,
                    ProjectChange {
                        video: true,
                        inspector: true,
                        ..Default::default()
                    },
                );
                let refresh = refresh.clone();
                gtk::glib::idle_add_local_once(move || refresh());
            });
            row.append(&remove);
            out.append(&row);
        }

        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .label("Add color")
            .halign(gtk::Align::Start)
            .css_classes(["flat"])
            .build();
        let key = context.selected_item.clone();
        let project = context.project.clone();
        let player = context.player_state.clone();
        let refresh = context.refresh.clone();
        add.connect_clicked(move |_| {
            let Some(key) = key.clone() else { return };
            let mut project = project.borrow_mut();
            let Some(effect) = project
                .video_item_mut(&key)
                .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
                .and_then(|modifier| match &mut modifier.effect {
                    ModifierEffect::Raster(effect) => match &mut **effect {
                        RasterModifierEffect::Dithering(effect) => Some(effect),
                        _ => None,
                    },
                    _ => None,
                })
            else {
                return;
            };
            effect
                .palette
                .push(TimelineValue::<Color<u8>>::new_const(Color::<u8>::WHITE));
            shrimply_project::project::commit_edit(&project, "add-dithering-palette-color");
            drop(project);
            player_state::refresh_project(
                &player,
                ProjectChange {
                    video: true,
                    inspector: true,
                    ..Default::default()
                },
            );
            let refresh = refresh.clone();
            gtk::glib::idle_add_local_once(move || refresh());
        });
        out.append(&add);
    }
}
