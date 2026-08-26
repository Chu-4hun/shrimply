use crate::InspectorContext;
use gtk::prelude::*;
use shrimply_video_modifiers::{ModifierEffect, VectorModifierEffect, repeat::RepeatModifier};
use uuid::Uuid;

use super::{ScalarOptions, integer_scalar_row, scalar_row, vec_row};

pub fn add_rows(value: &RepeatModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let count = ScalarOptions {
        minimum: Some(1.0),
        maximum: None,
        unit: None,
        rotating: false,
    };
    out.append(&integer_scalar_row(
        "Copies X",
        &value.copies_x,
        id,
        count,
        context,
    ));
    out.append(&integer_scalar_row(
        "Copies Y",
        &value.copies_y,
        id,
        count,
        context,
    ));
    out.append(&vec_row("Step", &value.step, id, false, None, context));
    out.append(&scalar_row(
        "Row offset",
        &value.row_offset,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));

    out.append(&super::step_row(
        "Offset axis",
        &value.row_offset_axis,
        id,
        context,
        "edit-vector-repeat-offset-axis",
        |modifier| match modifier {
            ModifierEffect::Vector(effect) => match &**effect {
                VectorModifierEffect::Repeat(effect) => Some(&effect.row_offset_axis),
                _ => None,
            },
            _ => None,
        },
        |modifier| match modifier {
            ModifierEffect::Vector(effect) => match &mut **effect {
                VectorModifierEffect::Repeat(effect) => Some(&mut effect.row_offset_axis),
                _ => None,
            },
            _ => None,
        },
    ));
}
