mod math;

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    Linear,
    #[default]
    ManimSmooth,
    SineIn,
    SineOut,
    SineInOut,
    QuadIn,
    QuadOut,
    QuadInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
    QuartIn,
    QuartOut,
    QuartInOut,
    QuintIn,
    QuintOut,
    QuintInOut,
    ExpoIn,
    ExpoOut,
    ExpoInOut,
    CircIn,
    CircOut,
    CircInOut,
    BackIn,
    BackOut,
    BackInOut,
    ElasticIn,
    ElasticOut,
    ElasticInOut,
    BounceIn,
    BounceOut,
    BounceInOut,
    Jump,
}

impl Interpolation {
    pub const CONTINUOUS: [Self; 32] = [
        Self::Linear,
        Self::ManimSmooth,
        Self::SineIn,
        Self::SineOut,
        Self::SineInOut,
        Self::QuadIn,
        Self::QuadOut,
        Self::QuadInOut,
        Self::CubicIn,
        Self::CubicOut,
        Self::CubicInOut,
        Self::QuartIn,
        Self::QuartOut,
        Self::QuartInOut,
        Self::QuintIn,
        Self::QuintOut,
        Self::QuintInOut,
        Self::ExpoIn,
        Self::ExpoOut,
        Self::ExpoInOut,
        Self::CircIn,
        Self::CircOut,
        Self::CircInOut,
        Self::BackIn,
        Self::BackOut,
        Self::BackInOut,
        Self::ElasticIn,
        Self::ElasticOut,
        Self::ElasticInOut,
        Self::BounceIn,
        Self::BounceOut,
        Self::BounceInOut,
    ];

    pub const KEYFRAME: [Self; 33] = [
        Self::Linear,
        Self::ManimSmooth,
        Self::SineIn,
        Self::SineOut,
        Self::SineInOut,
        Self::QuadIn,
        Self::QuadOut,
        Self::QuadInOut,
        Self::CubicIn,
        Self::CubicOut,
        Self::CubicInOut,
        Self::QuartIn,
        Self::QuartOut,
        Self::QuartInOut,
        Self::QuintIn,
        Self::QuintOut,
        Self::QuintInOut,
        Self::ExpoIn,
        Self::ExpoOut,
        Self::ExpoInOut,
        Self::CircIn,
        Self::CircOut,
        Self::CircInOut,
        Self::BackIn,
        Self::BackOut,
        Self::BackInOut,
        Self::ElasticIn,
        Self::ElasticOut,
        Self::ElasticInOut,
        Self::BounceIn,
        Self::BounceOut,
        Self::BounceInOut,
        Self::Jump,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::ManimSmooth => "Manim Smooth",
            Self::SineIn => "Sine In",
            Self::SineOut => "Sine Out",
            Self::SineInOut => "Sine In Out",
            Self::QuadIn => "Quad In",
            Self::QuadOut => "Quad Out",
            Self::QuadInOut => "Quad In Out",
            Self::CubicIn => "Cubic In",
            Self::CubicOut => "Cubic Out",
            Self::CubicInOut => "Cubic In Out",
            Self::QuartIn => "Quart In",
            Self::QuartOut => "Quart Out",
            Self::QuartInOut => "Quart In Out",
            Self::QuintIn => "Quint In",
            Self::QuintOut => "Quint Out",
            Self::QuintInOut => "Quint In Out",
            Self::ExpoIn => "Exponential In",
            Self::ExpoOut => "Exponential Out",
            Self::ExpoInOut => "Exponential In Out",
            Self::CircIn => "Circular In",
            Self::CircOut => "Circular Out",
            Self::CircInOut => "Circular In Out",
            Self::BackIn => "Back In",
            Self::BackOut => "Back Out",
            Self::BackInOut => "Back In Out",
            Self::ElasticIn => "Elastic In",
            Self::ElasticOut => "Elastic Out",
            Self::ElasticInOut => "Elastic In Out",
            Self::BounceIn => "Bounce In",
            Self::BounceOut => "Bounce Out",
            Self::BounceInOut => "Bounce In Out",
            Self::Jump => "Jump",
        }
    }

    pub fn value(self, progress: f64) -> f64 {
        math::value(self, progress)
    }

    pub fn derivative(self, progress: f64) -> Option<f64> {
        math::derivative(self, progress)
    }

    pub fn derivative_breakpoints(self) -> &'static [f64] {
        math::derivative_breakpoints(self)
    }
}
