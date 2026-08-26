use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span},
    timeline_value::TimelineValue,
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceColorModifier {
    pub amount: TimelineValue<f32>,
    #[serde(default = "super::default_true")]
    pub auto_level: bool,
}

impl Default for VoiceColorModifier {
    fn default() -> Self {
        Self {
            amount: TimelineValue::new_const(0.35),
            auto_level: true,
        }
    }
}

impl ModifierModel for VoiceColorModifier {
    fn display_name(&self) -> &'static str {
        "Phone mic"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["microphone", "phone", "recording", "preamp", "AGC"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.amount, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        timeline_value_span(&self.amount)
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (self.amount.id == id).then_some(&self.amount)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (self.amount.id == id).then_some(&mut self.amount)
    }
}
