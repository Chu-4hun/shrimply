use super::{InspectorContext, deferred_context, visual_duration, visual_local_time};
use crate::player_state::ProjectChange;
use crate::timeline_value::vector::vec2::{VecAccess, VecSpec, VecTarget, vec_control_with_lock};
use gtk::prelude::*;
use shrimply_video_modifiers::gaussian_blur::GaussianBlurModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &GaussianBlurModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    let context = deferred_context(context);
    out.append(&vec_control_with_lock(
        "Radius",
        &value.radius,
        &context,
        VecTarget {
            access: VecAccess::Modifier {
                id,
                value_id: value.radius.id,
            },
            scope_id: Some(value.radius.id),
            local_time: visual_local_time,
            duration: visual_duration,
            refresh: ProjectChange {
                video: true,
                ..Default::default()
            },
            commit_name: "visual-modifier-vector",
        },
        VecSpec {
            first_prefix: "X",
            second_prefix: "Y",
            drag_step: 1.0,
            digits: 0,
            width_chars: 7,
            minimum: Some(0.0),
            maximum: Some(100.0),
            unit_name: "px",
        },
        true,
    ));
}
