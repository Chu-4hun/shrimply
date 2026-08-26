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
    pub fn posterize(input: *const u32, mut out: DisjointSlice<u32>, levels: f32) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(d) = out.get_mut(idx) else { return };
        let [r, g, b, a] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        let n = (levels.round() - 1.0).max(1.0);
        let q = |v: f32| (v * n).round() / n;
        *d = math::Color::new(q(r), q(g), q(b), a).to_rgba_u32();
    }
}
