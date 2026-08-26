use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::DropShadowParams;
use std::sync::Arc;

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
    pub fn drop_shadow_horizontal(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<f32>,
        params: DropShadowParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = (i % width as usize) as i32 - params.offset.x.round() as i32;
        let y = (i / width as usize) as i32 - params.offset.y.round() as i32;
        let radius = params.radius as i32;
        let mut alpha = 0.0;
        for dx in -radius..=radius {
            let sample_x = x + dx;
            if sample_x >= 0 && sample_x < width as i32 && y >= 0 && y < height as i32 {
                alpha += math::Color::from_rgba_u32(unsafe {
                    *input.add(y as usize * width as usize + sample_x as usize)
                })
                .a;
            }
        }
        *output = alpha / (2 * radius + 1) as f32;
    }

    #[kernel]
    pub fn drop_shadow_vertical(
        input: *const u32,
        horizontal: *const f32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        params: DropShadowParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i % width as usize;
        let y = i / width as usize;
        let radius = params.radius as i32;
        let mut alpha = 0.0;
        for dy in -radius..=radius {
            let sample_y = y as i32 + dy;
            if sample_y >= 0 && sample_y < height as i32 {
                alpha += unsafe { *horizontal.add(sample_y as usize * width as usize + x) };
            }
        }
        alpha /= (2 * radius + 1) as f32;

        let base = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let shadow = math::Color::from_rgba_u32(params.color);
        let shadow_alpha = shadow.a * alpha;
        let shadow_weight = shadow_alpha * (1.0 - base.a);
        let output_alpha = base.a + shadow_weight;
        let inverse_alpha = if output_alpha > 0.0 {
            1.0 / output_alpha
        } else {
            0.0
        };
        *output = math::Color::new(
            (base.r * base.a + shadow.r * shadow_weight) * inverse_alpha,
            (base.g * base.a + shadow.g * shadow_weight) * inverse_alpha,
            (base.b * base.a + shadow.b * shadow_weight) * inverse_alpha,
            output_alpha,
        )
        .to_rgba_u32();
    }
}
