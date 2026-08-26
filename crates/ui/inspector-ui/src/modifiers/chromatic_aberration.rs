use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::chromatic_aberration::ChromaticAberrationModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &ChromaticAberrationModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&scalar_row(
        "Red X",
        &value.red_offset_x,
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
        "Red Y",
        &value.red_offset_y,
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
        "Blue X",
        &value.blue_offset_x,
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
        "Blue Y",
        &value.blue_offset_y,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
}
