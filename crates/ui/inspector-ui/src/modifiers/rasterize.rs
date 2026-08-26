use crate::InspectorContext;
use gtk::prelude::*;
use shrimply_video_modifiers::ModifierEffect;
use shrimply_video_modifiers::rasterize::RasterizeModifier;
use uuid::Uuid;

use super::vec_row;

pub fn add_rows(value: &RasterizeModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&vec_row("Size", value.size(), id, false, None, context));

    out.append(&super::step_row(
        "Upsampling",
        &value.sample_method,
        id,
        context,
        "edit-rasterize-upsampling",
        |modifier| match modifier {
            ModifierEffect::Rasterize(effect) => Some(&effect.sample_method),
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Rasterize(effect) => Some(&mut effect.sample_method),
            _ => None,
        },
    ));
}
