use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::TwirlParams;
use std::sync::Arc;
pub(crate) fn load(c: &Arc<CudaContext>) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(c)
}
#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;
    #[kernel]
    pub fn twirl(
        input: *const u32,
        w: u32,
        h: u32,
        mut out: DisjointSlice<u32>,
        params: TwirlParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(o) = out.get_mut(idx) else {
            return;
        };
        let x = (i as u32 % w) as f32;
        let y = (i as u32 / w) as f32;
        let px = params.center.x * (w - 1) as f32;
        let py = params.center.y * (h - 1) as f32;
        let dx = x - px;
        let dy = y - py;
        let d = (dx * dx + dy * dy).sqrt();
        if d >= params.radius || params.radius <= 0. {
            *o = unsafe { *input.add(i) };
            return;
        }
        let a = math::atan2_f32(dy, dx) + params.angle * (1. - d / params.radius);
        let sx = (px + math::sin_f32(a + core::f32::consts::FRAC_PI_2) * d)
            .round()
            .clamp(0., (w - 1) as f32) as usize;
        let sy = (py + math::sin_f32(a) * d)
            .round()
            .clamp(0., (h - 1) as f32) as usize;
        *o = unsafe { *input.add(sy * w as usize + sx) };
    }
}
