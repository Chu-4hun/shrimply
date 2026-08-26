use glam::Vec2;
use shrimply_math_geometry::{
    arrow_vertices, cross_vertices, fit_vertices, regular_polygon_vertices, star_vertices,
};

use crate::{Geometry, Shape3dKind, Shape3dRoundingStrategy};

const MIN_STAR_POINTS: u32 = 3;
const MAX_STAR_POINTS: u32 = 64;
const BASE_CURVE_SEGMENTS: u32 = 16;
const CURVE_SEGMENTS_PER_SMOOTHNESS: u32 = 8;
const RECT_CORNER_SEGMENTS: u32 = 12;
const DUPLICATE_VERTEX_EPSILON_SCALE: f32 = f32::EPSILON * 8.0;

pub(crate) fn profile(geometry: &Geometry, size: Vec2, smoothness: u32) -> Vec<Vec<Vec2>> {
    let centered = |vertices: Vec<Vec2>| {
        vertices
            .into_iter()
            .map(|point| point - size * 0.5)
            .collect::<Vec<_>>()
    };
    let polygon = match geometry.shape {
        Shape3dKind::Box => {
            return vec![box_profile(
                size,
                geometry.corner_radius,
                geometry.rounding_strategy,
            )];
        }
        Shape3dKind::Triangle => centered(vec![
            Vec2::new(size.x * 0.5, 0.0),
            size,
            Vec2::new(0.0, size.y),
        ]),
        Shape3dKind::Star => fit_vertices(
            star_vertices(
                geometry
                    .star_points
                    .round()
                    .clamp(MIN_STAR_POINTS as f32, MAX_STAR_POINTS as f32) as u32,
                geometry.star_inner_radius_percent.clamp(0.01, 0.99),
                -std::f32::consts::FRAC_PI_2,
            ),
            -size * 0.5,
            size,
        ),
        Shape3dKind::Arrow => centered(arrow_vertices(
            size,
            geometry.arrow_shaft_width_percent.clamp(0.01, 1.0),
            geometry.arrow_head_length_percent.clamp(0.01, 1.0),
        )),
        Shape3dKind::Diamond => fit_vertices(
            regular_polygon_vertices(4, -std::f32::consts::FRAC_PI_2),
            -size * 0.5,
            size,
        ),
        Shape3dKind::Pentagon => fit_vertices(
            regular_polygon_vertices(5, -std::f32::consts::FRAC_PI_2),
            -size * 0.5,
            size,
        ),
        Shape3dKind::Hexagon => fit_vertices(regular_polygon_vertices(6, 0.0), -size * 0.5, size),
        Shape3dKind::Octagon => fit_vertices(
            regular_polygon_vertices(8, std::f32::consts::FRAC_PI_8),
            -size * 0.5,
            size,
        ),
        Shape3dKind::Cross => centered(cross_vertices(
            size,
            geometry.cross_arm_thickness_percent.clamp(0.01, 1.0),
        )),
        Shape3dKind::Heart => return vec![heart(size, smoothness)],
        Shape3dKind::Disk => return disk(geometry, size, smoothness),
        Shape3dKind::Sphere | Shape3dKind::Cone | Shape3dKind::Torus | Shape3dKind::Capsule => {
            unreachable!()
        }
    };
    vec![rounded_polygon(
        &polygon,
        geometry.corner_radius.max(0.0),
        geometry.rounding_strategy,
        smoothness,
    )]
}

fn rounded_polygon(
    vertices: &[Vec2],
    radius: f32,
    strategy: Shape3dRoundingStrategy,
    smoothness: u32,
) -> Vec<Vec2> {
    if radius <= f32::EPSILON || vertices.len() < 3 {
        return vertices.to_vec();
    }
    let mut output = Vec::new();
    for index in 0..vertices.len() {
        let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
        let corner = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        let previous_edge = previous - corner;
        let next_edge = next - corner;
        let previous_length = previous_edge.length();
        let next_length = next_edge.length();
        if previous_length <= f32::EPSILON || next_length <= f32::EPSILON {
            continue;
        }
        let half_angle = (previous_edge.dot(next_edge) / (previous_length * next_length))
            .clamp(-1.0, 1.0)
            .acos()
            * 0.5;
        let distance = match strategy {
            Shape3dRoundingStrategy::Circular => radius / half_angle.tan().max(f32::EPSILON),
            Shape3dRoundingStrategy::Continuous | Shape3dRoundingStrategy::Chamfer => radius,
        }
        .min(previous_length * 0.45)
        .min(next_length * 0.45);
        let entry = corner + previous_edge / previous_length * distance;
        let exit = corner + next_edge / next_length * distance;
        output.push(entry);
        match strategy {
            Shape3dRoundingStrategy::Chamfer => output.push(exit),
            Shape3dRoundingStrategy::Continuous => {
                for step in 1..=smoothness {
                    let amount = step as f32 / smoothness as f32;
                    let inverse = 1.0 - amount;
                    output.push(
                        entry * inverse * inverse
                            + corner * (2.0 * inverse * amount)
                            + exit * amount * amount,
                    );
                }
            }
            Shape3dRoundingStrategy::Circular => {
                let weight = half_angle.cos().max(f32::EPSILON);
                for step in 1..=smoothness {
                    let amount = step as f32 / smoothness as f32;
                    let inverse = 1.0 - amount;
                    let corner_weight = 2.0 * weight * inverse * amount;
                    output.push(
                        (entry * inverse * inverse
                            + corner * corner_weight
                            + exit * amount * amount)
                            / (inverse * inverse + corner_weight + amount * amount),
                    );
                }
            }
        }
    }
    output
}

fn box_profile(size: Vec2, corner_radius: f32, strategy: Shape3dRoundingStrategy) -> Vec<Vec2> {
    let half = size * 0.5;
    let radius = corner_radius.max(0.0).min(size.x * 0.5).min(size.y * 0.5);
    if radius <= f32::EPSILON {
        return vec![
            -half,
            Vec2::new(half.x, -half.y),
            half,
            Vec2::new(-half.x, half.y),
        ];
    }
    if strategy == Shape3dRoundingStrategy::Chamfer {
        return vec![
            Vec2::new(-half.x + radius, -half.y),
            Vec2::new(half.x - radius, -half.y),
            Vec2::new(half.x, -half.y + radius),
            Vec2::new(half.x, half.y - radius),
            Vec2::new(half.x - radius, half.y),
            Vec2::new(-half.x + radius, half.y),
            Vec2::new(-half.x, half.y - radius),
            Vec2::new(-half.x, -half.y + radius),
        ];
    }
    let mut output = Vec::new();
    for (center, start, end) in [
        (
            Vec2::new(half.x - radius, -half.y + radius),
            -std::f32::consts::FRAC_PI_2,
            0.0,
        ),
        (
            Vec2::new(half.x - radius, half.y - radius),
            0.0,
            std::f32::consts::FRAC_PI_2,
        ),
        (
            Vec2::new(-half.x + radius, half.y - radius),
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ),
        (
            Vec2::new(-half.x + radius, -half.y + radius),
            std::f32::consts::PI,
            std::f32::consts::PI + std::f32::consts::FRAC_PI_2,
        ),
    ] {
        for step in 0..=RECT_CORNER_SEGMENTS {
            let amount = step as f32 / RECT_CORNER_SEGMENTS as f32;
            let angle = start + (end - start) * amount;
            let direction = match strategy {
                Shape3dRoundingStrategy::Continuous => Vec2::new(
                    angle.cos().signum() * angle.cos().abs().sqrt(),
                    angle.sin().signum() * angle.sin().abs().sqrt(),
                ),
                Shape3dRoundingStrategy::Circular => Vec2::new(angle.cos(), angle.sin()),
                Shape3dRoundingStrategy::Chamfer => unreachable!(),
            };
            output.push(center + direction * radius);
        }
    }
    let duplicate_epsilon = size.max_element() * DUPLICATE_VERTEX_EPSILON_SCALE;
    let duplicate_epsilon_squared = duplicate_epsilon * duplicate_epsilon;
    output.dedup_by(|a, b| (*a - *b).length_squared() <= duplicate_epsilon_squared);
    if output.len() > 1
        && (output[0] - output[output.len() - 1]).length_squared() <= duplicate_epsilon_squared
    {
        output.pop();
    }
    output
}

fn disk(geometry: &Geometry, size: Vec2, smoothness: u32) -> Vec<Vec<Vec2>> {
    let completion = geometry.disk_completion_degrees.clamp(0.0, 360.0);
    if completion <= f32::EPSILON {
        return Vec::new();
    }
    let sweep = completion.to_radians();
    let segments = ((BASE_CURVE_SEGMENTS + smoothness * CURVE_SEGMENTS_PER_SMOOTHNESS) as f32
        * completion
        / 360.0)
        .ceil()
        .max(3.0) as u32;
    let start = -std::f32::consts::FRAC_PI_2;
    let radius = size * 0.5;
    let point = |index: u32, scale: f32| {
        let angle = start + sweep * index as f32 / segments as f32;
        Vec2::new(angle.cos() * radius.x, angle.sin() * radius.y) * scale
    };
    let inner = geometry.disk_inner_radius_percent.clamp(0.0, 0.95);
    if completion >= 360.0 - f32::EPSILON {
        let outer = (0..segments).map(|index| point(index, 1.0)).collect();
        if inner <= f32::EPSILON {
            vec![outer]
        } else {
            let hole = (0..segments)
                .rev()
                .map(|index| point(index, inner))
                .collect();
            vec![outer, hole]
        }
    } else {
        let mut contour = (0..=segments)
            .map(|index| point(index, 1.0))
            .collect::<Vec<_>>();
        if inner <= f32::EPSILON {
            contour.push(Vec2::ZERO);
        } else {
            contour.extend((0..=segments).rev().map(|index| point(index, inner)));
        }
        vec![contour]
    }
}

fn heart(size: Vec2, smoothness: u32) -> Vec<Vec2> {
    let segments = BASE_CURVE_SEGMENTS + smoothness * CURVE_SEGMENTS_PER_SMOOTHNESS;
    let mut vertices = (0..segments)
        .map(|index| {
            let angle = std::f32::consts::TAU * index as f32 / segments as f32;
            let sin = angle.sin();
            Vec2::new(
                16.0 * sin * sin * sin,
                13.0 * angle.cos()
                    - 5.0 * (2.0 * angle).cos()
                    - 2.0 * (3.0 * angle).cos()
                    - (4.0 * angle).cos(),
            )
        })
        .collect::<Vec<_>>();
    vertices.reverse();
    fit_vertices(vertices, -size * 0.5, size)
}
