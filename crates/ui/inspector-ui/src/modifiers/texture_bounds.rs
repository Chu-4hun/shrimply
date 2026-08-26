use crate::InspectorContext;
use gtk::prelude::*;
use shrimply_video_modifiers::{
    ModifierEffect, RasterModifierEffect, texture_bounds::TextureBoundsModifier,
};
use uuid::Uuid;

use super::{ScalarOptions, scalar_row};

pub fn add_rows(
    value: &TextureBoundsModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    let options = ScalarOptions {
        minimum: None,
        maximum: None,
        unit: Some("px"),
        rotating: false,
    };
    for (label, edge) in [
        ("Top", &value.edges.top),
        ("Right", &value.edges.right),
        ("Bottom", &value.edges.bottom),
        ("Left", &value.edges.left),
    ] {
        out.append(&scalar_row(label, edge, id, options, context));
    }

    out.append(&super::step_row(
        "Addressing",
        &value.address_mode,
        id,
        context,
        "edit-texture-addressing",
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &**effect {
                RasterModifierEffect::TextureBounds(effect) => Some(&effect.address_mode),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::TextureBounds(effect) => Some(&mut effect.address_mode),
                _ => None,
            },
            _ => None,
        },
    ));
}
