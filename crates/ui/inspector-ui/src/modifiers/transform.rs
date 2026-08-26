use super::{InspectorContext, ScalarOptions, scalar_row, vec_row};
use gtk::prelude::*;
use shrimply_video_modifiers::transform::TransformModifier;
use uuid::Uuid;
pub fn add_rows(v: &TransformModifier, out: &gtk::Box, id: Uuid, c: &InspectorContext) {
    out.append(&vec_row("Position", v.position(), id, false, None, c));
    out.append(&vec_row("Anchor", v.anchor(), id, false, None, c));
    out.append(&vec_row("Scale", v.scale(), id, true, None, c));
    out.append(&vec_row("Shear", v.shear(), id, false, None, c));
    out.append(&scalar_row(
        "Rotation",
        v.rotation_degrees(),
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("°"),
            rotating: true,
        },
        c,
    ));
}
