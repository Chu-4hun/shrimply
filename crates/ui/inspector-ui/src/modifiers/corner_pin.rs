use gtk::prelude::*;
use shrimply_video_modifiers::corner_pin::CornerPinModifier;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, scalar_row, vec_row};

pub fn add_rows(value: &CornerPinModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    for (label, corner) in [
        ("Top left", &value.top_left),
        ("Top right", &value.top_right),
        ("Bottom right", &value.bottom_right),
        ("Bottom left", &value.bottom_left),
    ] {
        out.append(&vec_row(label, corner, id, true, Some((0.0, 1.0)), context));
    }
    out.append(&scalar_row(
        "Perspective",
        &value.perspective,
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
