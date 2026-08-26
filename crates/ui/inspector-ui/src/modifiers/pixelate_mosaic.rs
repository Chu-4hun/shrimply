use super::{InspectorContext, ScalarOptions, integer_scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::pixelate_mosaic::PixelateMosaicModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &PixelateMosaicModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&integer_scalar_row(
        "Block width",
        &value.block_width,
        id,
        ScalarOptions {
            minimum: Some(1.0),
            maximum: Some(512.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&integer_scalar_row(
        "Block height",
        &value.block_height,
        id,
        ScalarOptions {
            minimum: Some(1.0),
            maximum: Some(512.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
}
