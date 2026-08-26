use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::channel_mixer::ChannelMixerModifier;
use uuid::Uuid;

pub fn add_rows(
    value: &ChannelMixerModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&scalar_row(
        "Red ← red",
        &value.rr,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Red ← green",
        &value.rg,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Red ← blue",
        &value.rb,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Green ← red",
        &value.gr,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Green ← green",
        &value.gg,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Green ← blue",
        &value.gb,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Blue ← red",
        &value.br,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Blue ← green",
        &value.bg,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Blue ← blue",
        &value.bb,
        id,
        ScalarOptions {
            minimum: Some(-2.0),
            maximum: Some(2.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
}
