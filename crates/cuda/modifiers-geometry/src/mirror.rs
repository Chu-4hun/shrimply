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
    #[kernel]
    pub fn mirror(
        input: *const u32,
        w: u32,
        h: u32,
        mut out: DisjointSlice<u32>,
        horizontal: u32,
        vertical: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(o) = out.get_mut(idx) else {
            return;
        };
        let x = i as u32 % w;
        let y = i as u32 / w;
        let sx = if horizontal != 0 { w - 1 - x } else { x };
        let sy = if vertical != 0 { h - 1 - y } else { y };
        *o = unsafe { *input.add((sy * w + sx) as usize) };
    }
}
