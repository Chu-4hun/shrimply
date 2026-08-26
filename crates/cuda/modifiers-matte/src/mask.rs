use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;

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
    pub fn mask(
        input: *const u32,
        params: shrimply_render_core::MaskParams,
        mut output: DisjointSlice<u32>,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let source = unsafe { *input.add(i) };
        let [red, green, blue, alpha] = math::Color::from_rgba_u32(source).to_array();
        let mut amount = if params.mask.is_null() {
            0.0
        } else {
            let x = i as u32 % params.input_width;
            let y = i as u32 / params.input_width;
            let canvas = math::transform_point2(
                params.transform,
                glam::Vec2::new(x as f32 + 0.5, y as f32 + 0.5),
            );
            let canvas_x = canvas.x;
            let canvas_y = canvas.y;
            if canvas_x < 0.0
                || canvas_y < 0.0
                || canvas_x >= params.mask_width as f32
                || canvas_y >= params.mask_height as f32
            {
                *output_pixel =
                    math::Color::new(red, green, blue, if params.invert { alpha } else { 0.0 })
                        .to_rgba_u32();
                return;
            }
            let mask_x = canvas_x as u32;
            let mask_y = canvas_y as u32;
            let mask_pixel = unsafe {
                *params
                    .mask
                    .add((mask_y * params.mask_width + mask_x) as usize)
            };
            let mask = math::Color::from_rgba_u32(mask_pixel);
            if params.luminance {
                mask.rec709_luma() * mask.a
            } else {
                mask.a
            }
        };
        if params.invert {
            amount = 1.0 - amount;
        }
        *output_pixel = math::Color::new(red, green, blue, alpha * amount).to_rgba_u32();
    }
}
