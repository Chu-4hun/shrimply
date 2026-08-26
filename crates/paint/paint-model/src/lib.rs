use shrimply_asset::Asset;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use shrimply_math_color::Color;
use shrimply_math_geometry::{Size2D, Transform2D};
use shrimply_timeline_value::{TimelineBool, TimelineStepVariant, TimelineValue};

pub const DEFAULT_STROKE_WIDTH: f32 = 16.0;
pub const DEFAULT_STROKE_WIDTH_SCALE: f32 = 1.0;
pub const DEFAULT_THINNING: f32 = 0.5;
pub const DEFAULT_SMOOTHING: f32 = 0.5;
pub const DEFAULT_STREAMLINE: f32 = 0.5;
pub const DEFAULT_FILL_CLOSURE_TOLERANCE: f32 = 32.0;
pub const DEFAULT_TEXTURE_REPEAT_SCALE: f32 = 1.0;

pub type PaintTransform = Transform2D<TimelineValue<Vec2>, TimelineValue<f32>>;

pub use shrimply_paint_interpolation::{
    PaintDrawing, PaintDrawingKeyframe, PaintFill, PaintPoint, PaintStroke,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaintItem {
    pub revision: u64,
    pub drawing: TimelineValue<PaintDrawing>,
    #[serde(default = "default_palette")]
    pub palette: Vec<PaintPaletteEntry>,
    pub stroke: PaintStrokeOptions,
    pub fill: PaintFillOptions,
    pub stroke_transform: PaintTransform,
}

impl PaintItem {
    pub fn new(canvas: impl Size2D) -> Self {
        Self {
            revision: 0,
            drawing: TimelineValue::new_const(PaintDrawing::default()),
            palette: default_palette(),
            stroke: PaintStrokeOptions::default(),
            fill: PaintFillOptions::default(),
            stroke_transform: PaintTransform::fill(canvas),
        }
    }
}

impl Default for PaintItem {
    fn default() -> Self {
        Self::new(Vec2::ZERO)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaintStrokeOptions {
    pub width: TimelineValue<f32>,
    pub thinning: TimelineValue<f32>,
    pub smoothing: TimelineValue<f32>,
    pub streamline: TimelineValue<f32>,
    /// A value of zero disables centerline simplification.
    pub simplification_tolerance: TimelineValue<f32>,
    /// A value of zero disables centerline subdivision.
    pub maximum_subdivision_spacing: TimelineValue<f32>,
    pub start: PaintStrokeEndOptions,
    pub end: PaintStrokeEndOptions,
}

impl Default for PaintStrokeOptions {
    fn default() -> Self {
        Self {
            width: TimelineValue::new_const(DEFAULT_STROKE_WIDTH),
            thinning: TimelineValue::new_const(DEFAULT_THINNING),
            smoothing: TimelineValue::new_const(DEFAULT_SMOOTHING),
            streamline: TimelineValue::new_const(DEFAULT_STREAMLINE),
            simplification_tolerance: TimelineValue::new_const(0.0),
            maximum_subdivision_spacing: TimelineValue::new_const(0.0),
            start: PaintStrokeEndOptions::default(),
            end: PaintStrokeEndOptions::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaintFillOptions {
    pub closure_tolerance: TimelineValue<f32>,
}

impl Default for PaintFillOptions {
    fn default() -> Self {
        Self {
            closure_tolerance: TimelineValue::new_const(DEFAULT_FILL_CLOSURE_TOLERANCE),
        }
    }
}

fn default_palette() -> Vec<PaintPaletteEntry> {
    [Color::<u8>::BLACK, Color::<u8>::WHITE]
        .into_iter()
        .map(|color| PaintPaletteEntry {
            color: TimelineValue::new_const(color),
            texture: None,
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaintPaletteEntry {
    pub color: TimelineValue<Color<u8>>,
    #[serde(default)]
    pub texture: Option<PaintTextureOptions>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaintTextureOptions {
    pub image_path: Asset,
    pub repeat_scale: TimelineValue<f32>,
    pub rotation_degrees: TimelineValue<f32>,
}

impl PaintTextureOptions {
    pub fn new(image_path: impl Into<Asset>) -> Self {
        Self {
            image_path: image_path.into(),
            repeat_scale: TimelineValue::new_const(DEFAULT_TEXTURE_REPEAT_SCALE),
            rotation_degrees: TimelineValue::new_const(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintTaper {
    #[default]
    None,
    Full,
    Distance,
}

shrimply_timeline_value::timeline_step_type!(
    PaintTaper,
    PaintTaper::None,
    &[
        TimelineStepVariant {
            value: PaintTaper::None,
            key: "none",
            label: "None",
            icon: None,
        },
        TimelineStepVariant {
            value: PaintTaper::Full,
            key: "full",
            label: "Full",
            icon: None,
        },
        TimelineStepVariant {
            value: PaintTaper::Distance,
            key: "distance",
            label: "Distance",
            icon: None,
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaintStrokeEndOptions {
    pub cap: TimelineValue<TimelineBool>,
    pub taper: TimelineValue<PaintTaper>,
    pub taper_distance: TimelineValue<f32>,
}

impl Default for PaintStrokeEndOptions {
    fn default() -> Self {
        Self {
            cap: TimelineValue::new_const(TimelineBool::True),
            taper: TimelineValue::new_const(PaintTaper::None),
            taper_distance: TimelineValue::new_const(DEFAULT_STROKE_WIDTH),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPaintStrokeOptions {
    pub width: f32,
    pub thinning: f32,
    pub smoothing: f32,
    pub streamline: f32,
    pub simplification_tolerance: f32,
    pub maximum_subdivision_spacing: f32,
    pub start: ResolvedPaintStrokeEndOptions,
    pub end: ResolvedPaintStrokeEndOptions,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPaintStrokeEndOptions {
    pub cap: bool,
    pub taper: PaintTaper,
    pub taper_distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPaintFillOptions {
    pub closure_tolerance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPaintTextureOptions {
    pub repeat_scale: f32,
    pub rotation_degrees: f32,
}
