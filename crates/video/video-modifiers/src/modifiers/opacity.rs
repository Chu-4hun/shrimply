use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpacityModifier {
    pub opacity: TimelineValue<f32>,
}
impl Default for OpacityModifier {
    fn default() -> Self {
        Self {
            opacity: TimelineValue::<f32>::new_const(1.0),
        }
    }
}
impl ModifierModel for OpacityModifier {
    fn display_name(&self) -> &'static str {
        "Opacity"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.opacity, seen);
    }
    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.opacity)
    }
    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (self.opacity.id == id).then_some(&self.opacity)
    }
    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (self.opacity.id == id).then_some(&mut self.opacity)
    }
}
