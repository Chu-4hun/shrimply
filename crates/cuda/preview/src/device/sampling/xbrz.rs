// xBRZ-inspired arbitrary-scale edge reconstruction.
//
// The original algorithm emits fixed 2x-6x pixel blocks. The compositor samples arbitrary affine
// transforms instead, so this follows xBRZ's corner-pattern detection and evaluates the selected
// corner fills at the current subpixel position.

use super::{Nv12LayerParams, load_luma, load_rgba};
use crate::math;
use shrimply_math_color::Color;

const BLEND_NONE: u8 = 0;
const BLEND_NORMAL: u8 = 1;
const BLEND_DOMINANT: u8 = 2;
const STEEPNESS_THRESHOLD: f32 = 2.2;
const DOMINANT_DIRECTION_THRESHOLD: f32 = 3.6;
const COLOR_SIMILARITY_TOLERANCE: f32 = 30.0 / 255.0;
const SQRT_HALF: f32 = 0.707_106_77;

pub(super) fn sample_rgba(params: &Nv12LayerParams, x: f32, y: f32) -> Color<f32> {
    if !is_magnifying(params) {
        return super::sample_rgba_bilinear(params, x, y);
    }
    scale(params, x, y, |column, row| load_rgba(params, column, row))
}

pub(super) fn sample_luma(params: &Nv12LayerParams, x: f32, y: f32) -> f32 {
    if !is_magnifying(params) {
        return super::sample_luma_weighted(params, x, y, super::VideoSampleMethod::Lanczos3);
    }
    scale(params, x, y, |column, row| {
        let value = load_luma(params, column, row);
        Color::new(value, value, value, 1.0)
    })
    .r
}

fn scale(
    params: &Nv12LayerParams,
    x: f32,
    y: f32,
    load: impl Fn(i32, i32) -> Color<f32>,
) -> Color<f32> {
    let base_x = math::floor_f32(x + 0.5) as i32;
    let base_y = math::floor_f32(y + 0.5) as i32;
    let pixel_offset = math::Vec2::new(x - base_x as f32, y - base_y as f32);
    let magnification = magnification(params);
    let mut blends = [BLEND_NONE; 4];

    // Corner bits are x-positive (1) and y-positive (2): TL, TR, BL, BR.
    for corner in 0..4 {
        let sx = if corner & 1 == 0 { -1 } else { 1 };
        let sy = if corner & 2 == 0 { -1 } else { 1 };
        let at = |column: i32, row: i32| load(base_x + column * sx, base_y + row * sy);
        let center = at(0, 0);
        let right = at(1, 0);
        let bottom = at(0, 1);
        let diagonal = at(1, 1);

        if ((same(center, right) && same(bottom, diagonal))
            || (same(center, bottom) && same(right, diagonal)))
            || same(center, right)
            || same(center, bottom)
        {
            continue;
        }

        let downward = 4.0 * center.bt2020_ycbcr_distance(diagonal)
            + right.bt2020_ycbcr_distance(at(2, 1))
            + bottom.bt2020_ycbcr_distance(at(1, 2))
            + at(0, -1).bt2020_ycbcr_distance(right)
            + at(-1, 0).bt2020_ycbcr_distance(bottom);
        let upward = 4.0 * bottom.bt2020_ycbcr_distance(right)
            + diagonal.bt2020_ycbcr_distance(at(2, 0))
            + at(0, 2).bt2020_ycbcr_distance(diagonal)
            + center.bt2020_ycbcr_distance(at(1, -1))
            + at(-1, 1).bt2020_ycbcr_distance(center);
        if upward < downward {
            blends[corner] = if DOMINANT_DIRECTION_THRESHOLD * upward < downward {
                BLEND_DOMINANT
            } else {
                BLEND_NORMAL
            };
        }
    }

    let mut color = load(base_x, base_y);
    for corner in 0..4 {
        if blends[corner] == BLEND_NONE {
            continue;
        }
        let sx = if corner & 1 == 0 { -1 } else { 1 };
        let sy = if corner & 2 == 0 { -1 } else { 1 };
        let at = |column: i32, row: i32| load(base_x + column * sx, base_y + row * sy);
        let center = at(0, 0);
        let right = at(1, 0);
        let bottom = at(0, 1);
        let shallow = right.bt2020_ycbcr_distance(at(-1, 1));
        let steep = bottom.bt2020_ycbcr_distance(at(1, -1));
        let vertical_blend =
            blends[corner ^ 2] != BLEND_NONE && !is_color_similar(center, at(-1, 1));
        let horizontal_blend =
            blends[corner ^ 1] != BLEND_NONE && !is_color_similar(center, at(1, -1));
        let is_corner = !is_color_similar(center, at(1, 1))
            && is_color_similar(at(1, -1), right)
            && is_color_similar(right, at(1, 1))
            && is_color_similar(at(1, 1), bottom)
            && is_color_similar(bottom, at(-1, 1));
        let split_diagonally = blends[corner] == BLEND_DOMINANT
            || (!vertical_blend && !horizontal_blend && !is_corner);

        let mut origin = math::Vec2::new(0.0, SQRT_HALF);
        let mut direction = math::Vec2::new(1.0, -1.0);
        if split_diagonally {
            let is_shallow = (STEEPNESS_THRESHOLD * shallow <= steep
                && !same(center, at(-1, 1))
                && !same(at(-1, 0), at(-1, 1))) as u8 as f32;
            let is_steep = (STEEPNESS_THRESHOLD * steep <= shallow
                && !same(center, at(1, -1))
                && !same(at(1, -1), at(0, -1))) as u8 as f32;
            origin = math::Vec2::new(0.0, 0.5 * (1.0 - 0.5 * is_shallow));
            direction = math::Vec2::new(1.0 + is_shallow, -1.0 - is_steep);
        }

        let canonical_offset = pixel_offset * math::Vec2::new(sx as f32, sy as f32);
        let fill = if center.bt2020_ycbcr_distance(right) < center.bt2020_ycbcr_distance(bottom) {
            right
        } else {
            bottom
        };
        color = color.lerp(
            fill,
            scaled_fill_ratio(canonical_offset, origin, direction, magnification),
        );
    }
    color
}

fn same(a: Color<f32>, b: Color<f32>) -> bool {
    a.bt2020_ycbcr_distance(b) < 1.0 / 65_536.0
}

fn is_color_similar(a: Color<f32>, b: Color<f32>) -> bool {
    a.bt2020_ycbcr_distance(b) < COLOR_SIMILARITY_TOLERANCE
}

fn scaled_fill_ratio(
    pixel_offset: math::Vec2,
    origin: math::Vec2,
    direction: math::Vec2,
    scale: math::Vec2,
) -> f32 {
    let offset = pixel_offset - origin;
    let along = offset.dot(direction) / direction.length_squared();
    let distance = (offset - direction * along) * scale;
    let side = direction.perp_dot(offset).signum();
    let signed_distance = side * (distance.length_squared()).sqrt();
    math::smoothstep(-SQRT_HALF, SQRT_HALF, signed_distance)
}

fn is_magnifying(params: &Nv12LayerParams) -> bool {
    let scale = magnification(params);
    scale.x >= 1.0 && scale.y >= 1.0
}

fn magnification(params: &Nv12LayerParams) -> math::Vec2 {
    let x = (params.inverse.x_axis.x * params.inverse.x_axis.x
        + params.inverse.y_axis.x * params.inverse.y_axis.x)
        .sqrt();
    let y = (params.inverse.x_axis.y * params.inverse.x_axis.y
        + params.inverse.y_axis.y * params.inverse.y_axis.y)
        .sqrt();
    math::Vec2::new(1.0 / x.max(0.000_001), 1.0 / y.max(0.000_001))
}
