use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::emboss::EmbossModifier;
use uuid::Uuid;

pub fn add_rows(value: &EmbossModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&scalar_row(
        "Direction",
        &value.direction_degrees,
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
        "Depth",
        &value.depth,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(20.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Amount",
        &value.amount,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(1.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
}
