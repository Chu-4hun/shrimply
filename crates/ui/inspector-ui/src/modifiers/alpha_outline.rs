use super::{InspectorContext, ScalarOptions, color_row, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::alpha_outline::AlphaOutlineModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &AlphaOutlineModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&scalar_row(
        "Width",
        &value.width,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(100.0),
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&color_row("Color", &value.color, id, context));
}
