use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, scalar_row};
use shrimply_video_modifiers::color_correction::ColorCorrectionModifier;

pub fn add_rows(
    value: &ColorCorrectionModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    let row = |label, value, minimum, maximum, unit, rotating| {
        scalar_row(
            label,
            value,
            id,
            ScalarOptions {
                minimum,
                maximum,
                unit,
                rotating,
            },
            context,
        )
    };
    out.append(&row(
        "Exposure",
        &value.exposure,
        Some(-10.0),
        Some(10.0),
        Some("stops"),
        false,
    ));
    out.append(&row(
        "Gamma",
        &value.gamma,
        Some(0.01),
        Some(10.0),
        None,
        false,
    ));
    out.append(&row(
        "Temperature",
        &value.temperature,
        Some(-1.0),
        Some(1.0),
        None,
        false,
    ));
    out.append(&row(
        "Tint",
        &value.tint,
        Some(-1.0),
        Some(1.0),
        None,
        false,
    ));
    out.append(&row(
        "Brightness",
        &value.brightness,
        Some(-1.0),
        Some(1.0),
        None,
        false,
    ));
    out.append(&row(
        "Contrast",
        &value.contrast,
        Some(-1.0),
        Some(1.0),
        None,
        false,
    ));
    out.append(&row(
        "Hue",
        &value.hue_degrees,
        None,
        None,
        Some("deg"),
        true,
    ));
    out.append(&row(
        "Saturation",
        &value.saturation,
        Some(0.0),
        Some(2.0),
        None,
        false,
    ));
    out.append(&row(
        "Value",
        &value.value,
        Some(0.0),
        Some(2.0),
        None,
        false,
    ));
}
