mod cache;
mod close_up;
mod denoise;
mod echo;
mod effect;
mod filter;
mod modifier;
mod pitch;
mod reverb;
mod scalar;
mod voice_change;
mod voice_color;

pub use cache::{CacheFormat, CacheModifier, OpusCacheQuality};
pub use close_up::CloseUpModifier;
pub use denoise::{DenoiseEngine, DenoiseModifier};
pub use echo::EchoModifier;
pub use effect::AudioModifierEffect;
pub use filter::{FilterMode, FilterModifier};
pub use modifier::AudioModifier;
pub use pitch::{PitchModifier, PitchQuality};
pub use reverb::{ReverbMode, ReverbModifier};
pub use scalar::*;
pub use voice_change::{
    F0Method, PNEUMA_MAX_PITCH_OFFSET, PNEUMA_MAX_SPEED, PNEUMA_MIN_PITCH_OFFSET, PNEUMA_MIN_SPEED,
    PNEUMA_NO_MODEL, VoiceChangeModifier,
};
pub use voice_color::VoiceColorModifier;

fn default_true() -> bool {
    true
}
