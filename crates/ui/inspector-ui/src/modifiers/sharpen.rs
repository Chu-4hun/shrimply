use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::sharpen::SharpenModifier;
use uuid::Uuid;
pub fn add_rows(v: &SharpenModifier, o: &gtk::Box, id: Uuid, c: &InspectorContext) {
    o.append(&scalar_row(
        "Amount",
        &v.amount,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        c,
    ));
    o.append(&scalar_row(
        "Radius",
        &v.radius,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(20.0),
            unit: Some("px"),
            rotating: false,
        },
        c,
    ));
}
