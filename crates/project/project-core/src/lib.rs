use glam::Vec2;
use serde::{Deserialize, Serialize};
use shrimply_math_core::Fraction;
use shrimply_math_geometry::Size2D;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioClipTransitionCurve {
    #[default]
    EqualPower,
    Linear,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransitionSide {
    Intro,
    Outro,
}

impl Size2D for CanvasSize {
    fn size_2d(&self) -> Vec2 {
        Vec2::new(self.width.max(1) as f32, self.height.max(1) as f32)
    }
}

#[derive(Clone, Copy)]
pub struct FrameRate {
    pub value: Fraction,
    pub label: &'static str,
}

impl FrameRate {
    const fn new(value: Fraction, label: &'static str) -> Self {
        Self { value, label }
    }
}

pub const COMMON_FRAME_RATES: &[FrameRate] = &[
    FrameRate::new(Fraction::new_raw(1, 1), "1"),
    FrameRate::new(Fraction::new_raw(5, 1), "5"),
    FrameRate::new(Fraction::new_raw(10, 1), "10"),
    FrameRate::new(Fraction::new_raw(15, 1), "15"),
    FrameRate::new(Fraction::new_raw(24_000, 1_001), "23.976"),
    FrameRate::new(Fraction::new_raw(24, 1), "24"),
    FrameRate::new(Fraction::new_raw(25, 1), "25"),
    FrameRate::new(Fraction::new_raw(30_000, 1_001), "29.97"),
    FrameRate::new(Fraction::new_raw(30, 1), "30"),
    FrameRate::new(Fraction::new_raw(48, 1), "48"),
    FrameRate::new(Fraction::new_raw(50, 1), "50"),
    FrameRate::new(Fraction::new_raw(60_000, 1_001), "59.94"),
    FrameRate::new(Fraction::new_raw(60, 1), "60"),
    FrameRate::new(Fraction::new_raw(120, 1), "120"),
];

#[derive(Clone, Copy)]
pub struct ProjectPreset {
    pub label: &'static str,
    pub canvas_size: CanvasSize,
    pub fps: Fraction,
}

impl ProjectPreset {
    const fn new(label: &'static str, width: u32, height: u32, fps: u64) -> Self {
        Self {
            label,
            canvas_size: CanvasSize { width, height },
            fps: Fraction::new_raw(fps, 1),
        }
    }
}

pub const DEFAULT_CANVAS_SIZE: CanvasSize = CanvasSize {
    width: 1920,
    height: 1080,
};
pub const DEFAULT_PROJECT_FPS: Fraction = Fraction::new_raw(30, 1);

pub const PROJECT_PRESETS: &[ProjectPreset] = &[
    ProjectPreset::new("720p 30 FPS", 1280, 720, 30),
    ProjectPreset::new("720p 60 FPS", 1280, 720, 60),
    ProjectPreset::new("1080p 24 FPS", 1920, 1080, 24),
    ProjectPreset::new("1080p 30 FPS", 1920, 1080, 30),
    ProjectPreset::new("1080p 60 FPS", 1920, 1080, 60),
    ProjectPreset::new("1440p 30 FPS", 2560, 1440, 30),
    ProjectPreset::new("1440p 60 FPS", 2560, 1440, 60),
    ProjectPreset::new("4K 30 FPS", 3840, 2160, 30),
    ProjectPreset::new("4K 60 FPS", 3840, 2160, 60),
];
