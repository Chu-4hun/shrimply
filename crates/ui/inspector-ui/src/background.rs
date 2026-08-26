use std::cell::RefCell;
use std::rc::Rc;

use shrimply_core::timeline_value::{
    TimelineBase, TimelineBool, TimelineExpression, TimelineStep, TimelineValue, TimelineValueType,
};
use shrimply_project::project::{
    Background, BackgroundGenerator, BackgroundKind, CenteredLines, Checkerboard, Color,
    ColorGradient, Grid, PerlinNoise, Project, Rainbow, SolidColor, Time, VideoItemContent,
    Voronoi, WhiteNoise,
};

use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::{
    InspectedItem, InspectorContext,
    item::{DefaultInspectorItem, InspectorListItem},
    section::InspectorSection,
    timeline_value::{
        boolean::{BoolTarget, bool_control},
        color::{ColorAccess, ColorTarget, color_control},
        scalar::{ScalarAccess, ScalarSpec, ScalarTarget, scalar_control},
        vector::vec2::{VecAccess, VecSpec, VecTarget, vec_control_with_lock},
    },
    ui::{NumberPicker, enum_dropdown},
};

pub(super) fn item(background: &Background) -> InspectorListItem {
    DefaultInspectorItem::new(
        "background",
        "Background",
        background.clone(),
        controls,
        |context, _: Background| {
            edit(context, "reset-background", |generator| {
                *generator = BackgroundGenerator::default()
            });
        },
    )
    .boxed()
}

fn controls(background: &Background, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    kind(&section, background.generator.kind(), context);
    match &background.generator {
        BackgroundGenerator::SolidColor(value) => solid_color(&section, value, context),
        BackgroundGenerator::ColorGradient(value) => gradient(&section, value, context),
        BackgroundGenerator::Grid(value) => grid(&section, value, context),
        BackgroundGenerator::WhiteNoise(value) => noise(&section, value, context),
        BackgroundGenerator::PerlinNoise(value) => perlin(&section, value, context),
        BackgroundGenerator::CenteredLines(value) => centered_lines(&section, value, context),
        BackgroundGenerator::Rainbow(value) => rainbow(&section, value, context),
        BackgroundGenerator::Checkerboard(value) => checker(&section, value, context),
        BackgroundGenerator::Voronoi(value) => voronoi(&section, value, context),
        BackgroundGenerator::TestPattern => {}
    }
    vec![section.into_widget()]
}

fn solid_color(section: &InspectorSection, value: &SolidColor, context: &InspectorContext) {
    color_row(
        section,
        "Color",
        &value.color,
        context,
        |generator, value| {
            if let BackgroundGenerator::SolidColor(config) = generator {
                config.color = value;
            }
        },
    );
}

fn kind(section: &InspectorSection, current: BackgroundKind, context: &InspectorContext) {
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let dropdown = enum_dropdown(current, move |kind| {
        edit_parts(
            &project,
            &player,
            &key,
            "change-background-kind",
            |generator| {
                if generator.kind() == kind {
                    return false;
                }
                *generator = kind.generator();
                true
            },
        );
    });
    section.add_control_row("Type", &dropdown);
}

fn gradient(section: &InspectorSection, value: &ColorGradient, context: &InspectorContext) {
    choice(
        section,
        "Fill",
        &value.mode,
        context,
        |generator| match generator {
            BackgroundGenerator::ColorGradient(config) => Some(&config.mode),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::ColorGradient(config) => Some(&mut config.mode),
            _ => None,
        },
    );
    color_row(
        section,
        "Color A",
        &value.color_a,
        context,
        |generator, value| {
            if let BackgroundGenerator::ColorGradient(config) = generator {
                config.color_a = value;
            }
        },
    );
    color_row(
        section,
        "Color B",
        &value.color_b,
        context,
        |generator, value| {
            if let BackgroundGenerator::ColorGradient(config) = generator {
                config.color_b = value;
            }
        },
    );
    vec2(
        section,
        "Center",
        &value.center,
        (-2.0..=2.0, 0.01),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::ColorGradient(config) = generator {
                config.center = value;
            }
        },
    );
    number(
        section,
        "Angle",
        &value.angle_degrees,
        -360.0..=360.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::ColorGradient(config) = generator {
                config.angle_degrees = value;
            }
        },
    );
    number(
        section,
        "Scale",
        &value.scale,
        0.01..=100.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::ColorGradient(config) = generator {
                config.scale = value;
            }
        },
    );
    choice(
        section,
        "Blend curve",
        &value.curve,
        context,
        |generator| match generator {
            BackgroundGenerator::ColorGradient(config) => Some(&config.curve),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::ColorGradient(config) => Some(&mut config.curve),
            _ => None,
        },
    );
    vec2(
        section,
        "Position",
        &value.position,
        (-4096.0..=4096.0, 1.0),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::ColorGradient(config) = generator {
                config.position = value;
            }
        },
    );
    number(
        section,
        "Color position",
        &value.cycle_position,
        -10.0..=10.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::ColorGradient(config) = generator {
                config.cycle_position = value;
            }
        },
    );
}

fn grid(section: &InspectorSection, value: &Grid, context: &InspectorContext) {
    color_row(
        section,
        "Background",
        &value.background_color,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.background_color = value;
            }
        },
    );
    color_row(
        section,
        "Horizontal",
        &value.horizontal_color,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.horizontal_color = value;
            }
        },
    );
    color_row(
        section,
        "Vertical",
        &value.vertical_color,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.vertical_color = value;
            }
        },
    );
    vec2(
        section,
        "Spacing",
        &value.spacing,
        (1.0..=4096.0, 1.0),
        true,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.spacing = value;
            }
        },
    );
    vec2(
        section,
        "Line width",
        &value.line_width,
        (0.0..=256.0, 0.5),
        true,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.line_width = value;
            }
        },
    );
    vec2(
        section,
        "Position",
        &value.position,
        (-4096.0..=4096.0, 1.0),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.position = value;
            }
        },
    );
    number(
        section,
        "Rotation",
        &value.rotation_degrees,
        -360.0..=360.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.rotation_degrees = value;
            }
        },
    );
    choice(
        section,
        "Line style",
        &value.line_style,
        context,
        |generator| match generator {
            BackgroundGenerator::Grid(config) => Some(&config.line_style),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::Grid(config) => Some(&mut config.line_style),
            _ => None,
        },
    );
    number(
        section,
        "Dash length",
        &value.dash_length,
        0.1..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.dash_length = value;
            }
        },
    );
    number(
        section,
        "Dash gap",
        &value.dash_gap,
        0.1..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.dash_gap = value;
            }
        },
    );
    number(
        section,
        "Dash position",
        &value.dash_position,
        -4096.0..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.dash_position = value;
            }
        },
    );
    number(
        section,
        "Wobble",
        &value.wobble_amount,
        0.0..=512.0,
        0.5,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.wobble_amount = value;
            }
        },
    );
    number(
        section,
        "Wobble scale",
        &value.wobble_scale,
        0.1..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.wobble_scale = value;
            }
        },
    );
    number(
        section,
        "Wobble position",
        &value.wobble_position,
        -20.0..=20.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.wobble_position = value;
            }
        },
    );
    vec2(
        section,
        "Middle padding",
        &value.middle_padding,
        (0.0..=1.0, 0.01),
        true,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.middle_padding = value;
            }
        },
    );
    vec2(
        section,
        "Padding randomness",
        &value.padding_randomness,
        (0.0..=1.0, 0.01),
        true,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.padding_randomness = value;
            }
        },
    );
    integer(
        section,
        "Seed",
        &value.seed,
        0,
        u32::MAX,
        context,
        |generator, value| {
            if let BackgroundGenerator::Grid(config) = generator {
                config.seed = value;
            }
        },
    );
}

fn centered_lines(section: &InspectorSection, value: &CenteredLines, context: &InspectorContext) {
    color_row(
        section,
        "Background",
        &value.background_color,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.background_color = value;
            }
        },
    );
    color_row(
        section,
        "Lines",
        &value.line_color,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_color = value;
            }
        },
    );
    vec2(
        section,
        "Center",
        &value.center,
        (-2.0..=2.0, 0.01),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.center = value;
            }
        },
    );
    section.add_wide_control(&scalar_control(
        "Rotation",
        &value.rotation_degrees,
        context,
        scalar_target(value.rotation_degrees.id),
        ScalarSpec {
            drag_step: 1.0,
            digits: 0,
            integer: false,
            width_chars: 8,
            minimum: None,
            maximum: None,
            unit_name: Some("°"),
            rotating_icon: None,
            display: |value| f64::from(value),
            store: |value| value as f32,
            clamp: |value| value,
        },
    ));
    integer(
        section,
        "Line count",
        &value.line_count,
        1,
        4096,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_count = value;
            }
        },
    );
    number(
        section,
        "Line width",
        &value.line_width,
        0.0..=256.0,
        0.5,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_width = value;
            }
        },
    );
    number(
        section,
        "Width randomness",
        &value.line_width_randomness,
        0.0..=1.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_width_randomness = value;
            }
        },
    );
    number(
        section,
        "Line length",
        &value.line_length,
        0.0..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_length = value;
            }
        },
    );
    number(
        section,
        "Length randomness",
        &value.line_length_randomness,
        0.0..=1.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_length_randomness = value;
            }
        },
    );
    number(
        section,
        "Line offset",
        &value.line_offset,
        0.0..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_offset = value;
            }
        },
    );
    number(
        section,
        "Offset randomness",
        &value.line_offset_randomness,
        0.0..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.line_offset_randomness = value;
            }
        },
    );
    number(
        section,
        "Angular randomness",
        &value.angular_randomness,
        0.0..=1.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.angular_randomness = value;
            }
        },
    );
    number(
        section,
        "Fade length",
        &value.fade_length,
        0.0..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.fade_length = value;
            }
        },
    );
    integer(
        section,
        "Seed",
        &value.seed,
        0,
        u32::MAX,
        context,
        |generator, value| {
            if let BackgroundGenerator::CenteredLines(config) = generator {
                config.seed = value;
            }
        },
    );
}

fn noise(section: &InspectorSection, value: &WhiteNoise, context: &InspectorContext) {
    choice(
        section,
        "Distribution",
        &value.distribution,
        context,
        |generator| match generator {
            BackgroundGenerator::WhiteNoise(config) => Some(&config.distribution),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::WhiteNoise(config) => Some(&mut config.distribution),
            _ => None,
        },
    );
    choice(
        section,
        "Coloring",
        &value.color_mode,
        context,
        |generator| match generator {
            BackgroundGenerator::WhiteNoise(config) => Some(&config.color_mode),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::WhiteNoise(config) => Some(&mut config.color_mode),
            _ => None,
        },
    );
    color_row(
        section,
        "Color A",
        &value.color_a,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.color_a = value
            }
        },
    );
    color_row(
        section,
        "Color B",
        &value.color_b,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.color_b = value
            }
        },
    );
    integer(
        section,
        "Pixel size",
        &value.pixel_size,
        1,
        512,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.pixel_size = value
            }
        },
    );
    number(
        section,
        "Brightness",
        &value.brightness,
        -1.0..=1.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.brightness = value
            }
        },
    );
    number(
        section,
        "Contrast",
        &value.contrast,
        0.0..=8.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.contrast = value
            }
        },
    );
    boolean(
        section,
        "Animated",
        &value.animated,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.animated = value
            }
        },
    );
    duration(
        section,
        "Refresh interval",
        &value.refresh_interval,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.refresh_interval = value
            }
        },
    );
    integer(
        section,
        "Seed",
        &value.seed,
        0,
        u32::MAX,
        context,
        |generator, value| {
            if let BackgroundGenerator::WhiteNoise(config) = generator {
                config.seed = value
            }
        },
    );
}

fn perlin(section: &InspectorSection, value: &PerlinNoise, context: &InspectorContext) {
    choice(
        section,
        "Mode",
        &value.mode,
        context,
        |generator| match generator {
            BackgroundGenerator::PerlinNoise(config) => Some(&config.mode),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::PerlinNoise(config) => Some(&mut config.mode),
            _ => None,
        },
    );
    color_row(
        section,
        "Color A",
        &value.color_a,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.color_a = value
            }
        },
    );
    color_row(
        section,
        "Color B",
        &value.color_b,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.color_b = value
            }
        },
    );
    number(
        section,
        "Scale",
        &value.scale,
        1.0..=8192.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.scale = value
            }
        },
    );
    integer(
        section,
        "Octaves",
        &value.octaves,
        1,
        8,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.octaves = value
            }
        },
    );
    number(
        section,
        "Lacunarity",
        &value.lacunarity,
        1.0..=8.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.lacunarity = value
            }
        },
    );
    number(
        section,
        "Persistence",
        &value.persistence,
        0.0..=1.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.persistence = value
            }
        },
    );
    number(
        section,
        "Contrast",
        &value.contrast,
        0.0..=8.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.contrast = value
            }
        },
    );
    vec2(
        section,
        "Position",
        &value.position,
        (-4096.0..=4096.0, 1.0),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.position = value
            }
        },
    );
    number(
        section,
        "Evolution",
        &value.evolution,
        -20.0..=20.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.evolution = value
            }
        },
    );
    number(
        section,
        "Warp amount",
        &value.warp_amount,
        0.0..=8.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.warp_amount = value
            }
        },
    );
    number(
        section,
        "Warp scale",
        &value.warp_scale,
        1.0..=8192.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.warp_scale = value
            }
        },
    );
    integer(
        section,
        "Seed",
        &value.seed,
        0,
        u32::MAX,
        context,
        |generator, value| {
            if let BackgroundGenerator::PerlinNoise(config) = generator {
                config.seed = value
            }
        },
    );
}

fn rainbow(section: &InspectorSection, value: &Rainbow, context: &InspectorContext) {
    choice(
        section,
        "Fill",
        &value.fill,
        context,
        |generator| match generator {
            BackgroundGenerator::Rainbow(config) => Some(&config.fill),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::Rainbow(config) => Some(&mut config.fill),
            _ => None,
        },
    );
    choice(
        section,
        "Bands",
        &value.bands,
        context,
        |generator| match generator {
            BackgroundGenerator::Rainbow(config) => Some(&config.bands),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::Rainbow(config) => Some(&mut config.bands),
            _ => None,
        },
    );
    integer(
        section,
        "Band count",
        &value.band_count,
        2,
        256,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.band_count = value
            }
        },
    );
    vec2(
        section,
        "Center",
        &value.center,
        (-2.0..=2.0, 0.01),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.center = value
            }
        },
    );
    number(
        section,
        "Angle",
        &value.angle_degrees,
        -360.0..=360.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.angle_degrees = value
            }
        },
    );
    number(
        section,
        "Scale",
        &value.scale,
        0.01..=100.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.scale = value
            }
        },
    );
    number(
        section,
        "Saturation",
        &value.saturation,
        0.0..=1.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.saturation = value
            }
        },
    );
    number(
        section,
        "Brightness",
        &value.brightness,
        0.0..=4.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.brightness = value
            }
        },
    );
    number(
        section,
        "Alpha",
        &value.alpha,
        0.0..=1.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.alpha = value
            }
        },
    );
    vec2(
        section,
        "Position",
        &value.position,
        (-4096.0..=4096.0, 1.0),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.position = value
            }
        },
    );
    number(
        section,
        "Hue position",
        &value.hue_position,
        -10.0..=10.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Rainbow(config) = generator {
                config.hue_position = value
            }
        },
    );
}

fn checker(section: &InspectorSection, value: &Checkerboard, context: &InspectorContext) {
    color_row(
        section,
        "Color A",
        &value.color_a,
        context,
        |generator, value| {
            if let BackgroundGenerator::Checkerboard(config) = generator {
                config.color_a = value
            }
        },
    );
    color_row(
        section,
        "Color B",
        &value.color_b,
        context,
        |generator, value| {
            if let BackgroundGenerator::Checkerboard(config) = generator {
                config.color_b = value
            }
        },
    );
    vec2(
        section,
        "Cell size",
        &value.cell_size,
        (1.0..=4096.0, 1.0),
        true,
        context,
        |generator, value| {
            if let BackgroundGenerator::Checkerboard(config) = generator {
                config.cell_size = value
            }
        },
    );
    number(
        section,
        "Edge softness",
        &value.edge_softness,
        0.0..=256.0,
        0.5,
        context,
        |generator, value| {
            if let BackgroundGenerator::Checkerboard(config) = generator {
                config.edge_softness = value
            }
        },
    );
    vec2(
        section,
        "Position",
        &value.position,
        (-4096.0..=4096.0, 1.0),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::Checkerboard(config) = generator {
                config.position = value
            }
        },
    );
    number(
        section,
        "Rotation",
        &value.rotation_degrees,
        -360.0..=360.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Checkerboard(config) = generator {
                config.rotation_degrees = value
            }
        },
    );
}

fn voronoi(section: &InspectorSection, value: &Voronoi, context: &InspectorContext) {
    choice(
        section,
        "Fill",
        &value.fill,
        context,
        |generator| match generator {
            BackgroundGenerator::Voronoi(config) => Some(&config.fill),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::Voronoi(config) => Some(&mut config.fill),
            _ => None,
        },
    );
    choice(
        section,
        "Metric",
        &value.metric,
        context,
        |generator| match generator {
            BackgroundGenerator::Voronoi(config) => Some(&config.metric),
            _ => None,
        },
        |generator| match generator {
            BackgroundGenerator::Voronoi(config) => Some(&mut config.metric),
            _ => None,
        },
    );
    color_row(
        section,
        "Color A",
        &value.color_a,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.color_a = value
            }
        },
    );
    color_row(
        section,
        "Color B",
        &value.color_b,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.color_b = value
            }
        },
    );
    color_row(
        section,
        "Edge color",
        &value.edge_color,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.edge_color = value
            }
        },
    );
    number(
        section,
        "Cell size",
        &value.cell_size,
        1.0..=4096.0,
        1.0,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.cell_size = value
            }
        },
    );
    number(
        section,
        "Jitter",
        &value.jitter,
        0.0..=2.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.jitter = value
            }
        },
    );
    number(
        section,
        "Edge width",
        &value.edge_width,
        0.0..=256.0,
        0.5,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.edge_width = value
            }
        },
    );
    vec2(
        section,
        "Position",
        &value.position,
        (-4096.0..=4096.0, 1.0),
        false,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.position = value
            }
        },
    );
    number(
        section,
        "Motion amount",
        &value.motion_amount,
        0.0..=2.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.motion_amount = value
            }
        },
    );
    number(
        section,
        "Motion position",
        &value.motion_position,
        -20.0..=20.0,
        0.01,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.motion_position = value
            }
        },
    );
    integer(
        section,
        "Seed",
        &value.seed,
        0,
        u32::MAX,
        context,
        |generator, value| {
            if let BackgroundGenerator::Voronoi(config) = generator {
                config.seed = value
            }
        },
    );
}

fn number(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<f32>,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    context: &InspectorContext,
    _set: fn(&mut BackgroundGenerator, TimelineValue<f32>),
) {
    let (minimum, maximum) = (*range.start(), *range.end());
    section.add_wide_control(&scalar_control(
        label,
        value,
        context,
        scalar_target(value.id),
        ScalarSpec {
            drag_step: f64::from(step),
            digits: if step < 1.0 { 2 } else { 0 },
            integer: false,
            width_chars: 8,
            minimum: Some(f64::from(minimum)),
            maximum: Some(f64::from(maximum)),
            unit_name: None,
            rotating_icon: None,
            display: |value| f64::from(value),
            store: |value| value as f32,
            clamp: |value| value,
        },
    ));
}

fn integer(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<u32>,
    minimum: u32,
    maximum: u32,
    context: &InspectorContext,
    _set: fn(&mut BackgroundGenerator, TimelineValue<u32>),
) {
    let value_id = value.id;
    let position = player_state::snapshot(&context.player_state).position;
    let current = context
        .selected_item
        .clone()
        .and_then(|key| crate::video::visual_local_time(&context.project.borrow(), key, position))
        .map_or_else(|| value.fallback(), |time| value.value_at(time));
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let commit_project = context.project.clone();
    let control = NumberPicker::integer_builder(current)
        .minimum(f64::from(minimum))
        .maximum(f64::from(maximum))
        .on_change_integer(move |value: u32| {
            update_integer(&project, &player, key.clone(), value_id, value);
        })
        .on_commit(move |_| {
            shrimply_project::project::commit_edit(&commit_project.borrow(), "edit-background");
        })
        .build();
    let mut body = Vec::new();
    if value
        .expression
        .as_ref()
        .is_some_and(|expression| expression.enabled)
    {
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        body.push(crate::rhai_editor::editor(
            value.expression_source().map(str::to_string),
            crate::rhai_editor::ExpressionValue::Scalar,
            move |source| {
                update_integer_expression(&project, &player, key.clone(), value_id, Some(source))
            },
        ));
    }
    let keyframe_project = context.project.clone();
    let keyframe_player = context.player_state.clone();
    let keyframe_key = context.selected_item.clone();
    let expression_project = context.project.clone();
    let expression_player = context.player_state.clone();
    let expression_key = context.selected_item.clone();
    section.add_wide_control(&crate::timeline_value::layered::control(
        label,
        value,
        control,
        body,
        move |enabled| {
            toggle_integer_keyframes(
                &keyframe_project,
                &keyframe_player,
                keyframe_key.clone(),
                value_id,
                enabled,
            );
        },
        move |enabled| {
            toggle_integer_expression(
                &expression_project,
                &expression_player,
                expression_key.clone(),
                value_id,
                enabled,
            );
        },
    ));
}

fn vec2(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<glam::Vec2>,
    numeric: (std::ops::RangeInclusive<f32>, f32),
    lock: bool,
    context: &InspectorContext,
    _set: fn(&mut BackgroundGenerator, TimelineValue<glam::Vec2>),
) {
    let (range, step) = numeric;
    let (minimum, maximum) = (*range.start(), *range.end());
    section.add_wide_control(&vec_control_with_lock(
        label,
        value,
        context,
        vec_target(value.id),
        VecSpec {
            first_prefix: "X",
            second_prefix: "Y",
            drag_step: f64::from(step),
            digits: if step < 1.0 { 2 } else { 0 },
            width_chars: 7,
            minimum: Some(f64::from(minimum)),
            maximum: Some(f64::from(maximum)),
            unit_name: "",
        },
        lock,
    ));
}

fn boolean(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<TimelineBool>,
    context: &InspectorContext,
    _set: fn(&mut BackgroundGenerator, TimelineValue<TimelineBool>),
) {
    section.add_wide_control(&bool_control(
        label,
        value,
        value.fallback().get(),
        context,
        BoolTarget::Background { value_id: value.id },
    ));
}

fn duration(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<f32>,
    context: &InspectorContext,
    _set: fn(&mut BackgroundGenerator, TimelineValue<f32>),
) {
    section.add_wide_control(&scalar_control(
        label,
        value,
        context,
        scalar_target(value.id),
        ScalarSpec {
            drag_step: 0.01,
            digits: 3,
            integer: false,
            width_chars: 8,
            minimum: Some(0.001),
            maximum: Some(3600.0),
            unit_name: Some("s"),
            rotating_icon: None,
            display: |value| f64::from(value),
            store: |value| value as f32,
            clamp: |value| value.clamp(0.001, 3600.0),
        },
    ));
}

fn choice<T: TimelineStep>(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<T>,
    context: &InspectorContext,
    get: fn(&BackgroundGenerator) -> Option<&TimelineValue<T>>,
    get_mut: fn(&mut BackgroundGenerator) -> Option<&mut TimelineValue<T>>,
) {
    section.add_wide_control(&crate::timeline_value::step::step_control(
        label,
        value,
        context,
        crate::timeline_value::step::StepTarget::new(
            move |project, key| {
                let VideoItemContent::Background(background) = &project.video_item(&key)?.content
                else {
                    return None;
                };
                get(&background.generator)
            },
            move |project, key| {
                let VideoItemContent::Background(background) =
                    &mut project.video_item_mut(&key)?.content
                else {
                    return None;
                };
                get_mut(&mut background.generator)
            },
            "edit-background-enum",
            background_refresh(),
        ),
    ));
}

fn color_row(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<Color<u8>>,
    context: &InspectorContext,
    _set: fn(&mut BackgroundGenerator, TimelineValue<Color<u8>>),
) {
    section.add_wide_control(&color_control(
        label,
        value,
        context,
        ColorTarget {
            access: ColorAccess::Background { value_id: value.id },
            scope_id: None,
            local_time: video_local_time_for_key,
            global_time: video_global_time_for_key,
            duration: video_duration_for_key,
            refresh: background_refresh(),
            commit_name: "edit-background-color",
        },
    ));
}

fn scalar_target(value_id: uuid::Uuid) -> ScalarTarget {
    ScalarTarget {
        access: ScalarAccess::Background { value_id },
        scope_id: None,
        local_time: video_local_time_for_key,
        global_time: video_global_time_for_key,
        duration: video_duration_for_key,
        refresh: background_refresh(),
        commit_name: "edit-background-scalar",
    }
}

fn vec_target(value_id: uuid::Uuid) -> VecTarget {
    VecTarget {
        access: VecAccess::Background { value_id },
        scope_id: None,
        local_time: video_local_time_for_key,
        global_time: video_global_time_for_key,
        duration: video_duration_for_key,
        refresh: background_refresh(),
        commit_name: "edit-background-vector",
    }
}

fn background_refresh() -> ProjectChange {
    ProjectChange {
        video: true,
        inspector: true,
        ..Default::default()
    }
}

fn video_local_time_for_key(project: &Project, key: InspectedItem, position: Time) -> Option<Time> {
    crate::video::visual_local_time(project, key, position)
}

fn video_global_time_for_key(project: &Project, key: InspectedItem, time: Time) -> Option<Time> {
    crate::video::visual_global_time(project, key, time)
}

fn video_duration_for_key(project: &Project, key: InspectedItem) -> Option<Time> {
    let item = project.video_item(&key)?;
    shrimply_project::project::generated_item_keyframe_span(item)
        .map(|(_, end)| end)
        .or_else(|| crate::video::visual_duration(project, key))
}

fn edit(
    context: &InspectorContext,
    tag: &'static str,
    change: impl FnOnce(&mut BackgroundGenerator),
) {
    edit_parts(
        &context.project,
        &context.player_state,
        &context.selected_item,
        tag,
        |generator| {
            change(generator);
            true
        },
    );
}

fn edit_parts(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: &Option<InspectedItem>,
    tag: &'static str,
    change: impl FnOnce(&mut BackgroundGenerator) -> bool,
) {
    let Some(key) = key else {
        return;
    };
    let mut project_state = project.borrow_mut();
    let Some(VideoItemContent::Background(background)) = project_state
        .video_item_mut(key)
        .map(|item| &mut item.content)
    else {
        return;
    };
    if !change(&mut background.generator) {
        return;
    }
    shrimply_project::project::commit_edit(&project_state, tag);
    drop(project_state);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn background_integer_mut<'a>(
    project: &'a mut Project,
    key: &InspectedItem,
    value_id: uuid::Uuid,
) -> Option<&'a mut TimelineValue<u32>> {
    let VideoItemContent::Background(background) = &mut project.video_item_mut(key)?.content else {
        return None;
    };
    background.generator.integer_mut(value_id)
}

fn update_integer(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: Option<InspectedItem>,
    value_id: uuid::Uuid,
    next: u32,
) {
    let Some(key) = key else { return };
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(time) = crate::video::visual_local_time(&project, key.clone(), position) else {
        return;
    };
    let step = crate::keyframe_editor::project_frame_step(&project);
    let time = crate::keyframe_model::snap_to_frame(time, step);
    let Some(value) = background_integer_mut(&mut project, &key, value_id) else {
        return;
    };
    match &mut value.base {
        TimelineBase::Const(value) => *value = next,
        TimelineBase::Keyframes(keyframes) => {
            if let Some(keyframe) = keyframes
                .iter_mut()
                .find(|keyframe| crate::keyframe_model::same_frame(keyframe.time, time, step))
            {
                keyframe.time = time;
                keyframe.value = next;
            } else {
                keyframes.push(u32::keyframe(time, next));
                keyframes.sort_by_key(|keyframe| keyframe.time);
            }
        }
    }
    shrimply_project::project::commit_coalesced_edit(&project, "edit-background-integer");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            ..Default::default()
        },
    );
}

fn toggle_integer_keyframes(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: Option<InspectedItem>,
    value_id: uuid::Uuid,
    enabled: bool,
) {
    let Some(key) = key else { return };
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(time) = crate::video::visual_local_time(&project, key.clone(), position) else {
        return;
    };
    let Some(value) = background_integer_mut(&mut project, &key, value_id) else {
        return;
    };
    let current = value.value_at(time);
    value.base = if enabled {
        TimelineBase::Keyframes(vec![u32::keyframe(time, current)])
    } else {
        TimelineBase::Const(current)
    };
    shrimply_project::project::commit_edit(&project, "edit-background-integer-keyframes");
    drop(project);
    player_state::refresh_project(player, background_refresh());
}

fn update_integer_expression(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: Option<InspectedItem>,
    value_id: uuid::Uuid,
    source: Option<String>,
) {
    let Some(key) = key else { return };
    let mut project = project.borrow_mut();
    let Some(value) = background_integer_mut(&mut project, &key, value_id) else {
        return;
    };
    value
        .expression
        .get_or_insert_with(|| TimelineExpression {
            id: uuid::Uuid::new_v4(),
            enabled: true,
            source: String::new(),
        })
        .source = source.unwrap_or_default();
    shrimply_project::project::commit_coalesced_edit(
        &project,
        "edit-background-integer-expression",
    );
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            ..Default::default()
        },
    );
}

fn toggle_integer_expression(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: Option<InspectedItem>,
    value_id: uuid::Uuid,
    enabled: bool,
) {
    let Some(key) = key else { return };
    let mut project = project.borrow_mut();
    let Some(value) = background_integer_mut(&mut project, &key, value_id) else {
        return;
    };
    value
        .expression
        .get_or_insert_with(|| TimelineExpression {
            id: uuid::Uuid::new_v4(),
            enabled,
            source: "x".to_string(),
        })
        .enabled = enabled;
    shrimply_project::project::commit_edit(&project, "edit-background-integer-expression");
    drop(project);
    player_state::refresh_project(player, background_refresh());
}
