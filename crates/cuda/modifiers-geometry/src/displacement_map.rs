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
    use crate::math;
    #[kernel]
    pub fn displacement_map(
        input: *const u32,
        w: u32,
        h: u32,
        mut out: DisjointSlice<u32>,
        amount: f32,
        scale: f32,
        phase: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(o) = out.get_mut(idx) else {
            return;
        };
        let x = (i as u32 % w) as f32;
        let y = (i as u32 / w) as f32;
        let frequency = core::f32::consts::TAU / scale;
        let dx = amount * math::sin_f32(y * frequency + phase);
        let dy = amount * math::sin_f32(x * frequency + phase * 1.37);
        let sx = (x + dx).round().clamp(0., (w - 1) as f32) as usize;
        let sy = (y + dy).round().clamp(0., (h - 1) as f32) as usize;
        *o = unsafe { *input.add(sy * w as usize + sx) };
    }
}
