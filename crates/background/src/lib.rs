use glam::Vec2;
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    Color,
    timeline_value::{
        TimelineBool, TimelineStepVariant, TimelineValue, deserialize_timeline_value,
    },
};

pub const DEFAULT_GRID_SPACING: f32 = 64.0;
pub const DEFAULT_GRID_LINE_WIDTH: f32 = 2.0;
pub const DEFAULT_CENTERED_LINE_COUNT: u32 = 96;
pub const DEFAULT_CENTERED_LINE_WIDTH: f32 = 3.0;
pub const DEFAULT_CENTERED_LINE_LENGTH: f32 = 512.0;
pub const DEFAULT_CENTERED_LINE_OFFSET: f32 = 128.0;
pub const DEFAULT_CENTERED_LINE_OFFSET_RANDOMNESS: f32 = 64.0;
pub const DEFAULT_CENTERED_LINE_FADE_LENGTH: f32 = 192.0;
pub const DEFAULT_PERLIN_SCALE: f32 = 256.0;
pub const DEFAULT_CHECKER_SIZE: f32 = 64.0;
pub const DEFAULT_VORONOI_SIZE: f32 = 96.0;

fn find_timeline_mut<T, const N: usize>(
    values: [&mut TimelineValue<T>; N],
    id: uuid::Uuid,
) -> Option<&mut TimelineValue<T>>
where
    T: shrimply_core::timeline_value::TimelineValueType,
{
    values.into_iter().find(|value| value.id == id)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Background {
    #[serde(default)]
    pub generator: BackgroundGenerator,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, strum::Display, strum::EnumIter)]
pub enum BackgroundKind {
    #[default]
    #[strum(to_string = "Solid Color")]
    SolidColor,
    #[strum(to_string = "Color / Gradient")]
    ColorGradient,
    Grid,
    #[strum(to_string = "White Noise")]
    WhiteNoise,
    #[strum(to_string = "Perlin Noise")]
    PerlinNoise,
    #[strum(to_string = "Centered Lines")]
    CenteredLines,
    Rainbow,
    Checkerboard,
    Voronoi,
    #[strum(to_string = "Test Pattern")]
    TestPattern,
}

impl BackgroundKind {
    pub fn generator(self) -> BackgroundGenerator {
        match self {
            Self::SolidColor => BackgroundGenerator::SolidColor(Box::default()),
            Self::ColorGradient => BackgroundGenerator::ColorGradient(Box::default()),
            Self::Grid => BackgroundGenerator::Grid(Box::default()),
            Self::WhiteNoise => BackgroundGenerator::WhiteNoise(Box::default()),
            Self::PerlinNoise => BackgroundGenerator::PerlinNoise(Box::default()),
            Self::CenteredLines => BackgroundGenerator::CenteredLines(Box::default()),
            Self::Rainbow => BackgroundGenerator::Rainbow(Box::default()),
            Self::Checkerboard => BackgroundGenerator::Checkerboard(Box::default()),
            Self::Voronoi => BackgroundGenerator::Voronoi(Box::default()),
            Self::TestPattern => BackgroundGenerator::TestPattern,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BackgroundGenerator {
    SolidColor(Box<SolidColor>),
    ColorGradient(Box<ColorGradient>),
    Grid(Box<Grid>),
    WhiteNoise(Box<WhiteNoise>),
    PerlinNoise(Box<PerlinNoise>),
    CenteredLines(Box<CenteredLines>),
    Rainbow(Box<Rainbow>),
    Checkerboard(Box<Checkerboard>),
    Voronoi(Box<Voronoi>),
    TestPattern,
}

impl Default for BackgroundGenerator {
    fn default() -> Self {
        Self::SolidColor(Box::default())
    }
}

impl BackgroundGenerator {
    pub fn kind(&self) -> BackgroundKind {
        match self {
            Self::SolidColor(_) => BackgroundKind::SolidColor,
            Self::ColorGradient(_) => BackgroundKind::ColorGradient,
            Self::Grid(_) => BackgroundKind::Grid,
            Self::WhiteNoise(_) => BackgroundKind::WhiteNoise,
            Self::PerlinNoise(_) => BackgroundKind::PerlinNoise,
            Self::CenteredLines(_) => BackgroundKind::CenteredLines,
            Self::Rainbow(_) => BackgroundKind::Rainbow,
            Self::Checkerboard(_) => BackgroundKind::Checkerboard,
            Self::Voronoi(_) => BackgroundKind::Voronoi,
            Self::TestPattern => BackgroundKind::TestPattern,
        }
    }

    pub fn number(&self, id: uuid::Uuid) -> Option<&TimelineValue<f32>> {
        let values: &[&TimelineValue<f32>] = match self {
            Self::SolidColor(_) => &[],
            Self::ColorGradient(v) => &[&v.angle_degrees, &v.scale, &v.cycle_position],
            Self::Grid(v) => &[
                &v.rotation_degrees,
                &v.dash_length,
                &v.dash_gap,
                &v.dash_position,
                &v.wobble_amount,
                &v.wobble_scale,
                &v.wobble_position,
            ],
            Self::WhiteNoise(v) => &[&v.brightness, &v.contrast, &v.refresh_interval],
            Self::PerlinNoise(v) => &[
                &v.scale,
                &v.lacunarity,
                &v.persistence,
                &v.contrast,
                &v.evolution,
                &v.warp_amount,
                &v.warp_scale,
            ],
            Self::CenteredLines(v) => &[
                &v.rotation_degrees,
                &v.line_width,
                &v.line_width_randomness,
                &v.line_length,
                &v.line_length_randomness,
                &v.line_offset,
                &v.line_offset_randomness,
                &v.angular_randomness,
                &v.fade_length,
            ],
            Self::Rainbow(v) => &[
                &v.angle_degrees,
                &v.scale,
                &v.saturation,
                &v.brightness,
                &v.alpha,
                &v.hue_position,
            ],
            Self::Checkerboard(v) => &[&v.edge_softness, &v.rotation_degrees],
            Self::Voronoi(v) => &[
                &v.cell_size,
                &v.jitter,
                &v.edge_width,
                &v.motion_amount,
                &v.motion_position,
            ],
            Self::TestPattern => &[],
        };
        values.iter().copied().find(|value| value.id == id)
    }

    pub fn number_mut(&mut self, id: uuid::Uuid) -> Option<&mut TimelineValue<f32>> {
        match self {
            Self::SolidColor(_) => None,
            Self::ColorGradient(v) => find_timeline_mut(
                [&mut v.angle_degrees, &mut v.scale, &mut v.cycle_position],
                id,
            ),
            Self::Grid(v) => find_timeline_mut(
                [
                    &mut v.rotation_degrees,
                    &mut v.dash_length,
                    &mut v.dash_gap,
                    &mut v.dash_position,
                    &mut v.wobble_amount,
                    &mut v.wobble_scale,
                    &mut v.wobble_position,
                ],
                id,
            ),
            Self::WhiteNoise(v) => find_timeline_mut(
                [&mut v.brightness, &mut v.contrast, &mut v.refresh_interval],
                id,
            ),
            Self::PerlinNoise(v) => find_timeline_mut(
                [
                    &mut v.scale,
                    &mut v.lacunarity,
                    &mut v.persistence,
                    &mut v.contrast,
                    &mut v.evolution,
                    &mut v.warp_amount,
                    &mut v.warp_scale,
                ],
                id,
            ),
            Self::CenteredLines(v) => find_timeline_mut(
                [
                    &mut v.rotation_degrees,
                    &mut v.line_width,
                    &mut v.line_width_randomness,
                    &mut v.line_length,
                    &mut v.line_length_randomness,
                    &mut v.line_offset,
                    &mut v.line_offset_randomness,
                    &mut v.angular_randomness,
                    &mut v.fade_length,
                ],
                id,
            ),
            Self::Rainbow(v) => find_timeline_mut(
                [
                    &mut v.angle_degrees,
                    &mut v.scale,
                    &mut v.saturation,
                    &mut v.brightness,
                    &mut v.alpha,
                    &mut v.hue_position,
                ],
                id,
            ),
            Self::Checkerboard(v) => {
                find_timeline_mut([&mut v.edge_softness, &mut v.rotation_degrees], id)
            }
            Self::Voronoi(v) => find_timeline_mut(
                [
                    &mut v.cell_size,
                    &mut v.jitter,
                    &mut v.edge_width,
                    &mut v.motion_amount,
                    &mut v.motion_position,
                ],
                id,
            ),
            Self::TestPattern => None,
        }
    }

    pub fn number2(&self, id: uuid::Uuid) -> Option<&TimelineValue<Vec2>> {
        let values: &[&TimelineValue<Vec2>] = match self {
            Self::SolidColor(_) => &[],
            Self::ColorGradient(v) => &[&v.center, &v.position],
            Self::Grid(v) => &[
                &v.spacing,
                &v.line_width,
                &v.position,
                &v.middle_padding,
                &v.padding_randomness,
            ],
            Self::PerlinNoise(v) => &[&v.position],
            Self::Rainbow(v) => &[&v.center, &v.position],
            Self::Checkerboard(v) => &[&v.cell_size, &v.position],
            Self::Voronoi(v) => &[&v.position],
            Self::CenteredLines(v) => &[&v.center],
            Self::WhiteNoise(_) | Self::TestPattern => &[],
        };
        values.iter().copied().find(|value| value.id == id)
    }

    pub fn number2_mut(&mut self, id: uuid::Uuid) -> Option<&mut TimelineValue<Vec2>> {
        match self {
            Self::SolidColor(_) => None,
            Self::ColorGradient(v) => find_timeline_mut([&mut v.center, &mut v.position], id),
            Self::Grid(v) => find_timeline_mut(
                [
                    &mut v.spacing,
                    &mut v.line_width,
                    &mut v.position,
                    &mut v.middle_padding,
                    &mut v.padding_randomness,
                ],
                id,
            ),
            Self::PerlinNoise(v) => find_timeline_mut([&mut v.position], id),
            Self::Rainbow(v) => find_timeline_mut([&mut v.center, &mut v.position], id),
            Self::Checkerboard(v) => find_timeline_mut([&mut v.cell_size, &mut v.position], id),
            Self::Voronoi(v) => find_timeline_mut([&mut v.position], id),
            Self::CenteredLines(v) => find_timeline_mut([&mut v.center], id),
            Self::WhiteNoise(_) | Self::TestPattern => None,
        }
    }

    pub fn color(&self, id: uuid::Uuid) -> Option<&TimelineValue<Color<u8>>> {
        let values: &[&TimelineValue<Color<u8>>] = match self {
            Self::SolidColor(v) => &[&v.color],
            Self::ColorGradient(v) => &[&v.color_a, &v.color_b],
            Self::Grid(v) => &[&v.background_color, &v.horizontal_color, &v.vertical_color],
            Self::WhiteNoise(v) => &[&v.color_a, &v.color_b],
            Self::PerlinNoise(v) => &[&v.color_a, &v.color_b],
            Self::Checkerboard(v) => &[&v.color_a, &v.color_b],
            Self::Voronoi(v) => &[&v.color_a, &v.color_b, &v.edge_color],
            Self::CenteredLines(v) => &[&v.background_color, &v.line_color],
            Self::Rainbow(_) | Self::TestPattern => &[],
        };
        values.iter().copied().find(|value| value.id == id)
    }

    pub fn color_mut(&mut self, id: uuid::Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        match self {
            Self::SolidColor(v) => find_timeline_mut([&mut v.color], id),
            Self::ColorGradient(v) => find_timeline_mut([&mut v.color_a, &mut v.color_b], id),
            Self::Grid(v) => find_timeline_mut(
                [
                    &mut v.background_color,
                    &mut v.horizontal_color,
                    &mut v.vertical_color,
                ],
                id,
            ),
            Self::WhiteNoise(v) => find_timeline_mut([&mut v.color_a, &mut v.color_b], id),
            Self::PerlinNoise(v) => find_timeline_mut([&mut v.color_a, &mut v.color_b], id),
            Self::Checkerboard(v) => find_timeline_mut([&mut v.color_a, &mut v.color_b], id),
            Self::Voronoi(v) => {
                find_timeline_mut([&mut v.color_a, &mut v.color_b, &mut v.edge_color], id)
            }
            Self::CenteredLines(v) => {
                find_timeline_mut([&mut v.background_color, &mut v.line_color], id)
            }
            Self::Rainbow(_) | Self::TestPattern => None,
        }
    }

    pub fn integer(&self, id: uuid::Uuid) -> Option<&TimelineValue<u32>> {
        let values: &[&TimelineValue<u32>] = match self {
            Self::Grid(v) => &[&v.seed],
            Self::WhiteNoise(v) => &[&v.pixel_size, &v.seed],
            Self::PerlinNoise(v) => &[&v.octaves, &v.seed],
            Self::Rainbow(v) => &[&v.band_count],
            Self::Voronoi(v) => &[&v.seed],
            Self::CenteredLines(v) => &[&v.line_count, &v.seed],
            Self::SolidColor(_)
            | Self::ColorGradient(_)
            | Self::Checkerboard(_)
            | Self::TestPattern => &[],
        };
        values.iter().copied().find(|value| value.id == id)
    }

    pub fn integer_mut(&mut self, id: uuid::Uuid) -> Option<&mut TimelineValue<u32>> {
        match self {
            Self::Grid(v) => find_timeline_mut([&mut v.seed], id),
            Self::WhiteNoise(v) => find_timeline_mut([&mut v.pixel_size, &mut v.seed], id),
            Self::PerlinNoise(v) => find_timeline_mut([&mut v.octaves, &mut v.seed], id),
            Self::Rainbow(v) => find_timeline_mut([&mut v.band_count], id),
            Self::Voronoi(v) => find_timeline_mut([&mut v.seed], id),
            Self::CenteredLines(v) => find_timeline_mut([&mut v.line_count, &mut v.seed], id),
            Self::SolidColor(_)
            | Self::ColorGradient(_)
            | Self::Checkerboard(_)
            | Self::TestPattern => None,
        }
    }

    pub fn boolean(&self, id: uuid::Uuid) -> Option<&TimelineValue<TimelineBool>> {
        match self {
            Self::WhiteNoise(v) if v.animated.id == id => Some(&v.animated),
            _ => None,
        }
    }

    pub fn boolean_mut(&mut self, id: uuid::Uuid) -> Option<&mut TimelineValue<TimelineBool>> {
        match self {
            Self::WhiteNoise(v) if v.animated.id == id => Some(&mut v.animated),
            _ => None,
        }
    }

    pub fn ensure_ids(&mut self, seen: &mut HashSet<uuid::Uuid>) {
        macro_rules! ensure {
            ($($value:expr),* $(,)?) => {{
                $(shrimply_core::modifier_model::ensure_timeline_value_ids($value, seen);)*
            }};
        }
        match self {
            Self::SolidColor(v) => ensure!(&mut v.color),
            Self::ColorGradient(v) => ensure!(
                &mut v.mode,
                &mut v.curve,
                &mut v.color_a,
                &mut v.color_b,
                &mut v.center,
                &mut v.angle_degrees,
                &mut v.scale,
                &mut v.position,
                &mut v.cycle_position
            ),
            Self::Grid(v) => ensure!(
                &mut v.line_style,
                &mut v.background_color,
                &mut v.horizontal_color,
                &mut v.vertical_color,
                &mut v.spacing,
                &mut v.line_width,
                &mut v.position,
                &mut v.rotation_degrees,
                &mut v.dash_length,
                &mut v.dash_gap,
                &mut v.dash_position,
                &mut v.wobble_amount,
                &mut v.wobble_scale,
                &mut v.wobble_position,
                &mut v.middle_padding,
                &mut v.padding_randomness,
                &mut v.seed
            ),
            Self::WhiteNoise(v) => ensure!(
                &mut v.distribution,
                &mut v.color_mode,
                &mut v.color_a,
                &mut v.color_b,
                &mut v.pixel_size,
                &mut v.brightness,
                &mut v.contrast,
                &mut v.animated,
                &mut v.refresh_interval,
                &mut v.seed
            ),
            Self::PerlinNoise(v) => ensure!(
                &mut v.mode,
                &mut v.color_a,
                &mut v.color_b,
                &mut v.scale,
                &mut v.octaves,
                &mut v.lacunarity,
                &mut v.persistence,
                &mut v.contrast,
                &mut v.position,
                &mut v.evolution,
                &mut v.warp_amount,
                &mut v.warp_scale,
                &mut v.seed
            ),
            Self::CenteredLines(v) => ensure!(
                &mut v.background_color,
                &mut v.line_color,
                &mut v.center,
                &mut v.rotation_degrees,
                &mut v.line_count,
                &mut v.line_width,
                &mut v.line_width_randomness,
                &mut v.line_length,
                &mut v.line_length_randomness,
                &mut v.line_offset,
                &mut v.line_offset_randomness,
                &mut v.angular_randomness,
                &mut v.fade_length,
                &mut v.seed
            ),
            Self::Rainbow(v) => ensure!(
                &mut v.fill,
                &mut v.bands,
                &mut v.band_count,
                &mut v.center,
                &mut v.angle_degrees,
                &mut v.scale,
                &mut v.saturation,
                &mut v.brightness,
                &mut v.alpha,
                &mut v.position,
                &mut v.hue_position
            ),
            Self::Checkerboard(v) => ensure!(
                &mut v.color_a,
                &mut v.color_b,
                &mut v.cell_size,
                &mut v.edge_softness,
                &mut v.position,
                &mut v.rotation_degrees,
            ),
            Self::Voronoi(v) => ensure!(
                &mut v.fill,
                &mut v.metric,
                &mut v.color_a,
                &mut v.color_b,
                &mut v.edge_color,
                &mut v.cell_size,
                &mut v.jitter,
                &mut v.edge_width,
                &mut v.position,
                &mut v.motion_amount,
                &mut v.motion_position,
                &mut v.seed
            ),
            Self::TestPattern => {}
        }
    }

    pub fn keyframe_span(&self) -> Option<(shrimply_core::Time, shrimply_core::Time)> {
        macro_rules! span {
            ($($value:expr),* $(,)?) => { shrimply_core::modifier_model::combine([$((shrimply_core::modifier_model::timeline_value_span($value))),*]) };
        }
        match self {
            Self::SolidColor(v) => span!(&v.color),
            Self::ColorGradient(v) => span!(
                &v.mode,
                &v.curve,
                &v.color_a,
                &v.color_b,
                &v.center,
                &v.angle_degrees,
                &v.scale,
                &v.position,
                &v.cycle_position
            ),
            Self::Grid(v) => span!(
                &v.line_style,
                &v.background_color,
                &v.horizontal_color,
                &v.vertical_color,
                &v.spacing,
                &v.line_width,
                &v.position,
                &v.rotation_degrees,
                &v.dash_length,
                &v.dash_gap,
                &v.dash_position,
                &v.wobble_amount,
                &v.wobble_scale,
                &v.wobble_position,
                &v.middle_padding,
                &v.padding_randomness,
                &v.seed
            ),
            Self::WhiteNoise(v) => span!(
                &v.distribution,
                &v.color_mode,
                &v.color_a,
                &v.color_b,
                &v.pixel_size,
                &v.brightness,
                &v.contrast,
                &v.animated,
                &v.refresh_interval,
                &v.seed
            ),
            Self::PerlinNoise(v) => span!(
                &v.mode,
                &v.color_a,
                &v.color_b,
                &v.scale,
                &v.octaves,
                &v.lacunarity,
                &v.persistence,
                &v.contrast,
                &v.position,
                &v.evolution,
                &v.warp_amount,
                &v.warp_scale,
                &v.seed
            ),
            Self::CenteredLines(v) => span!(
                &v.background_color,
                &v.line_color,
                &v.center,
                &v.rotation_degrees,
                &v.line_count,
                &v.line_width,
                &v.line_width_randomness,
                &v.line_length,
                &v.line_length_randomness,
                &v.line_offset,
                &v.line_offset_randomness,
                &v.angular_randomness,
                &v.fade_length,
                &v.seed
            ),
            Self::Rainbow(v) => span!(
                &v.fill,
                &v.bands,
                &v.band_count,
                &v.center,
                &v.angle_degrees,
                &v.scale,
                &v.saturation,
                &v.brightness,
                &v.alpha,
                &v.position,
                &v.hue_position
            ),
            Self::Checkerboard(v) => span!(
                &v.color_a,
                &v.color_b,
                &v.cell_size,
                &v.edge_softness,
                &v.position,
                &v.rotation_degrees,
            ),
            Self::Voronoi(v) => span!(
                &v.fill,
                &v.metric,
                &v.color_a,
                &v.color_b,
                &v.edge_color,
                &v.cell_size,
                &v.jitter,
                &v.edge_width,
                &v.position,
                &v.motion_amount,
                &v.motion_position,
                &v.seed
            ),
            Self::TestPattern => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolidColor {
    pub color: TimelineValue<Color<u8>>,
}

impl Default for SolidColor {
    fn default() -> Self {
        Self {
            color: TimelineValue::new_const(Color::<u8>::BLACK),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientMode {
    Solid,
    #[default]
    Linear,
    Radial,
    Conic,
}

shrimply_core::timeline_value::timeline_step_type!(
    GradientMode,
    GradientMode::Linear,
    &[
        TimelineStepVariant {
            value: GradientMode::Solid,
            key: "solid",
            label: "Solid",
            icon: None
        },
        TimelineStepVariant {
            value: GradientMode::Linear,
            key: "linear",
            label: "Linear",
            icon: None
        },
        TimelineStepVariant {
            value: GradientMode::Radial,
            key: "radial",
            label: "Radial",
            icon: None
        },
        TimelineStepVariant {
            value: GradientMode::Conic,
            key: "conic",
            label: "Conic",
            icon: None
        },
    ]
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    Step,
    Linear,
    #[default]
    Smooth,
    Smoother,
}

shrimply_core::timeline_value::timeline_step_type!(
    Curve,
    Curve::Smooth,
    &[
        TimelineStepVariant {
            value: Curve::Step,
            key: "step",
            label: "Step",
            icon: None
        },
        TimelineStepVariant {
            value: Curve::Linear,
            key: "linear",
            label: "Linear",
            icon: None
        },
        TimelineStepVariant {
            value: Curve::Smooth,
            key: "smooth",
            label: "Smooth",
            icon: None
        },
        TimelineStepVariant {
            value: Curve::Smoother,
            key: "smoother",
            label: "Smoother",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorGradient {
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub mode: TimelineValue<GradientMode>,
    pub color_a: TimelineValue<Color<u8>>,
    pub color_b: TimelineValue<Color<u8>>,
    pub center: TimelineValue<Vec2>,
    pub angle_degrees: TimelineValue<f32>,
    pub scale: TimelineValue<f32>,
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub curve: TimelineValue<Curve>,
    #[serde(alias = "drift")]
    pub position: TimelineValue<Vec2>,
    #[serde(alias = "cycle_speed")]
    pub cycle_position: TimelineValue<f32>,
}

impl Default for ColorGradient {
    fn default() -> Self {
        Self {
            mode: TimelineValue::new_const(GradientMode::Linear),
            color_a: TimelineValue::new_const(Color::<u8>::BLACK),
            color_b: TimelineValue::new_const(Color::<u8>::WHITE),
            center: TimelineValue::new_const(Vec2::splat(0.5)),
            angle_degrees: TimelineValue::new_const(0.0),
            scale: TimelineValue::new_const(1.0),
            curve: TimelineValue::new_const(Curve::Linear),
            position: TimelineValue::new_const(Vec2::ZERO),
            cycle_position: TimelineValue::new_const(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridLineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

shrimply_core::timeline_value::timeline_step_type!(
    GridLineStyle,
    GridLineStyle::Solid,
    &[
        TimelineStepVariant {
            value: GridLineStyle::Solid,
            key: "solid",
            label: "Solid",
            icon: None
        },
        TimelineStepVariant {
            value: GridLineStyle::Dashed,
            key: "dashed",
            label: "Dashed",
            icon: None
        },
        TimelineStepVariant {
            value: GridLineStyle::Dotted,
            key: "dotted",
            label: "Dotted",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grid {
    pub background_color: TimelineValue<Color<u8>>,
    pub horizontal_color: TimelineValue<Color<u8>>,
    pub vertical_color: TimelineValue<Color<u8>>,
    pub spacing: TimelineValue<Vec2>,
    pub line_width: TimelineValue<Vec2>,
    #[serde(alias = "speed")]
    pub position: TimelineValue<Vec2>,
    pub rotation_degrees: TimelineValue<f32>,
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub line_style: TimelineValue<GridLineStyle>,
    pub dash_length: TimelineValue<f32>,
    pub dash_gap: TimelineValue<f32>,
    #[serde(alias = "dash_speed")]
    pub dash_position: TimelineValue<f32>,
    pub wobble_amount: TimelineValue<f32>,
    pub wobble_scale: TimelineValue<f32>,
    #[serde(alias = "wobble_speed")]
    pub wobble_position: TimelineValue<f32>,
    #[serde(default)]
    pub middle_padding: TimelineValue<Vec2>,
    #[serde(default)]
    pub padding_randomness: TimelineValue<Vec2>,
    pub seed: TimelineValue<u32>,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            background_color: TimelineValue::new_const(Color::<u8>::BLACK),
            horizontal_color: TimelineValue::new_const(Color::<u8>::from_rgba(255, 255, 255, 96)),
            vertical_color: TimelineValue::new_const(Color::<u8>::from_rgba(255, 255, 255, 96)),
            spacing: TimelineValue::new_const(Vec2::splat(DEFAULT_GRID_SPACING)),
            line_width: TimelineValue::new_const(Vec2::splat(DEFAULT_GRID_LINE_WIDTH)),
            position: TimelineValue::new_const(Vec2::ZERO),
            rotation_degrees: TimelineValue::new_const(0.0),
            line_style: TimelineValue::new_const(GridLineStyle::Solid),
            dash_length: TimelineValue::new_const(24.0),
            dash_gap: TimelineValue::new_const(16.0),
            dash_position: TimelineValue::new_const(0.0),
            wobble_amount: TimelineValue::new_const(0.0),
            wobble_scale: TimelineValue::new_const(160.0),
            wobble_position: TimelineValue::new_const(0.0),
            middle_padding: TimelineValue::new_const(Vec2::ZERO),
            padding_randomness: TimelineValue::new_const(Vec2::ZERO),
            seed: TimelineValue::new_const(0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CenteredLines {
    pub background_color: TimelineValue<Color<u8>>,
    pub line_color: TimelineValue<Color<u8>>,
    pub center: TimelineValue<Vec2>,
    #[serde(default)]
    pub rotation_degrees: TimelineValue<f32>,
    pub line_count: TimelineValue<u32>,
    pub line_width: TimelineValue<f32>,
    pub line_width_randomness: TimelineValue<f32>,
    pub line_length: TimelineValue<f32>,
    pub line_length_randomness: TimelineValue<f32>,
    pub line_offset: TimelineValue<f32>,
    pub line_offset_randomness: TimelineValue<f32>,
    #[serde(alias = "angular_uniformity")]
    pub angular_randomness: TimelineValue<f32>,
    #[serde(alias = "inner_fade")]
    pub fade_length: TimelineValue<f32>,
    pub seed: TimelineValue<u32>,
}

impl Default for CenteredLines {
    fn default() -> Self {
        Self {
            background_color: TimelineValue::new_const(Color::<u8>::BLACK),
            line_color: TimelineValue::new_const(Color::<u8>::from_rgba(255, 255, 255, 128)),
            center: TimelineValue::new_const(Vec2::splat(0.5)),
            rotation_degrees: TimelineValue::new_const(0.0),
            line_count: TimelineValue::new_const(DEFAULT_CENTERED_LINE_COUNT),
            line_width: TimelineValue::new_const(DEFAULT_CENTERED_LINE_WIDTH),
            line_width_randomness: TimelineValue::new_const(0.4),
            line_length: TimelineValue::new_const(DEFAULT_CENTERED_LINE_LENGTH),
            line_length_randomness: TimelineValue::new_const(0.5),
            line_offset: TimelineValue::new_const(DEFAULT_CENTERED_LINE_OFFSET),
            line_offset_randomness: TimelineValue::new_const(
                DEFAULT_CENTERED_LINE_OFFSET_RANDOMNESS,
            ),
            angular_randomness: TimelineValue::new_const(0.5),
            fade_length: TimelineValue::new_const(DEFAULT_CENTERED_LINE_FADE_LENGTH),
            seed: TimelineValue::new_const(0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseDistribution {
    #[default]
    Uniform,
    Gaussian,
    Binary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseColorMode {
    #[default]
    Monochrome,
    Rgb,
    Duotone,
}

shrimply_core::timeline_value::timeline_step_type!(
    NoiseDistribution,
    NoiseDistribution::Uniform,
    &[
        TimelineStepVariant {
            value: NoiseDistribution::Uniform,
            key: "uniform",
            label: "Uniform",
            icon: None
        },
        TimelineStepVariant {
            value: NoiseDistribution::Gaussian,
            key: "gaussian",
            label: "Gaussian",
            icon: None
        },
        TimelineStepVariant {
            value: NoiseDistribution::Binary,
            key: "binary",
            label: "Binary",
            icon: None
        },
    ]
);
shrimply_core::timeline_value::timeline_step_type!(
    NoiseColorMode,
    NoiseColorMode::Monochrome,
    &[
        TimelineStepVariant {
            value: NoiseColorMode::Monochrome,
            key: "monochrome",
            label: "Monochrome",
            icon: None
        },
        TimelineStepVariant {
            value: NoiseColorMode::Rgb,
            key: "rgb",
            label: "RGB",
            icon: None
        },
        TimelineStepVariant {
            value: NoiseColorMode::Duotone,
            key: "duotone",
            label: "Duotone",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhiteNoise {
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub distribution: TimelineValue<NoiseDistribution>,
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub color_mode: TimelineValue<NoiseColorMode>,
    pub color_a: TimelineValue<Color<u8>>,
    pub color_b: TimelineValue<Color<u8>>,
    pub pixel_size: TimelineValue<u32>,
    pub brightness: TimelineValue<f32>,
    pub contrast: TimelineValue<f32>,
    pub animated: TimelineValue<TimelineBool>,
    pub refresh_interval: TimelineValue<f32>,
    pub seed: TimelineValue<u32>,
}

impl Default for WhiteNoise {
    fn default() -> Self {
        Self {
            distribution: TimelineValue::new_const(NoiseDistribution::Uniform),
            color_mode: TimelineValue::new_const(NoiseColorMode::Monochrome),
            color_a: TimelineValue::new_const(Color::<u8>::BLACK),
            color_b: TimelineValue::new_const(Color::<u8>::WHITE),
            pixel_size: TimelineValue::new_const(1),
            brightness: TimelineValue::new_const(0.0),
            contrast: TimelineValue::new_const(1.0),
            animated: TimelineValue::new_const(TimelineBool::True),
            refresh_interval: TimelineValue::new_const(1.0 / 30.0),
            seed: TimelineValue::new_const(0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerlinMode {
    #[default]
    Fbm,
    Turbulence,
    Ridged,
}

shrimply_core::timeline_value::timeline_step_type!(
    PerlinMode,
    PerlinMode::Fbm,
    &[
        TimelineStepVariant {
            value: PerlinMode::Fbm,
            key: "fbm",
            label: "fBm",
            icon: None
        },
        TimelineStepVariant {
            value: PerlinMode::Turbulence,
            key: "turbulence",
            label: "Turbulence",
            icon: None
        },
        TimelineStepVariant {
            value: PerlinMode::Ridged,
            key: "ridged",
            label: "Ridged",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerlinNoise {
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub mode: TimelineValue<PerlinMode>,
    pub color_a: TimelineValue<Color<u8>>,
    pub color_b: TimelineValue<Color<u8>>,
    pub scale: TimelineValue<f32>,
    pub octaves: TimelineValue<u32>,
    pub lacunarity: TimelineValue<f32>,
    pub persistence: TimelineValue<f32>,
    pub contrast: TimelineValue<f32>,
    #[serde(alias = "drift")]
    pub position: TimelineValue<Vec2>,
    #[serde(alias = "evolution_speed")]
    pub evolution: TimelineValue<f32>,
    pub warp_amount: TimelineValue<f32>,
    pub warp_scale: TimelineValue<f32>,
    pub seed: TimelineValue<u32>,
}

impl Default for PerlinNoise {
    fn default() -> Self {
        Self {
            mode: TimelineValue::new_const(PerlinMode::Fbm),
            color_a: TimelineValue::new_const(Color::<u8>::BLACK),
            color_b: TimelineValue::new_const(Color::<u8>::WHITE),
            scale: TimelineValue::new_const(DEFAULT_PERLIN_SCALE),
            octaves: TimelineValue::new_const(5),
            lacunarity: TimelineValue::new_const(2.0),
            persistence: TimelineValue::new_const(0.5),
            contrast: TimelineValue::new_const(1.0),
            position: TimelineValue::new_const(Vec2::ZERO),
            evolution: TimelineValue::new_const(0.0),
            warp_amount: TimelineValue::new_const(0.0),
            warp_scale: TimelineValue::new_const(DEFAULT_PERLIN_SCALE),
            seed: TimelineValue::new_const(0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RainbowFill {
    #[default]
    Linear,
    Radial,
    Conic,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RainbowBands {
    #[default]
    Smooth,
    Stepped,
}

shrimply_core::timeline_value::timeline_step_type!(
    RainbowFill,
    RainbowFill::Linear,
    &[
        TimelineStepVariant {
            value: RainbowFill::Linear,
            key: "linear",
            label: "Linear",
            icon: None
        },
        TimelineStepVariant {
            value: RainbowFill::Radial,
            key: "radial",
            label: "Radial",
            icon: None
        },
        TimelineStepVariant {
            value: RainbowFill::Conic,
            key: "conic",
            label: "Conic",
            icon: None
        },
    ]
);
shrimply_core::timeline_value::timeline_step_type!(
    RainbowBands,
    RainbowBands::Smooth,
    &[
        TimelineStepVariant {
            value: RainbowBands::Smooth,
            key: "smooth",
            label: "Smooth",
            icon: None
        },
        TimelineStepVariant {
            value: RainbowBands::Stepped,
            key: "stepped",
            label: "Stepped",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rainbow {
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub fill: TimelineValue<RainbowFill>,
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub bands: TimelineValue<RainbowBands>,
    pub band_count: TimelineValue<u32>,
    pub center: TimelineValue<Vec2>,
    pub angle_degrees: TimelineValue<f32>,
    pub scale: TimelineValue<f32>,
    pub saturation: TimelineValue<f32>,
    pub brightness: TimelineValue<f32>,
    pub alpha: TimelineValue<f32>,
    #[serde(alias = "drift")]
    pub position: TimelineValue<Vec2>,
    #[serde(alias = "hue_speed")]
    pub hue_position: TimelineValue<f32>,
}

impl Default for Rainbow {
    fn default() -> Self {
        Self {
            fill: TimelineValue::new_const(RainbowFill::Linear),
            bands: TimelineValue::new_const(RainbowBands::Smooth),
            band_count: TimelineValue::new_const(7),
            center: TimelineValue::new_const(Vec2::splat(0.5)),
            angle_degrees: TimelineValue::new_const(0.0),
            scale: TimelineValue::new_const(1.0),
            saturation: TimelineValue::new_const(1.0),
            brightness: TimelineValue::new_const(1.0),
            alpha: TimelineValue::new_const(1.0),
            position: TimelineValue::new_const(Vec2::ZERO),
            hue_position: TimelineValue::new_const(0.0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkerboard {
    pub color_a: TimelineValue<Color<u8>>,
    pub color_b: TimelineValue<Color<u8>>,
    pub cell_size: TimelineValue<Vec2>,
    pub edge_softness: TimelineValue<f32>,
    #[serde(alias = "speed")]
    pub position: TimelineValue<Vec2>,
    pub rotation_degrees: TimelineValue<f32>,
}

impl Default for Checkerboard {
    fn default() -> Self {
        Self {
            color_a: TimelineValue::new_const(Color::<u8>::from_rgb(32, 32, 32)),
            color_b: TimelineValue::new_const(Color::<u8>::WHITE),
            cell_size: TimelineValue::new_const(Vec2::splat(DEFAULT_CHECKER_SIZE)),
            edge_softness: TimelineValue::new_const(0.0),
            position: TimelineValue::new_const(Vec2::ZERO),
            rotation_degrees: TimelineValue::new_const(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoronoiFill {
    Distance,
    #[default]
    Cells,
    Edges,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoronoiMetric {
    #[default]
    Euclidean,
    Manhattan,
    Chebyshev,
}

shrimply_core::timeline_value::timeline_step_type!(
    VoronoiFill,
    VoronoiFill::Cells,
    &[
        TimelineStepVariant {
            value: VoronoiFill::Distance,
            key: "distance",
            label: "Distance",
            icon: None
        },
        TimelineStepVariant {
            value: VoronoiFill::Cells,
            key: "cells",
            label: "Cells",
            icon: None
        },
        TimelineStepVariant {
            value: VoronoiFill::Edges,
            key: "edges",
            label: "Edges",
            icon: None
        },
    ]
);
shrimply_core::timeline_value::timeline_step_type!(
    VoronoiMetric,
    VoronoiMetric::Euclidean,
    &[
        TimelineStepVariant {
            value: VoronoiMetric::Euclidean,
            key: "euclidean",
            label: "Euclidean",
            icon: None
        },
        TimelineStepVariant {
            value: VoronoiMetric::Manhattan,
            key: "manhattan",
            label: "Manhattan",
            icon: None
        },
        TimelineStepVariant {
            value: VoronoiMetric::Chebyshev,
            key: "chebyshev",
            label: "Chebyshev",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Voronoi {
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub fill: TimelineValue<VoronoiFill>,
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub metric: TimelineValue<VoronoiMetric>,
    pub color_a: TimelineValue<Color<u8>>,
    pub color_b: TimelineValue<Color<u8>>,
    pub edge_color: TimelineValue<Color<u8>>,
    pub cell_size: TimelineValue<f32>,
    pub jitter: TimelineValue<f32>,
    pub edge_width: TimelineValue<f32>,
    #[serde(alias = "drift")]
    pub position: TimelineValue<Vec2>,
    pub motion_amount: TimelineValue<f32>,
    #[serde(alias = "motion_speed")]
    pub motion_position: TimelineValue<f32>,
    pub seed: TimelineValue<u32>,
}

impl Default for Voronoi {
    fn default() -> Self {
        Self {
            fill: TimelineValue::new_const(VoronoiFill::Cells),
            metric: TimelineValue::new_const(VoronoiMetric::Euclidean),
            color_a: TimelineValue::new_const(Color::<u8>::BLACK),
            color_b: TimelineValue::new_const(Color::<u8>::WHITE),
            edge_color: TimelineValue::new_const(Color::<u8>::BLACK),
            cell_size: TimelineValue::new_const(DEFAULT_VORONOI_SIZE),
            jitter: TimelineValue::new_const(1.0),
            edge_width: TimelineValue::new_const(2.0),
            position: TimelineValue::new_const(Vec2::ZERO),
            motion_amount: TimelineValue::new_const(0.0),
            motion_position: TimelineValue::new_const(0.0),
            seed: TimelineValue::new_const(0),
        }
    }
}
