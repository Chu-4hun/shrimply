use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::WaveRippleParams;
use std::sync::Arc;
pub(crate) fn load(c: &Arc<CudaContext>) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(c)
}
#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;
    #[kernel]
    pub fn wave_ripple(
        input: *const u32,
        w: u32,
        h: u32,
        mut out: DisjointSlice<u32>,
        params: WaveRippleParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(o) = out.get_mut(idx) else {
            return;
        };
        let x = (i as u32 % w) as f32;
        let y = (i as u32 / w) as f32;
        let c = math::sin_f32(params.angle + core::f32::consts::FRAC_PI_2);
        let s = math::sin_f32(params.angle);
        let displacement = params.amplitude
            * math::sin_f32(
                (x * c + y * s) / params.wavelength * core::f32::consts::TAU + params.phase,
            );
        let sx = (x - s * displacement).round().clamp(0., (w - 1) as f32) as usize;
        let sy = (y + c * displacement).round().clamp(0., (h - 1) as f32) as usize;
        *o = unsafe { *input.add(sy * w as usize + sx) };
    }
}
