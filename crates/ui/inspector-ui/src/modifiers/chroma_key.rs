use super::{InspectorContext, ScalarOptions, color_row, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::chroma_key::ChromaKeyModifier;
use uuid::Uuid;
pub fn add_rows(v: &ChromaKeyModifier, o: &gtk::Box, id: Uuid, c: &InspectorContext) {
    o.append(&color_row("Key color", &v.key_color, id, c));
    let s = ScalarOptions {
        minimum: Some(0.0),
        maximum: Some(1.0),
        unit: None,
        rotating: false,
    };
    o.append(&scalar_row("Similarity", &v.similarity, id, s, c));
    o.append(&scalar_row("Softness", &v.softness, id, s, c));
    o.append(&scalar_row("Spill", &v.spill_suppression, id, s, c));
}
