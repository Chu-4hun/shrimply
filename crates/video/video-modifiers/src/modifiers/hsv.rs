use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::timeline_value::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HsvModifier {
    pub hue_degrees: TimelineValue<f32>,
    pub saturation: TimelineValue<f32>,
    pub value: TimelineValue<f32>,
}

impl Default for HsvModifier {
    fn default() -> Self {
        Self {
            hue_degrees: TimelineValue::new_const(0.0),
            saturation: TimelineValue::new_const(1.0),
            value: TimelineValue::new_const(1.0),
        }
    }
}

impl ModifierModel for HsvModifier {
    fn display_name(&self) -> &'static str {
        "HSV"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["hue", "saturation", "value", "color", "colour"]
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

impl HsvModifier {
    fn values(&self) -> impl Iterator<Item = &TimelineValue<f32>> {
        [&self.hue_degrees, &self.saturation, &self.value].into_iter()
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut TimelineValue<f32>> {
        [&mut self.hue_degrees, &mut self.saturation, &mut self.value].into_iter()
    }
}
