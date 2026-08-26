use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::{
    ModifierEffect, RasterModifierEffect,
    halftone::{HalftoneMode, HalftoneModifier},
};
use uuid::Uuid;

pub fn add_rows(value: &HalftoneModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let position = crate::player_state::snapshot(&context.player_state).position;
    let local_time = context
        .selected_item
        .clone()
        .and_then(|key| crate::video::visual_local_time(&context.project.borrow(), key, position))
        .unwrap_or(shrimply_project::project::Time::ZERO);
    let mode = value.mode.value_at(local_time);
    out.append(&scalar_row(
        "Size",
        &value.size,
        id,
        ScalarOptions {
            minimum: Some(1.0),
            maximum: Some(100.0),
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        if mode == HalftoneMode::Monochrome {
            "Angle"
        } else {
            "Base angle"
        },
        &value.angle_degrees,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("deg"),
            rotating: true,
        },
        context,
    ));
    out.append(&scalar_row(
        "Contrast",
        &value.contrast,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(10.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&super::step_row(
        "Mode",
        &value.mode,
        id,
        context,
        "edit-halftone-mode",
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &**effect {
                RasterModifierEffect::Halftone(effect) => Some(&effect.mode),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::Halftone(effect) => Some(&mut effect.mode),
                _ => None,
            },
            _ => None,
        },
    ));
    if mode != HalftoneMode::Monochrome {
        out.append(&scalar_row(
            "Channel offset",
            &value.rgb_distance,
            id,
            ScalarOptions {
                minimum: Some(0.0),
                maximum: Some(100.0),
                unit: Some("px"),
                rotating: false,
            },
            context,
        ));
        out.append(&scalar_row(
            "Channel angle offset",
            &value.channel_angle_offset,
            id,
            ScalarOptions {
                minimum: None,
                maximum: None,
                unit: Some("deg"),
                rotating: true,
            },
            context,
        ));
    }
}
