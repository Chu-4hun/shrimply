use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilmGrainModifier {
    pub amount: TimelineValue<f32>,
    pub size: TimelineValue<f32>,
    pub colored: TimelineValue<f32>,
    pub seed: TimelineValue<f32>,
}

impl Default for FilmGrainModifier {
    fn default() -> Self {
        Self {
            amount: TimelineValue::<f32>::new_const(0.1),
            size: TimelineValue::<f32>::new_const(1.0),
            colored: TimelineValue::<f32>::new_const(0.0),
            seed: TimelineValue::<f32>::new_const(0.0),
        }
    }
}

impl ModifierModel for FilmGrainModifier {
    fn display_name(&self) -> &'static str {
        "Noise"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["film grain", "grain", "analog", "texture"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amount, seen);
        ensure_timeline_value_ids(&mut self.size, seen);
        ensure_timeline_value_ids(&mut self.colored, seen);
        ensure_timeline_value_ids(&mut self.seed, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.amount),
            timeline_value_span(&self.size),
            timeline_value_span(&self.colored),
            timeline_value_span(&self.seed),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.amount, &self.size, &self.colored, &self.seed]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.amount,
            &mut self.size,
            &mut self.colored,
            &mut self.seed,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }
}
