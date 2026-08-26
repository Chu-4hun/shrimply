use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::modifier_model::{KeyframeSpan, ModifierModel};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CacheModifier {
    #[serde(default)]
    pub format: CacheFormat,
    #[serde(default)]
    pub opus_quality: OpusCacheQuality,
}

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
pub enum CacheFormat {
    #[default]
    Opus,
    #[strum(to_string = "FLAC")]
    Flac,
}

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
pub enum OpusCacheQuality {
    Compact,
    #[default]
    Balanced,
    High,
}

impl OpusCacheQuality {
    pub const fn bitrate(self) -> usize {
        match self {
            Self::Compact => 96_000,
            Self::Balanced => 160_000,
            Self::High => 256_000,
        }
    }
}

impl ModifierModel for CacheModifier {
    fn display_name(&self) -> &'static str {
        "Cache"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["bake", "render cache", "proxy"]
    }

    fn ensure_ids(&mut self, _seen: &mut HashSet<Uuid>) {}

    fn keyframe_span(&self) -> KeyframeSpan {
        None
    }
}
