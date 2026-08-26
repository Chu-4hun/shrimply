use super::{InspectorContext, ScalarOptions, scalar_row, vec_row};
use gtk::prelude::*;
use shrimply_video_modifiers::fisheye::FisheyeModifier;
use uuid::Uuid;

pub fn add_rows(value: &FisheyeModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&scalar_row(
        "Intensity",
        &value.intensity,
        id,
        ScalarOptions {
            minimum: Some(-1.0),
            maximum: Some(1.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&vec_row(
        "Center",
        &value.center,
        id,
        true,
        Some((0.0, 1.0)),
        context,
    ));
}
