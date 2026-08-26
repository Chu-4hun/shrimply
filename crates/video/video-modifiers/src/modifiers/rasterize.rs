use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::{VideoSampleMethod, timeline_value::TimelineValue};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RasterizeModifier {
    #[serde(default)]
    marker: bool,
    #[serde(
        default,
        deserialize_with = "shrimply_core::timeline_value::deserialize_timeline_value"
    )]
    pub sample_method: TimelineValue<VideoSampleMethod>,
    #[serde(default)]
    size: TimelineValue<glam::Vec2>,
}

impl Default for RasterizeModifier {
    fn default() -> Self {
        Self::new(glam::Vec2::ZERO)
    }
}

impl RasterizeModifier {
    pub fn new(size: glam::Vec2) -> Self {
        Self {
            marker: false,
            sample_method: TimelineValue::new_const(Default::default()),
            size: TimelineValue::new_const(size),
        }
    }

    pub fn size(&self) -> &TimelineValue<glam::Vec2> {
        &self.size
    }
}

impl ModifierModel for RasterizeModifier {
    fn display_name(&self) -> &'static str {
        "Rasterize"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.sample_method, seen);
        ensure_timeline_value_ids(&mut self.size, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        super::combine([
            timeline_value_span(&self.sample_method),
            timeline_value_span(&self.size),
        ])
    }

    fn number2(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec2>> {
        (self.size.id == id).then_some(&self.size)
    }

    fn number2_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec2>> {
        (self.size.id == id).then_some(&mut self.size)
    }
}
