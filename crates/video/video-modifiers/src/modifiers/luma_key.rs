use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::timeline_value::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LumaKeyModifier {
    pub threshold: TimelineValue<f32>,
    pub softness: TimelineValue<f32>,
    pub invert: bool,
}

impl Default for LumaKeyModifier {
    fn default() -> Self {
        Self {
            threshold: TimelineValue::<f32>::new_const(0.5),
            softness: TimelineValue::<f32>::new_const(0.1),
            invert: false,
        }
    }
}

impl ModifierModel for LumaKeyModifier {
    fn display_name(&self) -> &'static str {
        "Luma key"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["luminance key", "brightness key", "keying", "matte"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.threshold, seen);
        ensure_timeline_value_ids(&mut self.softness, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.threshold),
            timeline_value_span(&self.softness),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.threshold, &self.softness]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.threshold, &mut self.softness]
            .into_iter()
            .find(|value| value.id == id)
    }
}
