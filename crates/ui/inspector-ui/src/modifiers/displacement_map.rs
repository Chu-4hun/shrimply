use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::displacement_map::DisplacementMapModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &DisplacementMapModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&scalar_row(
        "Amount",
        &value.amount,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Scale",
        &value.scale,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: None,
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Phase",
        &value.phase,
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
