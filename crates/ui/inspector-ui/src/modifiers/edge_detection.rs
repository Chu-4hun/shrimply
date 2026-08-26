use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, color_row, scalar_row};
use shrimply_video_modifiers::edge_detection::EdgeDetectionModifier;

pub fn add_rows(
    value: &EdgeDetectionModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
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
    out.append(&color_row("Edge color", &value.edge_color, id, context));
    out.append(&color_row(
        "Background color",
        &value.background_color,
        id,
        context,
    ));
}
