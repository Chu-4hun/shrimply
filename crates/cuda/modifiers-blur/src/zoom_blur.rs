use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::ZoomBlurParams;

pub(crate) fn load(
    context: &Arc<CudaContext>,
) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(context)
}

#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;

    #[kernel]
    pub fn zoom_blur(
        input: *const u32,
        width: u32,
        height: u32,
        mut output: DisjointSlice<u32>,
        params: ZoomBlurParams,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let x = (i as u32 % width) as f32;
        let y = (i as u32 / width) as f32;
        let center_x = params.center.x * width.saturating_sub(1) as f32;
        let center_y = params.center.y * height.saturating_sub(1) as f32;
        let sample_count = params.samples.clamp(1, 128);
        let mut sum = [0.0; 4];

        for sample in 0..sample_count {
            let fraction = if sample_count == 1 {
                0.0
            } else {
                sample as f32 / (sample_count - 1) as f32
            };
            let scale = params.strength * fraction;
            let sample_x = x + (center_x - x) * scale;
            let sample_y = y + (center_y - y) * scale;
            let color = unsafe { sample_bilinear(input, width, height, sample_x, sample_y) };
            sum[0] += color.r;
            sum[1] += color.g;
            sum[2] += color.b;
            sum[3] += color.a;
        }

        let count = sample_count as f32;
        let alpha = sum[3] / count;
        let color_divisor = sum[3].max(0.000_01);
        *output_pixel = math::Color::new(
            sum[0] / color_divisor,
            sum[1] / color_divisor,
            sum[2] / color_divisor,
            alpha,
        )
        .to_rgba_u32();
    }

    unsafe fn sample_bilinear(
        input: *const u32,
        width: u32,
        height: u32,
        x: f32,
        y: f32,
    ) -> math::Color<f32> {
        let x = x.clamp(0.0, width.saturating_sub(1) as f32);
        let y = y.clamp(0.0, height.saturating_sub(1) as f32);
        let x0 = math::floor_f32(x) as usize;
        let y0 = math::floor_f32(y) as usize;
        let x1 = (x0 + 1).min(width.saturating_sub(1) as usize);
        let y1 = (y0 + 1).min(height.saturating_sub(1) as usize);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let top_left = math::Color::from_rgba_u32(unsafe { *input.add(y0 * width as usize + x0) });
        let top_right = math::Color::from_rgba_u32(unsafe { *input.add(y0 * width as usize + x1) });
        let bottom_left =
            math::Color::from_rgba_u32(unsafe { *input.add(y1 * width as usize + x0) });
        let bottom_right =
            math::Color::from_rgba_u32(unsafe { *input.add(y1 * width as usize + x1) });
        top_left
            .premultiply()
            .lerp(top_right.premultiply(), fx)
            .lerp(
                bottom_left
                    .premultiply()
                    .lerp(bottom_right.premultiply(), fx),
                fy,
            )
    }
}
