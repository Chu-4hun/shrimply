use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DenoiseModifier {
    #[serde(default)]
    pub engine: DenoiseEngine,
    pub amount: TimelineValue<f32>,
    pub reduction_db: TimelineValue<f32>,
}

impl Default for DenoiseModifier {
    fn default() -> Self {
        Self {
            engine: DenoiseEngine::default(),
            amount: TimelineValue::new_const(1.0),
            reduction_db: TimelineValue::new_const(12.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenoiseEngine {
    #[default]
    Rnnoise,
    DeepFilterNet,
}

impl ModifierModel for DenoiseModifier {
    fn display_name(&self) -> &'static str {
        "Denoise"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["noise removal", "noise reduction", "cleanup", "RNNoise"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amount, seen);
        ensure_timeline_value_ids(&mut self.reduction_db, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.amount),
            timeline_value_span(&self.reduction_db),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.amount, &self.reduction_db]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.amount, &mut self.reduction_db]
            .into_iter()
            .find(|value| value.id == id)
    }
}
