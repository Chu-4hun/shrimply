use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;

pub(crate) fn load(
    context: &Arc<CudaContext>,
) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(context)
}

#[cuda_module]
pub(crate) mod device {
    use super::*;
    use shrimply_render_core::TransparentFillMaskParams;

    #[kernel]
    pub fn transparent_fill_apply_mask(
        input: *const u32,
        mask: *const u8,
        mut output: DisjointSlice<u32>,
        params: TransparentFillMaskParams,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let x = i % params.width as usize;
        let y = i / params.width as usize;
        if y >= params.height as usize {
            return;
        }
        let byte = unsafe { *mask.add(y * params.stride as usize + x / 8) };
        *output_pixel = if byte & (0x80 >> (x % 8)) != 0 {
            0
        } else {
            unsafe { *input.add(i) }
        };
    }
}
