use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::timeline_value::*;

fn default_amplitude() -> TimelineValue<f32> {
    TimelineValue::<f32>::new_const(2.0)
}

fn default_spacing() -> TimelineValue<f32> {
    TimelineValue::<f32>::new_const(12.0)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathOffsetModifier {
    #[serde(default = "default_amplitude")]
    pub amplitude: TimelineValue<f32>,
    #[serde(default = "default_spacing")]
    pub spacing: TimelineValue<f32>,
    #[serde(default)]
    pub seed: TimelineValue<f32>,
    #[serde(default)]
    pub evolution: TimelineValue<f32>,
}

impl Default for PathOffsetModifier {
    fn default() -> Self {
        Self {
            amplitude: default_amplitude(),
            spacing: default_spacing(),
            seed: TimelineValue::<f32>::new_const(0.0),
            evolution: TimelineValue::<f32>::new_const(0.0),
        }
    }
}

impl ModifierModel for PathOffsetModifier {
    fn display_name(&self) -> &'static str {
        "Path offset"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["wiggle", "wave", "displace", "distortion", "warp"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amplitude, seen);
        ensure_timeline_value_ids(&mut self.spacing, seen);
        ensure_timeline_value_ids(&mut self.seed, seen);
        ensure_timeline_value_ids(&mut self.evolution, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.amplitude),
            timeline_value_span(&self.spacing),
            timeline_value_span(&self.seed),
            timeline_value_span(&self.evolution),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.amplitude, &self.spacing, &self.seed, &self.evolution]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.amplitude,
            &mut self.spacing,
            &mut self.seed,
            &mut self.evolution,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }
}
