use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::{Color, timeline_value::TimelineValue};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EdgeDetectionModifier {
    pub amount: TimelineValue<f32>,
    pub edge_color: TimelineValue<shrimply_core::Color<u8>>,
    pub background_color: TimelineValue<shrimply_core::Color<u8>>,
}

impl Default for EdgeDetectionModifier {
    fn default() -> Self {
        Self {
            amount: TimelineValue::<f32>::new_const(1.0),
            edge_color: TimelineValue::<Color<u8>>::new_const(Color::<u8>::WHITE),
            background_color: TimelineValue::<Color<u8>>::new_const(Color::<u8>::BLACK),
        }
    }
}

impl ModifierModel for EdgeDetectionModifier {
    fn display_name(&self) -> &'static str {
        "Edge detection"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amount, seen);
        ensure_timeline_value_ids(&mut self.edge_color, seen);
        ensure_timeline_value_ids(&mut self.background_color, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.amount),
            timeline_value_span(&self.edge_color),
            timeline_value_span(&self.background_color),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (self.amount.id == id).then_some(&self.amount)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (self.amount.id == id).then_some(&mut self.amount)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        [&mut self.edge_color, &mut self.background_color]
            .into_iter()
            .find(|color| color.id == id)
    }
}
