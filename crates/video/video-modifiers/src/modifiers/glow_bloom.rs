use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlowBloomModifier {
    pub threshold: TimelineValue<f32>,
    pub radius: TimelineValue<f32>,
    pub intensity: TimelineValue<f32>,
}

impl Default for GlowBloomModifier {
    fn default() -> Self {
        Self {
            threshold: TimelineValue::<f32>::new_const(0.7),
            radius: TimelineValue::<f32>::new_const(10.0),
            intensity: TimelineValue::<f32>::new_const(1.0),
        }
    }
}

impl ModifierModel for GlowBloomModifier {
    fn display_name(&self) -> &'static str {
        "Glow"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["bloom", "glare", "halo", "light spill"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.threshold, seen);
        ensure_timeline_value_ids(&mut self.radius, seen);
        ensure_timeline_value_ids(&mut self.intensity, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.threshold),
            timeline_value_span(&self.radius),
            timeline_value_span(&self.intensity),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.threshold, &self.radius, &self.intensity]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.threshold, &mut self.radius, &mut self.intensity]
            .into_iter()
            .find(|value| value.id == id)
    }
}
