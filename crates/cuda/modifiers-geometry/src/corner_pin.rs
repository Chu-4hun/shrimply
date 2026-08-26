use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::CornerPinParams;

pub(crate) fn load(
    context: &Arc<CudaContext>,
) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(context)
}

#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;

    const SOURCE_BOUNDS_EPSILON: f32 = 0.000_01;

    #[kernel]
    pub fn corner_pin(params: CornerPinParams, mut out: DisjointSlice<u32>) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output) = out.get_mut(index) else {
            return;
        };
        let x = (i as u32 % params.width) as f32;
        let y = (i as u32 / params.width) as f32;
        let normalized_x = x / params.width.saturating_sub(1).max(1) as f32;
        let normalized_y = y / params.height.saturating_sub(1).max(1) as f32;
        let Some(projective) = math::projective_point(
            params.inverse_homography,
            math::Vec2::new(normalized_x, normalized_y),
        ) else {
            *output = 0;
            return;
        };
        if projective.x < -SOURCE_BOUNDS_EPSILON
            || projective.x > 1.0 + SOURCE_BOUNDS_EPSILON
            || projective.y < -SOURCE_BOUNDS_EPSILON
            || projective.y > 1.0 + SOURCE_BOUNDS_EPSILON
        {
            *output = 0;
            return;
        }
        let projective = projective.clamp(math::Vec2::ZERO, math::Vec2::ONE);
        let source = if params.perspective >= 1.0 {
            projective
        } else {
            let Some(bilinear) = math::inverse_bilinear_quad(
                params.corners,
                math::Vec2::new(normalized_x, normalized_y),
                projective,
            ) else {
                *output = 0;
                return;
            };
            let perspective = params.perspective.clamp(0.0, 1.0);
            bilinear
                .lerp(projective, perspective)
                .clamp(math::Vec2::ZERO, math::Vec2::ONE)
        };
        *output = unsafe {
            math::sample_bilinear_rgba(
                params.input,
                params.width,
                params.height,
                source.x * params.width.saturating_sub(1) as f32,
                source.y * params.height.saturating_sub(1) as f32,
            )
        };
    }
}
