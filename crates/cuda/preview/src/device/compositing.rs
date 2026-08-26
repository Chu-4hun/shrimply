use shrimply_math_color::Color;

use super::{LayerKind, Nv12LayerParams, TextureAddressMode, sampling};
use crate::math;

pub(super) fn composite_pixel(
    layers: &[Nv12LayerParams],
    motion_transforms: &[glam::Mat3],
    x: u32,
    y: u32,
    background: u32,
) -> u32 {
    let canvas_x = x as f32 + 0.5;
    let canvas_y = y as f32 + 0.5;
    let mut pixel = background;

    for params in layers {
        if params.motion_sample_count == 0 {
            if let Some(sample) = sample_layer(params, params.inverse, canvas_x, canvas_y) {
                pixel = sample
                    .blend_over::<true>(
                        Color::from_rgba_u32(pixel),
                        params.blend_mode,
                        sample.a * params.opacity,
                    )
                    .to_rgba_u32();
            }
            continue;
        }

        let mut red = 0.0;
        let mut green = 0.0;
        let mut blue = 0.0;
        let mut alpha = 0.0;
        let start = params.motion_transform_offset as usize;
        let end = start.saturating_add(params.motion_transform_count as usize);
        let Some(transforms) = motion_transforms.get(start..end) else {
            continue;
        };
        for inverse in transforms {
            let Some(sample) = sample_layer(params, *inverse, canvas_x, canvas_y) else {
                continue;
            };
            red += sample.r * sample.a;
            green += sample.g * sample.a;
            blue += sample.b * sample.a;
            alpha += sample.a;
        }
        if alpha <= 0.0 {
            continue;
        }
        let source = Color::new(red / alpha, green / alpha, blue / alpha, alpha);
        let averaged_alpha = alpha / params.motion_sample_count as f32;
        pixel = source
            .blend_over::<true>(
                Color::from_rgba_u32(pixel),
                params.blend_mode,
                averaged_alpha * params.opacity,
            )
            .to_rgba_u32();
    }

    pixel
}

fn sample_layer(
    params: &Nv12LayerParams,
    inverse: glam::Mat3,
    canvas_x: f32,
    canvas_y: f32,
) -> Option<Color<f32>> {
    let source = math::transform_point2(inverse, glam::Vec2::new(canvas_x, canvas_y));
    let source_x = source.x;
    let source_y = source.y;
    let crop_left = params.source_width as f32 * params.crop[3];
    let crop_right = params.source_width as f32 * (1.0 - params.crop[1]);
    let crop_top = params.source_height as f32 * params.crop[0];
    let crop_bottom = params.source_height as f32 * (1.0 - params.crop[2]);
    if source_x < crop_left - params.padding[3]
        || source_y < crop_top - params.padding[0]
        || source_x >= crop_right + params.padding[1]
        || source_y >= crop_bottom + params.padding[2]
        || (matches!(params.address_mode, TextureAddressMode::Transparent)
            && (source_x < crop_left
                || source_y < crop_top
                || source_x >= crop_right
                || source_y >= crop_bottom))
    {
        return None;
    }

    Some(match params.kind {
        LayerKind::Nv12 => sampling::sample_nv12(params, source_x, source_y).into(),
        LayerKind::Rgba => {
            let sample = sampling::sample_rgba(params, source_x, source_y);
            sample
        }
    })
}
