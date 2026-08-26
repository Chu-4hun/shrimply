// Ported from AMD FidelityFX Super Resolution 1.0.2 EASU (MIT).

use super::{
    ChromaComponent, Nv12LayerParams, Nv12Sample, TextureAddressMode, VideoSampleMethod, load_luma,
    load_rgba, sample_chroma_bilinear,
};
use crate::math;
use shrimply_math_color::Color;

pub(super) fn sample_rgba(params: &Nv12LayerParams, x: f32, y: f32) -> Color<f32> {
    let bilinear = super::sample_rgba_bilinear(params, x, y);
    if !is_magnifying(params) {
        return bilinear;
    }
    easu(x, y, |column, row| load_rgba(params, column, row)).with_alpha(bilinear.a)
}

pub(super) fn sample_nv12(params: &Nv12LayerParams, x: f32, y: f32, alpha: f32) -> Nv12Sample {
    if !is_magnifying(params) {
        return finish_nv12(
            params,
            Nv12Sample {
                luma: super::sample_luma_weighted(params, x, y, VideoSampleMethod::Lanczos3),
                cb: sample_chroma_bilinear(
                    params,
                    (x - 0.5) * 0.5,
                    (y - 0.5) * 0.5,
                    ChromaComponent::Cb,
                ) - 0.5,
                cr: sample_chroma_bilinear(
                    params,
                    (x - 0.5) * 0.5,
                    (y - 0.5) * 0.5,
                    ChromaComponent::Cr,
                ) - 0.5,
                alpha,
            },
        );
    }
    let rgb = easu(x, y, |column, row| nv12_rgb(params, column, row));
    let full_luma = rgb.rec709_luma();
    finish_nv12(
        params,
        Nv12Sample {
            luma: full_luma / 1.164_383_5 + 0.0625,
            cb: (rgb.b - full_luma) / 2.112_401_7,
            cr: (rgb.r - full_luma) / 1.792_741_1,
            alpha,
        },
    )
}

fn finish_nv12(params: &Nv12LayerParams, mut sample: Nv12Sample) -> Nv12Sample {
    if matches!(params.address_mode, TextureAddressMode::Transparent) && sample.alpha > 0.000_001 {
        sample.luma /= sample.alpha;
        sample.cb /= sample.alpha;
        sample.cr /= sample.alpha;
    }
    sample
}

fn nv12_rgb(params: &Nv12LayerParams, x: i32, y: i32) -> Color<f32> {
    let chroma_x = (x as f32 - 0.5) * 0.5;
    let chroma_y = (y as f32 - 0.5) * 0.5;
    let sample = Nv12Sample {
        luma: load_luma(params, x, y),
        cb: sample_chroma_bilinear(params, chroma_x, chroma_y, ChromaComponent::Cb) - 0.5,
        cr: sample_chroma_bilinear(params, chroma_x, chroma_y, ChromaComponent::Cr) - 0.5,
        alpha: 1.0,
    };
    sample.into()
}

fn is_magnifying(params: &Nv12LayerParams) -> bool {
    let x = (params.inverse.x_axis.x * params.inverse.x_axis.x
        + params.inverse.y_axis.x * params.inverse.y_axis.x)
        .sqrt();
    let y = (params.inverse.x_axis.y * params.inverse.x_axis.y
        + params.inverse.y_axis.y * params.inverse.y_axis.y)
        .sqrt();
    x > 0.0 && x <= 1.0 && y > 0.0 && y <= 1.0
}

fn easu(x: f32, y: f32, load: impl Fn(i32, i32) -> Color<f32>) -> Color<f32> {
    let base_x = math::floor_f32(x) as i32;
    let base_y = math::floor_f32(y) as i32;
    let point = math::Vec2::new(x - base_x as f32, y - base_y as f32);
    let pixels = [
        load(base_x, base_y - 1),
        load(base_x + 1, base_y - 1),
        load(base_x - 1, base_y),
        load(base_x, base_y),
        load(base_x + 1, base_y),
        load(base_x + 2, base_y),
        load(base_x - 1, base_y + 1),
        load(base_x, base_y + 1),
        load(base_x + 1, base_y + 1),
        load(base_x + 2, base_y + 1),
        load(base_x, base_y + 2),
        load(base_x + 1, base_y + 2),
    ];
    let mut luma = [0.0; 12];
    for index in 0..12 {
        luma[index] = 0.5 * pixels[index].r + pixels[index].g + 0.5 * pixels[index].b;
    }
    let mut direction = math::Vec2::ZERO;
    let mut length = 0.0;
    accumulate_direction(
        &mut direction,
        &mut length,
        (1.0 - point.x) * (1.0 - point.y),
        [luma[0], luma[2], luma[3], luma[4], luma[7]],
    );
    accumulate_direction(
        &mut direction,
        &mut length,
        point.x * (1.0 - point.y),
        [luma[1], luma[3], luma[4], luma[5], luma[8]],
    );
    accumulate_direction(
        &mut direction,
        &mut length,
        (1.0 - point.x) * point.y,
        [luma[3], luma[6], luma[7], luma[8], luma[10]],
    );
    accumulate_direction(
        &mut direction,
        &mut length,
        point.x * point.y,
        [luma[4], luma[7], luma[8], luma[9], luma[11]],
    );

    let magnitude = (direction.length_squared()).sqrt();
    if magnitude * magnitude < 1.0 / 32768.0 {
        direction = math::Vec2::X;
    } else {
        direction /= magnitude;
    }
    length *= 0.5;
    length *= length;
    let stretch = 1.0 / direction.x.abs().max(direction.y.abs());
    let anisotropy = math::Vec2::new(1.0 + (stretch - 1.0) * length, 1.0 - 0.5 * length);
    let lobe = 0.5 + ((0.25 - 0.04) - 0.5) * length;
    let clip = 1.0 / lobe;

    let nearest = [pixels[3], pixels[4], pixels[7], pixels[8]];
    let minimum = component_min(nearest);
    let maximum = component_max(nearest);
    let offsets = [
        [0.0, -1.0],
        [1.0, -1.0],
        [-1.0, 0.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [2.0, 0.0],
        [-1.0, 1.0],
        [0.0, 1.0],
        [1.0, 1.0],
        [2.0, 1.0],
        [0.0, 2.0],
        [1.0, 2.0],
    ];
    let mut color = Color::new(0.0, 0.0, 0.0, 1.0);
    let mut weight_sum = 0.0;
    for index in 0..12 {
        tap(
            &mut color,
            &mut weight_sum,
            math::Vec2::new(offsets[index][0], offsets[index][1]) - point,
            direction,
            anisotropy,
            lobe,
            clip,
            pixels[index],
        );
    }
    let inverse_weight = 1.0 / weight_sum;
    Color::new(
        (color.r * inverse_weight).clamp(minimum.r, maximum.r),
        (color.g * inverse_weight).clamp(minimum.g, maximum.g),
        (color.b * inverse_weight).clamp(minimum.b, maximum.b),
        1.0,
    )
}

fn accumulate_direction(direction: &mut math::Vec2, length: &mut f32, weight: f32, p: [f32; 5]) {
    let left = p[2] - p[1];
    let right = p[3] - p[2];
    let horizontal_range = left.abs().max(right.abs());
    let horizontal = p[3] - p[1];
    direction.x += horizontal * weight;
    let horizontal_length = if horizontal_range > 0.0 {
        (horizontal.abs() / horizontal_range).min(1.0)
    } else {
        0.0
    };
    *length += horizontal_length * horizontal_length * weight;

    let top = p[2] - p[0];
    let bottom = p[4] - p[2];
    let vertical_range = top.abs().max(bottom.abs());
    let vertical = p[4] - p[0];
    direction.y += vertical * weight;
    let vertical_length = if vertical_range > 0.0 {
        (vertical.abs() / vertical_range).min(1.0)
    } else {
        0.0
    };
    *length += vertical_length * vertical_length * weight;
}

fn tap(
    color: &mut Color<f32>,
    weight_sum: &mut f32,
    offset: math::Vec2,
    direction: math::Vec2,
    anisotropy: math::Vec2,
    lobe: f32,
    clip: f32,
    sample: Color<f32>,
) {
    let rotated_x = offset.dot(direction);
    let rotated_y = direction.perp_dot(offset);
    let x = rotated_x * anisotropy.x;
    let y = rotated_y * anisotropy.y;
    let distance = (x * x + y * y).min(clip);
    let mut base = 0.4 * distance - 1.0;
    let mut window = lobe * distance - 1.0;
    base *= base;
    window *= window;
    base = 25.0 / 16.0 * base - (25.0 / 16.0 - 1.0);
    let weight = base * window;
    color.r += sample.r * weight;
    color.g += sample.g * weight;
    color.b += sample.b * weight;
    *weight_sum += weight;
}

fn component_min(colors: [Color<f32>; 4]) -> Color<f32> {
    Color::new(
        colors[0]
            .r
            .min(colors[1].r)
            .min(colors[2].r)
            .min(colors[3].r),
        colors[0]
            .g
            .min(colors[1].g)
            .min(colors[2].g)
            .min(colors[3].g),
        colors[0]
            .b
            .min(colors[1].b)
            .min(colors[2].b)
            .min(colors[3].b),
        1.0,
    )
}

fn component_max(colors: [Color<f32>; 4]) -> Color<f32> {
    Color::new(
        colors[0]
            .r
            .max(colors[1].r)
            .max(colors[2].r)
            .max(colors[3].r),
        colors[0]
            .g
            .max(colors[1].g)
            .max(colors[2].g)
            .max(colors[3].g),
        colors[0]
            .b
            .max(colors[1].b)
            .max(colors[2].b)
            .max(colors[3].b),
        1.0,
    )
}
