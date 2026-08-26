use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::ColorCorrectionParams;

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
    pub fn color_correction(
        input: *const u32,
        mut out: DisjointSlice<u32>,
        params: ColorCorrectionParams,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output) = out.get_mut(index) else {
            return;
        };
        *output = math::Color::from_rgba_u32(unsafe { *input.add(i) })
            .corrected(params)
            .to_rgba_u32();
    }
}
