use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::timeline_value::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorCorrectionModifier {
    pub exposure: TimelineValue<f32>,
    pub gamma: TimelineValue<f32>,
    pub temperature: TimelineValue<f32>,
    pub tint: TimelineValue<f32>,
    pub brightness: TimelineValue<f32>,
    pub contrast: TimelineValue<f32>,
    pub hue_degrees: TimelineValue<f32>,
    pub saturation: TimelineValue<f32>,
    pub value: TimelineValue<f32>,
}

impl Default for ColorCorrectionModifier {
    fn default() -> Self {
        Self {
            exposure: TimelineValue::<f32>::new_const(0.0),
            gamma: TimelineValue::<f32>::new_const(1.0),
            temperature: TimelineValue::<f32>::new_const(0.0),
            tint: TimelineValue::<f32>::new_const(0.0),
            brightness: TimelineValue::<f32>::new_const(0.0),
            contrast: TimelineValue::<f32>::new_const(0.0),
            hue_degrees: TimelineValue::<f32>::new_const(0.0),
            saturation: TimelineValue::<f32>::new_const(1.0),
            value: TimelineValue::<f32>::new_const(1.0),
        }
    }
}

impl ModifierModel for ColorCorrectionModifier {
    fn display_name(&self) -> &'static str {
        "Color correction"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &[
            "color grade",
            "colour grade",
            "grading",
            "exposure",
            "contrast",
        ]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        for value in self.values_mut() {
            ensure_timeline_value_ids(value, seen);
        }
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine(self.values().map(timeline_value_span))
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        self.values().find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        self.values_mut().find(|value| value.id == id)
    }
}

impl ColorCorrectionModifier {
    fn values(&self) -> impl Iterator<Item = &TimelineValue<f32>> {
        [
            &self.exposure,
            &self.gamma,
            &self.temperature,
            &self.tint,
            &self.brightness,
            &self.contrast,
            &self.hue_degrees,
            &self.saturation,
            &self.value,
        ]
        .into_iter()
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut TimelineValue<f32>> {
        [
            &mut self.exposure,
            &mut self.gamma,
            &mut self.temperature,
            &mut self.tint,
            &mut self.brightness,
            &mut self.contrast,
            &mut self.hue_degrees,
            &mut self.saturation,
            &mut self.value,
        ]
        .into_iter()
    }
}
