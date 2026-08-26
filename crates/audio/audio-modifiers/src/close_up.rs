use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span},
    timeline_value::TimelineValue,
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloseUpModifier {
    pub distance_cm: TimelineValue<f32>,
}

impl Default for CloseUpModifier {
    fn default() -> Self {
        Self {
            distance_cm: TimelineValue::new_const(15.0),
        }
    }
}

impl ModifierModel for CloseUpModifier {
    fn display_name(&self) -> &'static str {
        "Close up"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["close mic", "intimate", "proximity", "warm", "podcast"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.distance_cm, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.distance_cm)
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (self.distance_cm.id == id).then_some(&self.distance_cm)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (self.distance_cm.id == id).then_some(&mut self.distance_cm)
    }
}
