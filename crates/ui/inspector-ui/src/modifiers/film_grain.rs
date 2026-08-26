use super::{InspectorContext, ScalarOptions, integer_scalar_row, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::film_grain::FilmGrainModifier;
use uuid::Uuid;

pub fn add_rows(value: &FilmGrainModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
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
    out.append(&scalar_row(
        "Size",
        &value.size,
        id,
        ScalarOptions {
            minimum: Some(0.1),
            maximum: Some(20.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Color",
        &value.colored,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(1.0),
            unit: None,
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
}
