use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::ThresholdParams;
use std::sync::Arc;
pub(crate) fn load(c: &Arc<CudaContext>) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(c)
}
#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;
    #[kernel]
    pub fn threshold(input: *const u32, mut out: DisjointSlice<u32>, params: ThresholdParams) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(d) = out.get_mut(idx) else { return };
        let input = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let color = if input.rec709_luma() >= params.threshold {
            params.high
        } else {
            params.low
        };
        *d = color.with_alpha(input.a).to_rgba_u32();
    }
}
