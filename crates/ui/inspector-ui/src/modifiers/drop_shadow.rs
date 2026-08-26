use super::{InspectorContext, ScalarOptions, color_row, scalar_row, vec_row};
use gtk::prelude::*;
use shrimply_video_modifiers::drop_shadow::DropShadowModifier;
use uuid::Uuid;

pub fn add_rows(value: &DropShadowModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&vec_row("Offset", &value.offset, id, false, None, context));
    out.append(&scalar_row(
        "Blur",
        &value.blur_radius,
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
