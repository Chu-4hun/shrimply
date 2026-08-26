use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::{VideoSampleMethod, timeline_value::TimelineValue};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SamplingModifier {
    pub method: TimelineValue<VideoSampleMethod>,
}

impl Default for SamplingModifier {
    fn default() -> Self {
        Self {
            method: TimelineValue::new_const(VideoSampleMethod::Lanczos3),
        }
    }
}

impl ModifierModel for SamplingModifier {
    fn display_name(&self) -> &'static str {
        "Sampling"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["filtering", "nearest", "linear", "interpolation"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.method, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.method)
    }

    fn sample_method(&self, id: Uuid) -> Option<&TimelineValue<VideoSampleMethod>> {
        (self.method.id == id).then_some(&self.method)
    }

    fn sample_method_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<VideoSampleMethod>> {
        (self.method.id == id).then_some(&mut self.method)
    }
}
