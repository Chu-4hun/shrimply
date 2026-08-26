use gtk::prelude::*;
use uuid::Uuid;

use super::{InspectorContext, color_row};
use shrimply_video_modifiers::colorize_duotone::ColorizeDuotoneModifier;

pub fn add_rows(
    value: &ColorizeDuotoneModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&color_row("Shadow color", &value.shadow_color, id, context));
    out.append(&color_row(
        "Highlight color",
        &value.highlight_color,
        id,
        context,
    ));
}
