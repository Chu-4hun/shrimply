use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use std::sync::Arc;
pub(crate) fn load(c: &Arc<CudaContext>) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(c)
}
#[cuda_module]
pub(crate) mod device {
    use super::*;
    #[kernel]
    pub fn lens_distortion(
        input: *const u32,
        w: u32,
        h: u32,
        mut out: DisjointSlice<u32>,
        distortion: f32,
        center: crate::math::Vec2,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(o) = out.get_mut(idx) else {
            return;
        };
        let x = (i as u32 % w) as f32;
        let y = (i as u32 / w) as f32;
        let px = center.x * (w - 1) as f32;
        let py = center.y * (h - 1) as f32;
        let nx = (x - px) / w as f32;
        let ny = (y - py) / h as f32;
        let scale = 1. + distortion * (nx * nx + ny * ny) * 4.;
        let sx = (px + nx * scale * w as f32).round() as i32;
        let sy = (py + ny * scale * h as f32).round() as i32;
        *o = if sx >= 0 && sy >= 0 && sx < w as i32 && sy < h as i32 {
            unsafe { *input.add(sy as usize * w as usize + sx as usize) }
        } else {
            0
        };
    }
}
