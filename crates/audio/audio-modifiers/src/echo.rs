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
pub struct EchoModifier {
    pub delay_ms: TimelineValue<f32>,
    pub feedback: TimelineValue<f32>,
    #[serde(default)]
    pub ping_pong: bool,
    pub mix: TimelineValue<f32>,
}

impl Default for EchoModifier {
    fn default() -> Self {
        Self {
            delay_ms: TimelineValue::new_const(250.0),
            feedback: TimelineValue::new_const(0.35),
            ping_pong: false,
            mix: TimelineValue::new_const(0.25),
        }
    }
}

impl ModifierModel for EchoModifier {
    fn display_name(&self) -> &'static str {
        "Echo"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["delay", "repeat", "feedback"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.delay_ms, seen);
        ensure_timeline_value_ids(&mut self.feedback, seen);
        ensure_timeline_value_ids(&mut self.mix, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.delay_ms),
            timeline_value_span(&self.feedback),
            timeline_value_span(&self.mix),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.delay_ms, &self.feedback, &self.mix]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.delay_ms, &mut self.feedback, &mut self.mix]
            .into_iter()
            .find(|value| value.id == id)
    }
}
