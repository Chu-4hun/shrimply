use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
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
    pub fn alpha_outline_horizontal(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<f32>,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i % width as usize;
        let row = i - x;
        let start = x.saturating_sub(radius as usize);
        let end = (x + radius as usize).min(width as usize - 1);
        let mut alpha: f32 = 0.0;
        for sample_x in start..=end {
            alpha = alpha.max(math::Color::from_rgba_u32(unsafe { *input.add(row + sample_x) }).a);
        }
        *output = alpha;
    }

    #[kernel]
    pub fn alpha_outline_vertical(
        input: *const u32,
        horizontal: *const f32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
        color: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let base = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let x = i % width as usize;
        let y = i / width as usize;
        let start = y.saturating_sub(radius as usize);
        let end = (y + radius as usize).min(height as usize - 1);
        let mut alpha: f32 = 0.0;
        for sample_y in start..=end {
            alpha = alpha.max(unsafe { *horizontal.add(sample_y * width as usize + x) });
        }

        let outline = math::Color::from_rgba_u32(color);
        let outline_alpha = outline.a * alpha * (1.0 - base.a);
        let output_alpha = base.a + outline_alpha;
        let inverse_alpha = if output_alpha > 0.0 {
            1.0 / output_alpha
        } else {
            0.0
        };
        *output = math::Color::new(
            (base.r * base.a + outline.r * outline_alpha) * inverse_alpha,
            (base.g * base.a + outline.g * outline_alpha) * inverse_alpha,
            (base.b * base.a + outline.b * outline_alpha) * inverse_alpha,
            output_alpha,
        )
        .to_rgba_u32();
    }
}
