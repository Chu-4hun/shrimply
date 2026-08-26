use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GaussianBlurChannels {
    #[default]
    Rgba,
    Rgb,
    Alpha,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GaussianBlurModifier {
    pub radius: TimelineValue<glam::Vec2>,
    #[serde(default)]
    pub channels: GaussianBlurChannels,
}
impl Default for GaussianBlurModifier {
    fn default() -> Self {
        Self {
            radius: TimelineValue::new_const(glam::Vec2::splat(10.0)),
            channels: GaussianBlurChannels::Rgba,
        }
    }
}
impl ModifierModel for GaussianBlurModifier {
    fn display_name(&self) -> &'static str {
        "Gaussian blur"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.radius, seen);
    }
    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.radius)
    }
    fn number2(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec2>> {
        (self.radius.id == id).then_some(&self.radius)
    }
    fn number2_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec2>> {
        (self.radius.id == id).then_some(&mut self.radius)
    }
}
