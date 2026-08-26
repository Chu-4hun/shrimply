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
    pub fn invert(input: *const u32, mut out: DisjointSlice<u32>, amount: f32) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(d) = out.get_mut(idx) else { return };
        let [r, g, b, a] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        *d = math::Color::new(
            r + (1.0 - 2.0 * r) * amount,
            g + (1.0 - 2.0 * g) * amount,
            b + (1.0 - 2.0 * b) * amount,
            a,
        )
        .to_rgba_u32();
    }
}
