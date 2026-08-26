use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::scanlines_crt::ScanlinesCrtModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &ScanlinesCrtModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&scalar_row(
        "Spacing",
        &value.spacing,
        id,
        ScalarOptions {
            minimum: Some(1.0),
            maximum: Some(100.0),
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Intensity",
        &value.intensity,
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
        "Curvature",
        &value.curvature,
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
        "Mask strength",
        &value.mask_strength,
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
