use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use std::sync::Arc;
pub(crate) fn load(
    context: &Arc<CudaContext>,
) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(context)
}
#[cuda_module]
pub(crate) mod device {
    use super::*;
    #[kernel]
    pub fn pixelate_mosaic(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        block_width: u32,
        block_height: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let bw = block_width.max(1);
        let bh = block_height.max(1);
        let sx = (x / bw * bw + bw / 2).min(width - 1);
        let sy = (y / bh * bh + bh / 2).min(height - 1);
        *output = unsafe { *input.add((sy * width + sx) as usize) };
    }
}
