use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::ColorizeDuotoneParams;

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
    pub fn colorize_duotone(
        input: *const u32,
        mut out: DisjointSlice<u32>,
        params: ColorizeDuotoneParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let color = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let luminance = color.rec709_luma();
        *output = params
            .shadow
            .lerp(params.highlight, luminance)
            .with_alpha(color.a)
            .to_rgba_u32();
    }
}
