use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::modifier_model::{KeyframeSpan, ModifierModel};
use uuid::Uuid;

pub const PNEUMA_NO_MODEL: &str = "none";
pub const PNEUMA_MIN_PITCH_OFFSET: i32 = -32;
pub const PNEUMA_MAX_PITCH_OFFSET: i32 = 32;
pub const PNEUMA_MIN_SPEED: f32 = 0.5;
pub const PNEUMA_MAX_SPEED: f32 = 2.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceChangeModifier {
    pub model: String,
    #[serde(default)]
    pub pitch_offset: i32,
    #[serde(default)]
    pub f0_method: F0Method,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_true")]
    pub maintain_pitch: bool,
}

impl Default for VoiceChangeModifier {
    fn default() -> Self {
        Self {
            model: PNEUMA_NO_MODEL.to_string(),
            pitch_offset: 0,
            f0_method: F0Method::default(),
            speed: default_speed(),
            maintain_pitch: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum F0Method {
    Crepe,
    #[default]
    Rmvpe,
    Fcpe,
    SwiftF0,
}

impl F0Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crepe => "crepe",
            Self::Rmvpe => "rmvpe",
            Self::Fcpe => "fcpe",
            Self::SwiftF0 => "swift-f0",
        }
    }
}

impl ModifierModel for VoiceChangeModifier {
    fn display_name(&self) -> &'static str {
        "Voice Change"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["Pneuma", "voice conversion", "RVC", "speaker"]
    }

    fn ensure_ids(&mut self, _seen: &mut HashSet<Uuid>) {}

    fn keyframe_span(&self) -> KeyframeSpan {
        KeyframeSpan::default()
    }
}

fn default_speed() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}
