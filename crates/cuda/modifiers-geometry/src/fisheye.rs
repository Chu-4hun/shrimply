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
    pub fn fisheye(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        intensity: f32,
        center: math::Vec2,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = (i as u32 % width) as f32;
        let y = (i as u32 / width) as f32;
        let size = math::Vec2::new(width as f32, height as f32).max(math::Vec2::ONE);
        let center = center.clamp(math::Vec2::ZERO, math::Vec2::ONE) * (size - math::Vec2::ONE);
        let offset = (math::Vec2::new(x, y) - center) / size;
        let scale =
            1.0 - intensity.clamp(-1.0, 1.0) * (1.0 - offset.length_squared() * 4.0).max(0.0);
        let source = center + offset * scale * size;
        let source_x = source.x.round().clamp(0.0, width.saturating_sub(1) as f32) as usize;
        let source_y = source.y.round().clamp(0.0, height.saturating_sub(1) as f32) as usize;
        *output = unsafe { *input.add(source_y * width as usize + source_x) };
    }
}
