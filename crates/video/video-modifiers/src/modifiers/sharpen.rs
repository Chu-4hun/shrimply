use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharpenModifier {
    pub amount: TimelineValue<f32>,
    pub radius: TimelineValue<f32>,
}
impl Default for SharpenModifier {
    fn default() -> Self {
        Self {
            amount: TimelineValue::<f32>::new_const(0.5),
            radius: TimelineValue::<f32>::new_const(2.0),
        }
    }
}
impl ModifierModel for SharpenModifier {
    fn display_name(&self) -> &'static str {
        "Sharpen"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amount, seen);
        ensure_timeline_value_ids(&mut self.radius, seen);
    }
    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.amount),
            timeline_value_span(&self.radius),
        ])
    }
    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.amount, &self.radius]
            .into_iter()
            .find(|value| value.id == id)
    }
    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.amount, &mut self.radius]
            .into_iter()
            .find(|value| value.id == id)
    }
}
