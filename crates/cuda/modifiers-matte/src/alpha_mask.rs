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
    pub fn alpha_mask(
        input: *const u32,
        params: shrimply_render_core::AlphaMaskParams,
        mut output: DisjointSlice<u32>,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let source = unsafe { *input.add(i) };
        let [red, green, blue, alpha] = math::Color::from_rgba_u32(source).to_array();
        if params.mask.is_null() {
            *output_pixel = math::Color::new(red, green, blue, 0.0).to_rgba_u32();
            return;
        }
        let x = i as u32 % params.input_width;
        let y = i as u32 / params.input_width;
        let mask_x =
            ((x as f32 + 0.5) * params.mask_width as f32 / params.input_width as f32) as u32;
        let mask_y =
            ((y as f32 + 0.5) * params.mask_height as f32 / params.input_height as f32) as u32;
        let mask_pixel = unsafe {
            *params.mask.add(
                (mask_y.min(params.mask_height - 1) * params.mask_width
                    + mask_x.min(params.mask_width - 1)) as usize,
            )
        };
        let mask = math::Color::from_rgba_u32(mask_pixel);
        let amount = mask.rec709_luma() * mask.a;
        *output_pixel = math::Color::new(red, green, blue, alpha * amount).to_rgba_u32();
    }

    #[kernel]
    pub fn shape_alpha_mask(
        input: *const u32,
        params: shrimply_render_core::ShapeAlphaMaskParams,
        mut output: DisjointSlice<u32>,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let x = i as u32 % params.input_width;
        let y = i as u32 / params.input_width;
        let canvas_x = x as f32 + 0.5;
        let canvas_y = y as f32 + 0.5;
        let local =
            math::transform_point2(params.canvas_to_local, glam::Vec2::new(canvas_x, canvas_y));
        let amount = math::shape_alpha_mask_amount(
            local / glam::Vec2::new(params.local_width.max(1.0), params.local_height.max(1.0)),
            &params,
        );
        let [red, green, blue, alpha] =
            math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        if params.base.is_null() {
            *output_pixel = math::Color::new(red, green, blue, alpha * amount).to_rgba_u32();
            return;
        }
        let [base_red, base_green, base_blue, base_alpha] =
            math::Color::from_rgba_u32(unsafe { *params.base.add(i) }).to_array();
        let output_alpha = base_alpha + (alpha - base_alpha) * amount;
        if output_alpha <= f32::EPSILON {
            *output_pixel = 0;
            return;
        }
        let keep = 1.0 - amount;
        *output_pixel = math::Color::new(
            (base_red * base_alpha * keep + red * alpha * amount) / output_alpha,
            (base_green * base_alpha * keep + green * alpha * amount) / output_alpha,
            (base_blue * base_alpha * keep + blue * alpha * amount) / output_alpha,
            output_alpha,
        )
        .to_rgba_u32();
    }
}
