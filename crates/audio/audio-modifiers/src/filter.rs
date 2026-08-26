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
pub struct FilterModifier {
    #[serde(default)]
    pub mode: FilterMode,
    pub cutoff_hz: TimelineValue<f32>,
    pub resonance: TimelineValue<f32>,
}

impl Default for FilterModifier {
    fn default() -> Self {
        Self {
            mode: FilterMode::default(),
            cutoff_hz: TimelineValue::new_const(5_000.0),
            resonance: TimelineValue::new_const(std::f32::consts::FRAC_1_SQRT_2),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    LowPass,
    HighPass,
}

impl ModifierModel for FilterModifier {
    fn display_name(&self) -> &'static str {
        "Filter"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["low pass", "high pass", "band pass", "cutoff", "EQ"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.cutoff_hz, seen);
        ensure_timeline_value_ids(&mut self.resonance, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.cutoff_hz),
            timeline_value_span(&self.resonance),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.cutoff_hz, &self.resonance]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.cutoff_hz, &mut self.resonance]
            .into_iter()
            .find(|value| value.id == id)
    }
}
