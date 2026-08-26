#![feature(proc_macro_hygiene)]

use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use shrimply_render_core::{
    LayerBlendMode, LayerCompositeParams, LayerKind, Nv12LayerParams, TextureAddressMode,
    VideoSampleMethod,
};

mod math {
    pub use shrimply_render_core::math::*;
}

#[cuda_module]
mod device {
    use super::*;
    mod compositing;
    mod layered_image;
    mod sampling;

    #[kernel]
    pub fn composite_layered_image_layer(
        params: LayerCompositeParams,
        mut output: DisjointSlice<u32>,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(destination) = output.get_mut(idx) else {
            return;
        };
        let column = i % params.width as usize;
        let row = i / params.width as usize;
        let source_stride = params.source_pitch / core::mem::size_of::<u32>();
        let source = unsafe { *params.source.add(row * source_stride + column) };
        let clipping_alpha = if params.clipping_base.is_null() {
            1.0
        } else {
            let clipping_base_stride = params.clipping_base_pitch / core::mem::size_of::<u32>();
            math::Color::from_rgba_u32(unsafe {
                *params
                    .clipping_base
                    .add(row * clipping_base_stride + column)
            })
            .a * params.clipping_base_opacity.clamp(0.0, 1.0)
        };
        *destination = layered_image::blend(
            source,
            *destination,
            params.mode,
            params.opacity * clipping_alpha,
            params.noise_seed,
            i as u32,
        );
    }

    #[kernel]
    pub fn composite_nv12_layers(
        layers: &[Nv12LayerParams],
        motion_transforms: &[glam::Mat3],
        mut out: DisjointSlice<u32>,
        background: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(pixel) = out.get_mut(idx) else {
            return;
        };
        *pixel = background;
        let Some(first_layer) = layers.first() else {
            return;
        };
        let x = (i as u32) % first_layer.canvas_width;
        let y = (i as u32) / first_layer.canvas_width;
        *pixel = compositing::composite_pixel(layers, motion_transforms, x, y, background);
    }

    #[kernel]
    pub fn tone_map_hdr(
        input: *const math::Color,
        background: *const math::Color,
        mut output: DisjointSlice<u32>,
        toon_color_levels: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(destination) = output.get_mut(idx) else {
            return;
        };
        let source = unsafe { *input.add(i) };
        let background = unsafe { *background.add(i) };
        let mut color = source.aces_tone_mapped::<true>();
        if toon_color_levels > 0.0 {
            for channel in [&mut color.r, &mut color.g, &mut color.b] {
                *channel = (*channel * toon_color_levels).floor() / toon_color_levels;
            }
        }
        color.r = (color.r + background.r).clamp(0.0, 1.0);
        color.g = (color.g + background.g).clamp(0.0, 1.0);
        color.b = (color.b + background.b).clamp(0.0, 1.0);
        *destination = color
            .with_alpha(source.a)
            .linear_to_srgb::<false>()
            .to_rgba_u32();
    }
}

fn main() {}
