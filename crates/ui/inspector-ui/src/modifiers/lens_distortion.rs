use super::{InspectorContext, ScalarOptions, scalar_row, vec_row};
use gtk::prelude::*;
use shrimply_video_modifiers::lens_distortion::LensDistortionModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &LensDistortionModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&scalar_row(
        "Distortion",
        &value.distortion,
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
