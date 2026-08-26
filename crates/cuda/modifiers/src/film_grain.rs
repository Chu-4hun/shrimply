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
    fn hash(mut x: u32) -> f32 {
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846ca68b);
        x ^= x >> 16;
        (x as f32) / (u32::MAX as f32)
    }
    #[kernel]
    pub fn film_grain(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<u32>,
        amount: f32,
        size: f32,
        colored: f32,
        seed: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(d) = out.get_mut(idx) else { return };
        let [r, g, b, a] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        let block = size.round().max(1.0) as u32;
        let x = (i as u32 % width) / block;
        let y = (i as u32 / width) / block;
        let base = x
            .wrapping_add(y.wrapping_mul(65537))
            .wrapping_add(seed as u32);
        let n = |s: u32| (hash(base.wrapping_add(s)) - 0.5) * amount;
        let mono = n(0);
        let mix = colored.clamp(0.0, 1.0);
        *d = math::Color::new(
            r + mono * (1.0 - mix) + n(17) * mix,
            g + mono * (1.0 - mix) + n(31) * mix,
            b + mono * (1.0 - mix) + n(47) * mix,
            a,
        )
        .to_rgba_u32();
    }
}
