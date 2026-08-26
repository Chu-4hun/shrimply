use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::invert::InvertModifier;
use uuid::Uuid;

pub fn add_rows(value: &InvertModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
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
