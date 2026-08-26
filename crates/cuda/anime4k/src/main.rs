#![feature(proc_macro_hygiene)]

use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use shrimply_math_color::Color;

#[path = "../../../video/anime4k/src/types.rs"]
mod types;

use types::{AlphaParams, ConvolutionParams, ConvolutionTerm, ImageDescriptor};

const ACTIVATION_POSITIVE: u32 = 1;
const ACTIVATION_NEGATIVE: u32 = 2;

#[cuda_module]
mod device {
    use super::*;

    #[kernel]
    pub fn rgba_to_float(
        source: *const u8,
        pitch: usize,
        width: u32,
        mut output: DisjointSlice<[f32; 4]>,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(destination) = output.get_mut(index) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let pixel = unsafe { *(source.add(y as usize * pitch + x as usize * 4) as *const u32) };
        *destination = Color::from_rgba_u32(pixel).premultiply().to_array();
    }

    #[kernel]
    pub fn nv12_to_float(
        y_plane: *const u8,
        uv_plane: *const u8,
        y_pitch: usize,
        uv_pitch: usize,
        width: u32,
        height: u32,
        mut output: DisjointSlice<[f32; 4]>,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(destination) = output.get_mut(index) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let luma = unsafe { *y_plane.add(y as usize * y_pitch + x as usize) } as f32 / 255.0;
        let chroma_x = (x as f32 - 0.5) * 0.5;
        let chroma_y = (y as f32 - 0.5) * 0.5;
        let (cb, cr) = sample_chroma(
            uv_plane,
            uv_pitch,
            width.div_ceil(2),
            height.div_ceil(2),
            chroma_x,
            chroma_y,
        );
        let yy = (luma - 0.0625).max(0.0) * 1.164_383_5;
        *destination = [
            (yy + 1.792_741_1 * cr).clamp(0.0, 1.0),
            (yy - 0.213_248_61 * cb - 0.532_909_33 * cr).clamp(0.0, 1.0),
            (yy + 2.112_401_7 * cb).clamp(0.0, 1.0),
            1.0,
        ];
    }

    #[kernel]
    pub fn convolution(
        params: ConvolutionParams,
        terms: &[ConvolutionTerm],
        mut output: DisjointSlice<[f32; 4]>,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(destination) = output.get_mut(index) else {
            return;
        };
        let x = i as u32 % params.width;
        let y = i as u32 / params.width;
        let mut result = params.bias;
        for term in terms {
            let mut value = sample(
                params.images[term.input as usize],
                x,
                y,
                params.width,
                params.height,
                term.offset_x,
                term.offset_y,
            );
            if term.activation == ACTIVATION_POSITIVE {
                for component in &mut value {
                    *component = component.max(0.0);
                }
            } else if term.activation == ACTIVATION_NEGATIVE {
                for component in &mut value {
                    *component = (-*component).max(0.0);
                }
            }
            for row in 0..4 {
                result[row] += term.weights[row] * value[0]
                    + term.weights[4 + row] * value[1]
                    + term.weights[8 + row] * value[2]
                    + term.weights[12 + row] * value[3];
            }
        }
        for component in &mut result {
            *component *= params.result_scale;
        }
        if !params.residual.pixels.is_null() {
            let value = sample(params.residual, x, y, params.width, params.height, 0.0, 0.0);
            for component in 0..4 {
                result[component] += value[component] * params.residual_scale;
            }
        }
        *destination = result;
    }

    #[kernel]
    pub fn depth_to_space_x2(
        convolution: ImageDescriptor,
        residual: ImageDescriptor,
        width: u32,
        mut output: DisjointSlice<[f32; 4]>,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let height = output.len() as u32 / width;
        let Some(destination) = output.get_mut(index) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let source_x = x / 2;
        let source_y = y / 2;
        let channel = (y % 2) * 2 + x % 2;
        let convolution_pixel = unsafe {
            *convolution
                .pixels
                .add((source_y * convolution.width + source_x) as usize)
        };
        let residual_pixel = sample(residual, x, y, width, height, 0.0, 0.0);
        let value = convolution_pixel[channel as usize];
        *destination = [
            value + residual_pixel[0],
            value + residual_pixel[1],
            value + residual_pixel[2],
            value + residual_pixel[3],
        ];
    }

    #[kernel]
    pub fn float_to_rgba_opaque(source: *const [f32; 4], mut output: DisjointSlice<u32>) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(destination) = output.get_mut(index) else {
            return;
        };
        let value = unsafe { *source.add(i) };
        *destination = Color::new(value[0], value[1], value[2], 1.0).to_rgba_u32();
    }

    #[kernel]
    pub fn float_to_rgba_alpha(params: AlphaParams, mut output: DisjointSlice<u32>) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(destination) = output.get_mut(index) else {
            return;
        };
        let x = i as u32 % params.width;
        let y = i as u32 / params.width;
        let value = unsafe { *params.source.add(i) };
        let alpha = sample_alpha(
            params.alpha_source,
            params.alpha_pitch,
            params.alpha_width,
            params.alpha_height,
            (x as f32 + 0.5) * params.alpha_width as f32 / params.width as f32 - 0.5,
            (y as f32 + 0.5) * params.alpha_height as f32 / params.height as f32 - 0.5,
        );
        *destination = Color::new(
            value[0].clamp(0.0, alpha),
            value[1].clamp(0.0, alpha),
            value[2].clamp(0.0, alpha),
            alpha,
        )
        .unpremultiply()
        .to_rgba_u32();
    }

    fn sample(
        image: ImageDescriptor,
        x: u32,
        y: u32,
        output_width: u32,
        output_height: u32,
        offset_x: f32,
        offset_y: f32,
    ) -> [f32; 4] {
        let source_x = (x as f32 + 0.5) * image.width as f32 / output_width as f32 - 0.5 + offset_x;
        let source_y =
            (y as f32 + 0.5) * image.height as f32 / output_height as f32 - 0.5 + offset_y;
        let x0 = source_x.floor() as i32;
        let y0 = source_y.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;
        let fx = source_x - x0 as f32;
        let fy = source_y - y0 as f32;
        let top = lerp4(load_pixel(image, x0, y0), load_pixel(image, x1, y0), fx);
        let bottom = lerp4(load_pixel(image, x0, y1), load_pixel(image, x1, y1), fx);
        lerp4(top, bottom, fy)
    }

    fn load_pixel(image: ImageDescriptor, x: i32, y: i32) -> [f32; 4] {
        let x = x.clamp(0, image.width as i32 - 1) as u32;
        let y = y.clamp(0, image.height as i32 - 1) as u32;
        unsafe { *image.pixels.add((y * image.width + x) as usize) }
    }

    fn lerp4(left: [f32; 4], right: [f32; 4], amount: f32) -> [f32; 4] {
        [
            left[0] + (right[0] - left[0]) * amount,
            left[1] + (right[1] - left[1]) * amount,
            left[2] + (right[2] - left[2]) * amount,
            left[3] + (right[3] - left[3]) * amount,
        ]
    }

    fn sample_chroma(
        source: *const u8,
        pitch: usize,
        width: u32,
        height: u32,
        x: f32,
        y: f32,
    ) -> (f32, f32) {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let a = load_chroma(source, pitch, width, height, x0, y0);
        let b = load_chroma(source, pitch, width, height, x0 + 1, y0);
        let c = load_chroma(source, pitch, width, height, x0, y0 + 1);
        let d = load_chroma(source, pitch, width, height, x0 + 1, y0 + 1);
        (
            (a.0 + (b.0 - a.0) * fx) + ((c.0 + (d.0 - c.0) * fx) - (a.0 + (b.0 - a.0) * fx)) * fy,
            (a.1 + (b.1 - a.1) * fx) + ((c.1 + (d.1 - c.1) * fx) - (a.1 + (b.1 - a.1) * fx)) * fy,
        )
    }

    fn load_chroma(
        source: *const u8,
        pitch: usize,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
    ) -> (f32, f32) {
        let x = x.clamp(0, width as i32 - 1) as usize;
        let y = y.clamp(0, height as i32 - 1) as usize;
        let address = unsafe { source.add(y * pitch + x * 2) };
        (
            unsafe { *address } as f32 / 255.0 - 0.5,
            unsafe { *address.add(1) } as f32 / 255.0 - 0.5,
        )
    }

    fn sample_alpha(
        source: *const u8,
        pitch: usize,
        width: u32,
        height: u32,
        x: f32,
        y: f32,
    ) -> f32 {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let load = |column: i32, row: i32| {
            let column = column.clamp(0, width as i32 - 1) as usize;
            let row = row.clamp(0, height as i32 - 1) as usize;
            (unsafe { *source.add(row * pitch + column * 4 + 3) }) as f32 / 255.0
        };
        let top = load(x0, y0) + (load(x0 + 1, y0) - load(x0, y0)) * fx;
        let bottom = load(x0, y0 + 1) + (load(x0 + 1, y0 + 1) - load(x0, y0 + 1)) * fx;
        top + (bottom - top) * fy
    }
}

fn main() {}
