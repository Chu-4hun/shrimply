use gtk::prelude::*;
use shrimply_video_modifiers::scene_3d::SunLightModifier;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, color_row, scalar_row, vec3_row};

pub fn add_rows(value: &SunLightModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&vec3_row(
        "Rotation",
        &value.rotation_degrees,
        id,
        true,
        context,
    ));
    out.append(&color_row("Color", &value.color, id, context));
    out.append(&scalar_row(
        "Intensity",
        &value.intensity,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: None,
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Angular radius",
        &value.angular_radius_degrees,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(45.0),
            unit: Some("°"),
            rotating: false,
        },
        context,
    ));
}
