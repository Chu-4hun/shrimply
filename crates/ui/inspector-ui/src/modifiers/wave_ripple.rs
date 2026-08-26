use super::{InspectorContext, ScalarOptions, scalar_row};
use gtk::prelude::*;
use shrimply_video_modifiers::wave_ripple::WaveRippleModifier;
use uuid::Uuid;

pub fn add_rows(value: &WaveRippleModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&scalar_row(
        "Amplitude",
        &value.amplitude,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Wavelength",
        &value.wavelength,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: None,
            unit: Some("px"),
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Angle",
        &value.angle_degrees,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("deg"),
            rotating: true,
        },
        context,
    ));
    out.append(&scalar_row(
        "Phase",
        &value.phase,
        id,
        ScalarOptions {
            minimum: None,
            maximum: None,
            unit: Some("deg"),
            rotating: true,
        },
        context,
    ));
}
