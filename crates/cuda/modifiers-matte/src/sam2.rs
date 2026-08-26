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
    use shrimply_render_core::{Sam2MaskParams, Sam2ProxyParams};

    #[kernel]
    pub fn sam2_proxy(input: *const u32, output: *mut u32, params: Sam2ProxyParams) {
        let index = thread::index_1d().get();
        let pixels = params.model_size as usize * params.model_size as usize;
        if index >= pixels || params.input_width == 0 || params.input_height == 0 {
            return;
        }
        let x = index % params.model_size as usize;
        let y = index / params.model_size as usize;
        let source_x = (((x as f32 + 0.5) * params.input_width as f32 / params.model_size as f32)
            - 0.5)
            .clamp(0.0, params.input_width.saturating_sub(1) as f32);
        let source_y = (((y as f32 + 0.5) * params.input_height as f32 / params.model_size as f32)
            - 0.5)
            .clamp(0.0, params.input_height.saturating_sub(1) as f32);
        let pixel = unsafe {
            math::sample_bilinear_rgba(
                input,
                params.input_width,
                params.input_height,
                source_x,
                source_y,
            )
        };
        let [red, green, blue, alpha] = math::Color::from_rgba_u32(pixel).to_array();
        unsafe {
            *output.add(index) =
                math::Color::new(red * alpha, green * alpha, blue * alpha, 1.0).to_rgba_u32()
        };
    }

    #[kernel]
    pub fn sam2_apply_mask(
        input: *const u32,
        masks: *const i8,
        mut output: DisjointSlice<u32>,
        params: Sam2MaskParams,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let x = i % params.output_width as usize;
        let y = i / params.output_width as usize;
        let mask_x = (((x as f32 + 0.5) * params.mask_size as f32 / params.output_width as f32)
            - 0.5)
            .clamp(0.0, params.mask_size.saturating_sub(1) as f32);
        let mask_y = (((y as f32 + 0.5) * params.mask_size as f32 / params.output_height as f32)
            - 0.5)
            .clamp(0.0, params.mask_size.saturating_sub(1) as f32);
        let x0 = math::floor_f32(mask_x) as usize;
        let y0 = math::floor_f32(mask_y) as usize;
        let x1 = (x0 + 1).min(params.mask_size.saturating_sub(1) as usize);
        let y1 = (y0 + 1).min(params.mask_size.saturating_sub(1) as usize);
        let at = |x, y| unsafe {
            *masks.add(y * params.mask_size as usize + x) as f32 / params.quantization_scale
        };
        let top = math::lerp(at(x0, y0), at(x1, y0), mask_x - x0 as f32);
        let bottom = math::lerp(at(x0, y1), at(x1, y1), mask_x - x0 as f32);
        let logit = math::lerp(top, bottom, mask_y - y0 as f32);
        let half_softness = params.softness.max(0.0) * 0.5;
        let selected = if half_softness > 0.0 {
            math::smoothstep(
                params.threshold - half_softness,
                params.threshold + half_softness,
                logit,
            )
        } else if logit > params.threshold {
            1.0
        } else {
            0.0
        };
        let keep = if params.invert {
            1.0 - selected
        } else {
            selected
        };
        let [red, green, blue, alpha] =
            math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        *output_pixel = math::Color::new(red, green, blue, alpha * keep).to_rgba_u32();
    }
}
