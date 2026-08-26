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
    use shrimply_render_core::AffineStabilizationParams;

    #[kernel]
    pub fn affine_stabilization(params: AffineStabilizationParams, mut out: DisjointSlice<u32>) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output) = out.get_mut(index) else {
            return;
        };
        let x = (i as u32 % params.width) as f32;
        let y = (i as u32 / params.width) as f32;
        let source = math::transform_point2(params.source_transform, glam::Vec2::new(x, y));
        let source_x = source.x;
        let source_y = source.y;
        if source_x < 0.0
            || source_y < 0.0
            || source_x > params.width.saturating_sub(1) as f32
            || source_y > params.height.saturating_sub(1) as f32
        {
            *output = 0;
            return;
        }
        *output = unsafe {
            math::sample_bilinear_rgba(
                params.input,
                params.width,
                params.height,
                source_x,
                source_y,
            )
        };
    }
}
