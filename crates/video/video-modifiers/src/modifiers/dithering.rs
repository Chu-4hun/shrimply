use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use shrimply_core::{Color, timeline_value::*};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DitheringPattern {
    Bayer2x2,
    #[default]
    Bayer4x4,
    Bayer8x8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DitheringColorMode {
    #[default]
    Color,
    Grayscale,
    Palette,
}

shrimply_core::timeline_value::timeline_step_type!(
    DitheringPattern,
    DitheringPattern::Bayer4x4,
    &[
        TimelineStepVariant {
            value: DitheringPattern::Bayer2x2,
            key: "bayer_2x2",
            label: "Bayer 2x2",
            icon: None
        },
        TimelineStepVariant {
            value: DitheringPattern::Bayer4x4,
            key: "bayer_4x4",
            label: "Bayer 4x4",
            icon: None
        },
        TimelineStepVariant {
            value: DitheringPattern::Bayer8x8,
            key: "bayer_8x8",
            label: "Bayer 8x8",
            icon: None
        },
    ]
);
shrimply_core::timeline_value::timeline_step_type!(
    DitheringColorMode,
    DitheringColorMode::Color,
    &[
        TimelineStepVariant {
            value: DitheringColorMode::Color,
            key: "color",
            label: "Color",
            icon: None
        },
        TimelineStepVariant {
            value: DitheringColorMode::Grayscale,
            key: "grayscale",
            label: "Grayscale",
            icon: None
        },
        TimelineStepVariant {
            value: DitheringColorMode::Palette,
            key: "palette",
            label: "Palette",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DitheringModifier {
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub pattern: TimelineValue<DitheringPattern>,
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub color_mode: TimelineValue<DitheringColorMode>,
    pub levels: TimelineValue<f32>,
    pub amount: TimelineValue<f32>,
    pub palette: Vec<TimelineValue<shrimply_core::Color<u8>>>,
}

impl Default for DitheringModifier {
    fn default() -> Self {
        Self {
            pattern: TimelineValue::new_const(DitheringPattern::Bayer4x4),
            color_mode: TimelineValue::new_const(DitheringColorMode::Color),
            levels: TimelineValue::<f32>::new_const(4.0),
            amount: TimelineValue::<f32>::new_const(1.0),
            palette: vec![
                TimelineValue::<Color<u8>>::new_const(Color::<u8>::BLACK),
                TimelineValue::<Color<u8>>::new_const(Color::<u8>::WHITE),
            ],
        }
    }
}

impl ModifierModel for DitheringModifier {
    fn display_name(&self) -> &'static str {
        "Dithering"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["dither", "ordered noise", "error diffusion", "retro"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.pattern, seen);
        ensure_timeline_value_ids(&mut self.color_mode, seen);
        ensure_timeline_value_ids(&mut self.levels, seen);
        ensure_timeline_value_ids(&mut self.amount, seen);
        for color in &mut self.palette {
            ensure_timeline_value_ids(color, seen);
        }
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.pattern),
            timeline_value_span(&self.color_mode),
            timeline_value_span(&self.levels),
            timeline_value_span(&self.amount),
            combine(self.palette.iter().map(timeline_value_span)),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.levels, &self.amount]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.levels, &mut self.amount]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        self.palette.iter_mut().find(|value| value.id == id)
    }
}
