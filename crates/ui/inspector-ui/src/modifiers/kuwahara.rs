use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::kuwahara::KuwaharaModifier;
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect};
use uuid::Uuid;

pub fn add_rows(v: &KuwaharaModifier, out: &gtk::Box, id: Uuid, c: &InspectorContext) {
    out.append(&super::step_row(
        "Version",
        &v.version,
        id,
        c,
        "edit-kuwahara-version",
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &**effect {
                RasterModifierEffect::Kuwahara(effect) => Some(&effect.version),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::Kuwahara(effect) => Some(&mut effect.version),
                _ => None,
            },
            _ => None,
        },
    ));

    out.append(&scalar_row(
        "Radius",
        &v.radius,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(32.0),
            unit: Some("px"),
            rotating: false,
        },
        c,
    ));
}
