use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use uuid::Uuid;

use crate::default_true;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PitchModifier {
    pub semitones: TimelineValue<f32>,
    #[serde(default = "default_formant_semitones")]
    pub formant_semitones: TimelineValue<f32>,
    #[serde(default = "default_true")]
    pub preserve_formants: bool,
    #[serde(default)]
    pub quality: PitchQuality,
    #[serde(default = "default_true")]
    pub link_channels: bool,
}

impl Default for PitchModifier {
    fn default() -> Self {
        Self {
            semitones: TimelineValue::new_const(0.0),
            formant_semitones: default_formant_semitones(),
            preserve_formants: true,
            quality: PitchQuality::default(),
            link_channels: true,
        }
    }
}

fn default_formant_semitones() -> TimelineValue<f32> {
    TimelineValue::new_const(0.0)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PitchQuality {
    LowLatency,
    #[default]
    Balanced,
}

impl ModifierModel for PitchModifier {
    fn display_name(&self) -> &'static str {
        "Pitch"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["transpose", "semitones", "formant", "tune"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.semitones, seen);
        ensure_timeline_value_ids(&mut self.formant_semitones, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.semitones),
            timeline_value_span(&self.formant_semitones),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.semitones, &self.formant_semitones]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.semitones, &mut self.formant_semitones]
            .into_iter()
            .find(|value| value.id == id)
    }
}
