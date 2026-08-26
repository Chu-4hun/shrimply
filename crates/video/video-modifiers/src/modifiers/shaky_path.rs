use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::timeline_value::*;

fn default_amplitude() -> TimelineValue<f32> {
    TimelineValue::<f32>::new_const(4.0)
}

fn default_step_size() -> TimelineValue<f32> {
    TimelineValue::<f32>::new_const(10.0)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShakyPathModifier {
    #[serde(default = "default_amplitude")]
    pub amplitude: TimelineValue<f32>,
    #[serde(default = "default_step_size")]
    pub step_size: TimelineValue<f32>,
    #[serde(default)]
    pub seed: TimelineValue<f32>,
    #[serde(default)]
    pub evolution: TimelineValue<f32>,
}

impl Default for ShakyPathModifier {
    fn default() -> Self {
        Self {
            amplitude: default_amplitude(),
            step_size: default_step_size(),
            seed: TimelineValue::<f32>::new_const(0.0),
            evolution: TimelineValue::<f32>::new_const(0.0),
        }
    }
}

impl ModifierModel for ShakyPathModifier {
    fn display_name(&self) -> &'static str {
        "Shaky path"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["shake", "jitter", "wiggle", "distortion", "warp", "roughen"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amplitude, seen);
        ensure_timeline_value_ids(&mut self.step_size, seen);
        ensure_timeline_value_ids(&mut self.seed, seen);
        ensure_timeline_value_ids(&mut self.evolution, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.amplitude),
            timeline_value_span(&self.step_size),
            timeline_value_span(&self.seed),
            timeline_value_span(&self.evolution),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [
            &self.amplitude,
            &self.step_size,
            &self.seed,
            &self.evolution,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.amplitude,
            &mut self.step_size,
            &mut self.seed,
            &mut self.evolution,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }
}
