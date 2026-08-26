use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PosterizeModifier {
    pub levels: TimelineValue<f32>,
}

impl Default for PosterizeModifier {
    fn default() -> Self {
        Self {
            levels: TimelineValue::<f32>::new_const(8.0),
        }
    }
}

impl ModifierModel for PosterizeModifier {
    fn display_name(&self) -> &'static str {
        "Posterize"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.levels, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.levels)
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.levels].into_iter().find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.levels].into_iter().find(|value| value.id == id)
    }
}
