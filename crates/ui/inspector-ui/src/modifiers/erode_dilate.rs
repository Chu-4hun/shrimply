use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, integer_scalar_row};
use shrimply_video_modifiers::{
    ModifierEffect, RasterModifierEffect, erode_dilate::ErodeDilateModifier,
};

pub fn add_rows(value: &ErodeDilateModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&super::step_row(
        "Operation",
        &value.operation,
        id,
        context,
        "edit-erode-dilate-operation",
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &**effect {
                RasterModifierEffect::ErodeDilate(effect) => Some(&effect.operation),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::ErodeDilate(effect) => Some(&mut effect.operation),
                _ => None,
            },
            _ => None,
        },
    ));
    out.append(&integer_scalar_row(
        "Radius",
        &value.radius,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(128.0),
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
}
