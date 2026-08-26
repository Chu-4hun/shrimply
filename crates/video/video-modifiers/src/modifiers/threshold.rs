use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::{Color, timeline_value::TimelineValue};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThresholdModifier {
    pub threshold: TimelineValue<f32>,
    pub low_color: TimelineValue<shrimply_core::Color<u8>>,
    pub high_color: TimelineValue<shrimply_core::Color<u8>>,
}

impl Default for ThresholdModifier {
    fn default() -> Self {
        Self {
            threshold: TimelineValue::<f32>::new_const(0.5),
            low_color: TimelineValue::<Color<u8>>::new_const(Color::<u8>::BLACK),
            high_color: TimelineValue::<Color<u8>>::new_const(Color::<u8>::WHITE),
        }
    }
}

impl ModifierModel for ThresholdModifier {
    fn display_name(&self) -> &'static str {
        "Threshold"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.threshold, seen);
        ensure_timeline_value_ids(&mut self.low_color, seen);
        ensure_timeline_value_ids(&mut self.high_color, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.threshold),
            timeline_value_span(&self.low_color),
            timeline_value_span(&self.high_color),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (self.threshold.id == id).then_some(&self.threshold)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (self.threshold.id == id).then_some(&mut self.threshold)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        [&mut self.low_color, &mut self.high_color]
            .into_iter()
            .find(|color| color.id == id)
    }
}
