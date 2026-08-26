use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{modifier_model::ensure_timeline_value_ids, timeline_value::TimelineValue};
use uuid::Uuid;

pub const DEFAULT_AUDIO_GENERATOR_FREQUENCY_HZ: f32 = 440.0;
pub const DEFAULT_AUDIO_GENERATOR_PULSE_WIDTH: f32 = 0.5;

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum AudioWaveform {
    #[default]
    Sine,
    #[strum(to_string = "Square / Pulse")]
    SquarePulse,
    Triangle,
    Sawtooth,
    #[strum(to_string = "White Noise")]
    WhiteNoise,
    #[strum(to_string = "Pink Noise")]
    PinkNoise,
    #[strum(to_string = "Brown Noise")]
    BrownNoise,
}

impl AudioWaveform {
    pub fn is_noise(self) -> bool {
        matches!(self, Self::WhiteNoise | Self::PinkNoise | Self::BrownNoise)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioGenerator {
    #[serde(default)]
    pub waveform: AudioWaveform,
    #[serde(default = "default_frequency_hz")]
    pub frequency_hz: TimelineValue<f32>,
    #[serde(default = "default_pulse_width")]
    pub pulse_width: TimelineValue<f32>,
    #[serde(default)]
    pub seed: TimelineValue<f32>,
}

impl Default for AudioGenerator {
    fn default() -> Self {
        Self {
            waveform: AudioWaveform::default(),
            frequency_hz: default_frequency_hz(),
            pulse_width: default_pulse_width(),
            seed: TimelineValue::new_const(0.0),
        }
    }
}

impl AudioGenerator {
    pub fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.frequency_hz, &self.pulse_width, &self.seed]
            .into_iter()
            .find(|value| value.id == id)
    }

    pub fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.frequency_hz,
            &mut self.pulse_width,
            &mut self.seed,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    pub fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.frequency_hz, seen);
        ensure_timeline_value_ids(&mut self.pulse_width, seen);
        ensure_timeline_value_ids(&mut self.seed, seen);
    }
}

fn default_frequency_hz() -> TimelineValue<f32> {
    TimelineValue::new_const(DEFAULT_AUDIO_GENERATOR_FREQUENCY_HZ)
}

fn default_pulse_width() -> TimelineValue<f32> {
    TimelineValue::new_const(DEFAULT_AUDIO_GENERATOR_PULSE_WIDTH)
}
