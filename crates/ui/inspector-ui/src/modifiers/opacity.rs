use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::opacity::OpacityModifier;
use uuid::Uuid;
pub fn add_rows(v: &OpacityModifier, o: &gtk::Box, id: Uuid, c: &InspectorContext) {
    o.append(&scalar_row(
        "Opacity",
        &v.opacity,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(1.0),
            unit: None,
            rotating: false,
        },
        c,
    ));
}
