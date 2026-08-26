use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, scalar_row};
use shrimply_video_modifiers::hsv::HsvModifier;

pub fn add_rows(value: &HsvModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&scalar_row(
        "Hue",
        &value.hue_degrees,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("deg"),
            rotating: true,
        },
        context,
    ));
    for (label, value) in [("Saturation", &value.saturation), ("Value", &value.value)] {
        out.append(&scalar_row(
            label,
            value,
            id,
            ScalarOptions {
                minimum: Some(0.0),
                maximum: Some(2.0),
                unit: None,
                rotating: false,
            },
            context,
        ));
    }
}
