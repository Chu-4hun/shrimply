use super::{InspectorContext, ScalarOptions, integer_scalar_row, scalar_row, vec_row};
use gtk::prelude::*;
use shrimply_video_modifiers::kaleidoscope::KaleidoscopeModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &KaleidoscopeModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&vec_row(
        "Center",
        &value.center,
        id,
        true,
        Some((0.0, 1.0)),
        context,
    ));
    out.append(&integer_scalar_row(
        "Segments",
        &value.segments,
        id,
        ScalarOptions {
            minimum: Some(2.0),
            maximum: Some(64.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Rotation",
        &value.rotation_degrees,
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
