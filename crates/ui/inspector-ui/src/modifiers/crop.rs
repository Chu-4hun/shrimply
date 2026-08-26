use super::{InspectorContext, ScalarOptions, scalar_row};
use crate::player_state::{self, ProjectChange};
use gtk::prelude::*;
use shrimply_ui_foundation::ui::selector;
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect, crop::CropModifier};
use uuid::Uuid;

pub fn add_rows(value: &CropModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let (percentage, edges) = match value {
        CropModifier::Percentage(edges) => (true, edges),
        CropModifier::Pixels(edges) => (false, edges),
    };
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    out.append(&selector(
        "Mode",
        percentage,
        [(true, "Percentage"), (false, "Pixels")],
        move |percentage| {
            let Some(key) = &key else { return };
            let mut project = project.borrow_mut();
            let Some(crop) = project
                .video_item_mut(key)
                .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
                .and_then(|modifier| match &mut modifier.effect {
                    ModifierEffect::Raster(effect) => match &mut **effect {
                        RasterModifierEffect::Crop(crop) => Some(crop),
                        _ => None,
                    },
                    _ => None,
                })
            else {
                return;
            };
            if matches!(crop, CropModifier::Percentage(_)) == percentage {
                return;
            }
            let edges = match crop {
                CropModifier::Percentage(edges) | CropModifier::Pixels(edges) => edges.clone(),
            };
            *crop = if percentage {
                CropModifier::Percentage(edges)
            } else {
                CropModifier::Pixels(edges)
            };
            shrimply_project::project::commit_edit(&project, "edit-crop-mode");
            drop(project);
            player_state::refresh_project(
                &player,
                ProjectChange {
                    video: true,
                    inspector: true,
                    ..Default::default()
                },
            );
        },
    ));

    let s = ScalarOptions {
        minimum: Some(0.0),
        maximum: percentage.then_some(100.0),
        unit: Some(if percentage { "%" } else { "px" }),
        rotating: false,
    };
    for (l, x) in [
        ("Top", &edges.top),
        ("Right", &edges.right),
        ("Bottom", &edges.bottom),
        ("Left", &edges.left),
    ] {
        out.append(&scalar_row(l, x, id, s, context));
    }
}
