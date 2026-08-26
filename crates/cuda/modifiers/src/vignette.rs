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
    pub fn vignette(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        amount: f32,
        midpoint: f32,
        softness: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        *output = apply(
            unsafe { *input.add(i) },
            i,
            width,
            height,
            amount,
            midpoint,
            softness,
        );
    }

    fn apply(
        pixel: u32,
        index: usize,
        width: u32,
        height: u32,
        amount: f32,
        midpoint: f32,
        softness: f32,
    ) -> u32 {
        let [r, g, b, a] = math::Color::from_rgba_u32(pixel).to_array();
        let x = (index as u32 % width) as f32 / width.max(1) as f32 * 2.0 - 1.0;
        let y = (index as u32 / width) as f32 / height.max(1) as f32 * 2.0 - 1.0;
        let distance = (x * x + y * y).sqrt() / core::f32::consts::SQRT_2;
        let edge = math::smoothstep(
            midpoint.clamp(0.0, 1.0),
            (midpoint + softness).clamp(0.000_01, 1.000_01),
            distance,
        );
        let shade = 1.0 - amount.clamp(0.0, 1.0) * edge;
        math::Color::new(r * shade, g * shade, b * shade, a).to_rgba_u32()
    }
}
