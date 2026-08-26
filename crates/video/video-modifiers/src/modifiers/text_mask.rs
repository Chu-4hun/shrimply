use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::timeline_value::TimelineValue;

pub const SNAP_THRESHOLD: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum TextMaskPartialMode {
    #[default]
    Clip,
    Fade,
    Snap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum TextMaskDirection {
    #[default]
    #[strum(to_string = "Left to right")]
    LeftToRight,
    #[strum(to_string = "Right to left")]
    RightToLeft,
    #[strum(to_string = "Top to bottom")]
    TopToBottom,
    #[strum(to_string = "Bottom to top")]
    BottomToTop,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextMaskModifier {
    pub amount: TimelineValue<f32>,
    pub partial_mode: TextMaskPartialMode,
    pub direction: TextMaskDirection,
}

impl Default for TextMaskModifier {
    fn default() -> Self {
        Self {
            amount: TimelineValue::new_const(1.0),
            partial_mode: TextMaskPartialMode::Clip,
            direction: TextMaskDirection::LeftToRight,
        }
    }
}

impl ModifierModel for TextMaskModifier {
    fn display_name(&self) -> &'static str {
        "Text mask"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["text", "reveal", "typewriter", "alpha", "clip"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amount, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.amount)
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (self.amount.id == id).then_some(&self.amount)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (self.amount.id == id).then_some(&mut self.amount)
    }
}
