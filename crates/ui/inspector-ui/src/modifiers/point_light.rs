use gtk::prelude::*;
use shrimply_video_modifiers::scene_3d::PointLightModifier;
use uuid::Uuid;

use super::{InspectorContext, ScalarOptions, color_row, scalar_row, vec3_row};

pub fn add_rows(value: &PointLightModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&vec3_row("Position", &value.position, id, false, context));
    out.append(&color_row("Color", &value.color, id, context));
    for (label, value, minimum) in [
        ("Intensity", &value.intensity, 0.0),
        ("Range", &value.range, f64::EPSILON),
        ("Radius", &value.radius, 0.0),
    ] {
        out.append(&scalar_row(
            label,
            value,
            id,
            ScalarOptions {
                minimum: Some(minimum),
                maximum: None,
                unit: None,
                rotating: false,
            },
            context,
        ));
    }
}
