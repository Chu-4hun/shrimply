use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, color_row, scalar_row};
use shrimply_video_modifiers::threshold::ThresholdModifier;

pub fn add_rows(value: &ThresholdModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&scalar_row(
        "Threshold",
        &value.threshold,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(1.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&color_row("Low color", &value.low_color, id, context));
    out.append(&color_row("High color", &value.high_color, id, context));
}
