use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlphaOutlineModifier {
    pub width: TimelineValue<f32>,
    pub color: TimelineValue<shrimply_core::Color<u8>>,
}

impl Default for AlphaOutlineModifier {
    fn default() -> Self {
        Self {
            width: TimelineValue::<f32>::new_const(3.0),
            color: TimelineValue::<shrimply_core::Color<u8>>::new_const(
                shrimply_core::Color::<u8>::BLACK,
            ),
        }
    }
}

impl ModifierModel for AlphaOutlineModifier {
    fn display_name(&self) -> &'static str {
        "Alpha outline"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["stroke", "border", "contour", "edge"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.width, seen);
        ensure_timeline_value_ids(&mut self.color, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.width),
            timeline_value_span(&self.color),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.width].into_iter().find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.width].into_iter().find(|value| value.id == id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        [&mut self.color].into_iter().find(|value| value.id == id)
    }
}
