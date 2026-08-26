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
    pub fn luma_key(
        input: *const u32,
        mut output: DisjointSlice<u32>,
        threshold: f32,
        softness: f32,
        invert: bool,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let color = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let luma = color.rec709_luma();
        let half_softness = softness.max(0.000_01) * 0.5;
        let mut keep = math::smoothstep(threshold - half_softness, threshold + half_softness, luma);
        if invert {
            keep = 1.0 - keep;
        }
        *output_pixel = color.with_alpha(color.a * keep).to_rgba_u32();
    }
}
