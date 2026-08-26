use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::{Color, timeline_value::TimelineValue};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorizeDuotoneModifier {
    pub shadow_color: TimelineValue<shrimply_core::Color<u8>>,
    pub highlight_color: TimelineValue<shrimply_core::Color<u8>>,
}

impl Default for ColorizeDuotoneModifier {
    fn default() -> Self {
        Self {
            shadow_color: TimelineValue::<Color<u8>>::new_const(Color::<u8>::BLACK),
            highlight_color: TimelineValue::<Color<u8>>::new_const(Color::<u8>::WHITE),
        }
    }
}

impl ModifierModel for ColorizeDuotoneModifier {
    fn display_name(&self) -> &'static str {
        "Duotone"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["colorize", "tint", "recolor", "two color", "two colour"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.shadow_color, seen);
        ensure_timeline_value_ids(&mut self.highlight_color, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.shadow_color),
            timeline_value_span(&self.highlight_color),
        ])
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        [&mut self.shadow_color, &mut self.highlight_color]
            .into_iter()
            .find(|color| color.id == id)
    }
}
