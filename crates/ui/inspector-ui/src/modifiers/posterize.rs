use super::{InspectorContext, ScalarOptions, integer_scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::posterize::PosterizeModifier;
use uuid::Uuid;

pub fn add_rows(value: &PosterizeModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&integer_scalar_row(
        "Levels",
        &value.levels,
        id,
        ScalarOptions {
            minimum: Some(2.0),
            maximum: Some(256.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
}
