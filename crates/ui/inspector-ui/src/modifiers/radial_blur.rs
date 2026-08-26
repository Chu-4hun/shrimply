use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, integer_scalar_row, scalar_row, vec_row};
use shrimply_video_modifiers::radial_blur::RadialBlurModifier;

pub fn add_rows(value: &RadialBlurModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&vec_row(
        "Center",
        &value.center,
        id,
        true,
        Some((0.0, 1.0)),
        context,
    ));
    out.append(&scalar_row(
        "Angle",
        &value.angle_degrees,
        id,
        ScalarOptions {
            minimum: Some(-360.0),
            maximum: Some(360.0),
            unit: Some("deg"),
            rotating: true,
        },
        context,
    ));
    out.append(&integer_scalar_row(
        "Samples",
        &value.samples,
        id,
        ScalarOptions {
            minimum: Some(2.0),
            maximum: Some(128.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
}
