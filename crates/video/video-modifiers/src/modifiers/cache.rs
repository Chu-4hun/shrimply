use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::modifier_model::{KeyframeSpan, ModifierModel};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CacheModifier {
    #[serde(default)]
    pub quality: CacheQuality,
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
pub enum CacheQuality {
    Compact,
    #[default]
    Balanced,
    High,
    Lossless,
}

impl CacheQuality {
    pub const fn qp(self) -> u32 {
        match self {
            Self::Compact => 32,
            Self::Balanced => 20,
            Self::High => 12,
            Self::Lossless => 0,
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
