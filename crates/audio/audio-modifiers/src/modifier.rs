use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AudioModifierEffect, default_true};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioModifier {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub effect: AudioModifierEffect,
}

impl AudioModifier {
    pub fn new(effect: AudioModifierEffect) -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: true,
            effect,
        }
    }
}
