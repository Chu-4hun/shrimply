use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvertModifier {
    pub amount: TimelineValue<f32>,
}

impl Default for InvertModifier {
    fn default() -> Self {
        Self {
            amount: TimelineValue::<f32>::new_const(1.0),
        }
    }
}

impl ModifierModel for InvertModifier {
    fn display_name(&self) -> &'static str {
        "Invert"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amount, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.amount)
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.amount].into_iter().find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.amount].into_iter().find(|value| value.id == id)
    }
}
