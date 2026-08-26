use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::vignette::VignetteModifier;
use uuid::Uuid;
pub fn add_rows(v: &VignetteModifier, o: &gtk::Box, id: Uuid, c: &InspectorContext) {
    let s = ScalarOptions {
        minimum: Some(0.0),
        maximum: Some(1.0),
        unit: None,
        rotating: false,
    };
    o.append(&scalar_row("Amount", &v.amount, id, s, c));
    o.append(&scalar_row("Midpoint", &v.midpoint, id, s, c));
    o.append(&scalar_row("Softness", &v.softness, id, s, c));
}
