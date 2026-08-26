use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, integer_scalar_row, scalar_row};
use shrimply_video_modifiers::shaky_path::ShakyPathModifier;

pub fn add_rows(value: &ShakyPathModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&scalar_row(
        "Amplitude",
        &value.amplitude,
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
        "Step size",
        &value.step_size,
        id,
        ScalarOptions {
            minimum: Some(0.1),
            maximum: None,
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));

    out.append(&integer_scalar_row(
        "Seed",
        &value.seed,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: None,
            unit: None,
            rotating: false,
        },
        context,
    ));

    out.append(&integer_scalar_row(
        "Evolution",
        &value.evolution,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: None,
            rotating: false,
        },
        context,
    ));
}
