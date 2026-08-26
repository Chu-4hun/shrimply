use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use uuid::Uuid;

use super::{KeyframeSpan, ModifierModel};

pub const MAX_SPECKLE_SIZE: u32 = 16;
pub const MIN_COLOR_PRECISION: u32 = 1;
pub const MAX_COLOR_PRECISION: u32 = 8;
pub const MAX_GRADIENT_STEP: u32 = 255;
pub const MAX_BINARY_THRESHOLD: u32 = 255;
pub const MAX_ANGLE_DEGREES: u32 = 180;
pub const MIN_SEGMENT_LENGTH: f32 = 3.5;
pub const MAX_SEGMENT_LENGTH: f32 = 10.0;
pub const MAX_ITERATIONS: u32 = 32;
pub const MAX_PATH_PRECISION: u32 = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum VectorizePreset {
    Custom,
    #[default]
    Poster,
    Photo,
    #[strum(to_string = "Black & White")]
    BlackAndWhite,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum VectorizeColorMode {
    #[default]
    Color,
    #[strum(to_string = "Black & White")]
    BlackAndWhite,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum VectorizeHierarchy {
    #[default]
    Stacked,
    Cutout,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum VectorizePathMode {
    Pixel,
    Polygon,
    #[default]
    Spline,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VectorizeModifier {
    pub preset: VectorizePreset,
    pub color_mode: VectorizeColorMode,
    pub hierarchy: VectorizeHierarchy,
    pub path_mode: VectorizePathMode,
    pub speckle_size: u32,
    pub color_precision: u32,
    pub gradient_step: u32,
    pub binary_threshold: u32,
    pub corner_threshold_degrees: u32,
    pub segment_length: f32,
    pub max_iterations: u32,
    pub splice_threshold_degrees: u32,
    pub path_precision: u32,
}

impl Default for VectorizeModifier {
    fn default() -> Self {
        Self::from_preset(VectorizePreset::Poster)
    }
}

impl VectorizeModifier {
    pub fn from_preset(preset: VectorizePreset) -> Self {
        let mut modifier = match preset {
            VectorizePreset::Custom | VectorizePreset::Poster => Self {
                preset: VectorizePreset::Poster,
                color_mode: VectorizeColorMode::Color,
                hierarchy: VectorizeHierarchy::Stacked,
                path_mode: VectorizePathMode::Spline,
                speckle_size: 4,
                color_precision: 8,
                gradient_step: 16,
                binary_threshold: 128,
                corner_threshold_degrees: 60,
                segment_length: 4.0,
                max_iterations: 10,
                splice_threshold_degrees: 45,
                path_precision: 2,
            },
            VectorizePreset::Photo => Self {
                preset,
                speckle_size: 10,
                gradient_step: 48,
                corner_threshold_degrees: 180,
                ..Self::from_preset(VectorizePreset::Poster)
            },
            VectorizePreset::BlackAndWhite => Self {
                preset,
                color_mode: VectorizeColorMode::BlackAndWhite,
                color_precision: 6,
                ..Self::from_preset(VectorizePreset::Poster)
            },
        };
        modifier.preset = preset;
        modifier
    }
}

impl ModifierModel for VectorizeModifier {
    fn display_name(&self) -> &'static str {
        "Vectorize"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["trace", "image", "svg", "path", "bezier"]
    }

    fn ensure_ids(&mut self, _seen: &mut HashSet<Uuid>) {}

    fn keyframe_span(&self) -> KeyframeSpan {
        None
    }
}
