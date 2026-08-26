use super::{InspectorContext, ScalarOptions, scalar_row, vec_row};
use gtk::prelude::*;
use shrimply_video_modifiers::twirl::TwirlModifier;
use uuid::Uuid;

pub fn add_rows(value: &TwirlModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&vec_row(
        "Center",
        &value.center,
        id,
        true,
        Some((0.0, 1.0)),
        context,
    ));
    out.append(&scalar_row(
        "Radius",
        &value.radius,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(1.0),
            unit: Some("x"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Angle",
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
}
