use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::glow_bloom::GlowBloomModifier;
use uuid::Uuid;

pub fn add_rows(value: &GlowBloomModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
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
    out.append(&scalar_row(
        "Radius",
        &value.radius,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(100.0),
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Intensity",
        &value.intensity,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(10.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
}
