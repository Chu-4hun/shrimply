// Ported from NVIDIA Image Scaling SDK 1.0.3 (MIT).
// The reference compute shader shares its 6x6 source tile across a workgroup. Shrimply's
// compositor evaluates arbitrary affine layers per output pixel, so this port keeps the same
// filter math in registers and relies on the texture cache for neighboring output pixels.

use super::{Nv12LayerParams, VideoSampleMethod, load_luma, load_rgba};
use crate::math;
use shrimply_math_color::Color;

mod coefficients;

const PHASES: f32 = 64.0;
const DETECT_RATIO: f32 = 2.201_171_9;
const DETECT_THRESHOLD: f32 = 0.0625;
const MIN_CONTRAST_RATIO: f32 = 2.0;
const RATIO_NORMALIZATION: f32 = 0.125;
const EPSILON: f32 = 1.0 / 255.0;
const SHARP_START_Y: f32 = 0.45;
const SHARP_SCALE_Y: f32 = 1.0 / 0.45;
const SHARP_STRENGTH_MIN: f32 = 0.4;
const SHARP_STRENGTH_SCALE: f32 = 1.2;
const SHARP_LIMIT_MIN: f32 = 0.14;
const SHARP_LIMIT_SCALE: f32 = 0.36;

pub(super) fn sample_luma(params: &Nv12LayerParams, x: f32, y: f32) -> f32 {
    if !supported_scale(params) {
        return super::sample_luma_weighted(params, x, y, VideoSampleMethod::Lanczos3);
    }
    let original = super::sample_luma_bilinear(params, x, y);
    let filtered = filtered_luma(x, y, |column, row| load_luma(params, column, row));
    original + (filtered - original) / 1.164_383_5
}

pub(super) fn sample_rgba(params: &Nv12LayerParams, x: f32, y: f32) -> Color<f32> {
    let mut sample = super::sample_rgba_bilinear(params, x, y);
    if !supported_scale(params) {
        return sample;
    }
    let original_luma = sample.rec709_luma();
    let filtered = filtered_luma(x, y, |column, row| {
        load_rgba(params, column, row).rec709_luma()
    });
    let correction = filtered - original_luma;
    sample.r = (sample.r + correction).clamp(0.0, 1.0);
    sample.g = (sample.g + correction).clamp(0.0, 1.0);
    sample.b = (sample.b + correction).clamp(0.0, 1.0);
    sample
}

fn supported_scale(params: &Nv12LayerParams) -> bool {
    let x = (params.inverse.x_axis.x * params.inverse.x_axis.x
        + params.inverse.y_axis.x * params.inverse.y_axis.x)
        .sqrt();
    let y = (params.inverse.x_axis.y * params.inverse.x_axis.y
        + params.inverse.y_axis.y * params.inverse.y_axis.y)
        .sqrt();
    x > 0.0 && x <= 1.0 && y > 0.0 && y <= 1.0
}

fn filtered_luma(x: f32, y: f32, load: impl Fn(i32, i32) -> f32) -> f32 {
    let base_x = math::floor_f32(x) as i32;
    let base_y = math::floor_f32(y) as i32;
    let fraction_x = x - base_x as f32;
    let fraction_y = y - base_y as f32;
    let phase_x = (fraction_x * PHASES) as usize;
    let phase_y = (fraction_y * PHASES) as usize;
    let mut pixels = [[0.0; 6]; 6];
    for (row, values) in pixels.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            *value = load(base_x + column as i32 - 2, base_y + row as i32 - 2);
        }
    }

    let edges = [
        [edge_map(&pixels, 0, 0), edge_map(&pixels, 0, 1)],
        [edge_map(&pixels, 1, 0), edge_map(&pixels, 1, 1)],
    ];
    let weights = interpolate_edges(edges, fraction_x, fraction_y);
    let base_weight = 1.0 - weights[0] - weights[1] - weights[2] - weights[3];
    filter_normal(&pixels, phase_x, phase_y) * base_weight
        + directional_filters(&pixels, fraction_x, fraction_y, phase_x, phase_y, weights)
}

fn edge_map(p: &[[f32; 6]; 6], row: usize, column: usize) -> [f32; 4] {
    let g0 = (p[row][column] + p[row][column + 1] + p[row][column + 2]
        - p[row + 2][column]
        - p[row + 2][column + 1]
        - p[row + 2][column + 2])
        .abs();
    let g45 = (p[row + 1][column] + p[row][column] + p[row][column + 1]
        - p[row + 2][column + 1]
        - p[row + 2][column + 2]
        - p[row + 1][column + 2])
        .abs();
    let g90 = (p[row][column] + p[row + 1][column] + p[row + 2][column]
        - p[row][column + 2]
        - p[row + 1][column + 2]
        - p[row + 2][column + 2])
        .abs();
    let g135 = (p[row + 1][column] + p[row + 2][column] + p[row + 2][column + 1]
        - p[row][column + 1]
        - p[row][column + 2]
        - p[row + 1][column + 2])
        .abs();
    let max_axis = g0.max(g90);
    let min_axis = g0.min(g90);
    let max_diagonal = g45.max(g135);
    let min_diagonal = g45.min(g135);
    if max_axis + max_diagonal == 0.0 {
        return [0.0; 4];
    }
    let axis_share = (max_axis / (max_axis + max_diagonal)).min(1.0);
    let diagonal_share = 1.0 - axis_share;
    let axis = max_axis > min_axis * DETECT_RATIO
        && max_axis > DETECT_THRESHOLD
        && max_axis > min_diagonal;
    let diagonal = max_diagonal > min_diagonal * DETECT_RATIO
        && max_diagonal > DETECT_THRESHOLD
        && max_diagonal > min_axis;
    let axis_weight = if axis && diagonal { axis_share } else { 1.0 };
    let diagonal_weight = if axis && diagonal {
        diagonal_share
    } else {
        1.0
    };
    [
        if axis && g0 >= g90 { axis_weight } else { 0.0 },
        if axis && g90 > g0 { axis_weight } else { 0.0 },
        if diagonal && g45 >= g135 {
            diagonal_weight
        } else {
            0.0
        },
        if diagonal && g135 > g45 {
            diagonal_weight
        } else {
            0.0
        },
    ]
}

fn interpolate_edges(edges: [[[f32; 4]; 2]; 2], x: f32, y: f32) -> [f32; 4] {
    let mut result = [0.0; 4];
    for (index, value) in result.iter_mut().enumerate() {
        let top = math::lerp(edges[0][0][index], edges[0][1][index], x);
        let bottom = math::lerp(edges[1][0][index], edges[1][1][index], x);
        *value = math::lerp(top, bottom, y);
    }
    result
}

fn filter_normal(p: &[[f32; 6]; 6], phase_x: usize, phase_y: usize) -> f32 {
    let mut result = 0.0;
    for column in 0..6 {
        let mut vertical = 0.0;
        for row in 0..6 {
            vertical += p[row][column] * coefficients::SCALER[phase_y][row];
        }
        result += vertical * coefficients::SCALER[phase_x][column];
    }
    result
}

fn directional_filters(
    p: &[[f32; 6]; 6],
    fx: f32,
    fy: f32,
    phase_x: usize,
    phase_y: usize,
    weights: [f32; 4],
) -> f32 {
    let mut result = 0.0;
    if weights[0] > 0.0 {
        let mut values = [0.0; 6];
        for i in 0..6 {
            values[i] = math::lerp(p[i][2], p[i][3], fx);
        }
        result += evaluate(values, phase_y) * weights[0];
    }
    if weights[1] > 0.0 {
        let mut values = [0.0; 6];
        for i in 0..6 {
            values[i] = math::lerp(p[2][i], p[3][i], fy);
        }
        result += evaluate(values, phase_x) * weights[1];
    }
    if weights[2] > 0.0 {
        let mut blend = 0.5 + 0.5 * (fx - fy);
        let mut temporary = [0.0; 7];
        temporary[1] = math::lerp(p[2][1], p[1][2], blend);
        temporary[3] = math::lerp(p[3][2], p[2][3], blend);
        temporary[5] = math::lerp(p[4][3], p[3][4], blend);
        blend -= 0.5;
        let (a, b, c, d) = if blend >= 0.0 {
            (p[0][2], p[1][3], p[2][4], p[3][5])
        } else {
            (p[2][0], p[3][1], p[4][2], p[5][3])
        };
        temporary[0] = math::lerp(p[1][1], a, blend.abs());
        temporary[2] = math::lerp(p[2][2], b, blend.abs());
        temporary[4] = math::lerp(p[3][3], c, blend.abs());
        temporary[6] = math::lerp(p[4][4], d, blend.abs());
        let mut phase = fx + fy;
        let offset = if phase >= 1.0 {
            phase -= 1.0;
            1
        } else {
            0
        };
        result += evaluate(
            [
                temporary[offset],
                temporary[offset + 1],
                temporary[offset + 2],
                temporary[offset + 3],
                temporary[offset + 4],
                temporary[offset + 5],
            ],
            phase_index(phase),
        ) * weights[2];
    }
    if weights[3] > 0.0 {
        let mut blend = 0.5 * (fx + fy);
        let mut temporary = [0.0; 7];
        temporary[1] = math::lerp(p[3][1], p[4][2], blend);
        temporary[3] = math::lerp(p[2][2], p[3][3], blend);
        temporary[5] = math::lerp(p[1][3], p[2][4], blend);
        blend -= 0.5;
        let (a, b, c, d) = if blend >= 0.0 {
            (p[5][2], p[4][3], p[3][4], p[2][5])
        } else {
            (p[3][0], p[2][1], p[1][2], p[0][3])
        };
        temporary[0] = math::lerp(p[4][1], a, blend.abs());
        temporary[2] = math::lerp(p[3][2], b, blend.abs());
        temporary[4] = math::lerp(p[2][3], c, blend.abs());
        temporary[6] = math::lerp(p[1][4], d, blend.abs());
        let mut phase = 1.0 + fx - fy;
        let offset = if phase >= 1.0 {
            phase -= 1.0;
            1
        } else {
            0
        };
        result += evaluate(
            [
                temporary[offset],
                temporary[offset + 1],
                temporary[offset + 2],
                temporary[offset + 3],
                temporary[offset + 4],
                temporary[offset + 5],
            ],
            phase_index(phase),
        ) * weights[3];
    }
    result
}

fn phase_index(phase: f32) -> usize {
    ((phase * PHASES) as usize).min(63)
}

fn evaluate(pixels: [f32; 6], phase: usize) -> f32 {
    let mut value = 0.0;
    let mut unsharp = 0.0;
    for i in 0..6 {
        value += coefficients::SCALER[phase][i] * pixels[i];
        unsharp += coefficients::USM[phase][i] * pixels[i];
    }
    let scale = 1.0 - ((value - SHARP_START_Y) * SHARP_SCALE_Y).clamp(0.0, 1.0);
    unsharp *= scale * SHARP_STRENGTH_SCALE + SHARP_STRENGTH_MIN;
    let limit = (scale * SHARP_LIMIT_SCALE + SHARP_LIMIT_MIN) * value;
    unsharp = unsharp.clamp(-limit, limit);
    unsharp *= local_tone_improvement(pixels, phase);
    value + unsharp
}

fn local_tone_improvement(p: [f32; 6], phase: usize) -> f32 {
    let selector = phase <= 32;
    let first_min = p[1].min(p[2]).min(if selector { p[0] } else { p[3] });
    let first_max = p[1].max(p[2]).max(if selector { p[0] } else { p[3] });
    let second_min = p[3].min(p[4]).min(if selector { p[2] } else { p[5] });
    let second_max = p[3].max(p[4]).max(if selector { p[2] } else { p[5] });
    let first_contrast = first_max - first_min;
    let second_contrast = second_max - second_min;
    1.0 - ((first_contrast.max(second_contrast) / (first_contrast.min(second_contrast) + EPSILON)
        - MIN_CONTRAST_RATIO)
        * RATIO_NORMALIZATION)
        .clamp(0.0, 1.0)
}
