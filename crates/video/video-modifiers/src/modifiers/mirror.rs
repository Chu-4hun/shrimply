use super::{KeyframeSpan, ModifierModel};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MirrorModifier {
    pub horizontal: bool,
    pub vertical: bool,
}

impl Default for MirrorModifier {
    fn default() -> Self {
        Self {
            horizontal: true,
            vertical: false,
        }
    }
}

impl ModifierModel for MirrorModifier {
    fn display_name(&self) -> &'static str {
        "Mirror"
    }

    fn ensure_ids(&mut self, _seen: &mut HashSet<Uuid>) {}

    fn keyframe_span(&self) -> KeyframeSpan {
        None
    }
}
