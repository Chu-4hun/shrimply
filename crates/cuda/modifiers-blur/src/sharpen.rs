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
    use crate::math;
    #[kernel]
    pub fn sharpen_blur_horizontal(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(pixel) = out.get_mut(idx) else {
            return;
        };
        *pixel = unsafe { math::gaussian_horizontal_rgba(input, i, width, radius) };
    }
    #[kernel]
    pub fn sharpen_blur_vertical(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(pixel) = out.get_mut(idx) else {
            return;
        };
        *pixel = unsafe { math::gaussian_vertical_rgba(input, i, width, height, radius) };
    }
    #[kernel]
    pub fn unsharp_mask(
        original: *const u32,
        blurred: *const u32,
        mut out: DisjointSlice<u32>,
        amount: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(pixel) = out.get_mut(idx) else {
            return;
        };
        let [r, g, b, a] = math::Color::from_rgba_u32(unsafe { *original.add(i) }).to_array();
        let [br, bg, bb, _] = math::Color::from_rgba_u32(unsafe { *blurred.add(i) }).to_array();
        *pixel = math::Color::new(
            r + (r - br) * amount,
            g + (g - bg) * amount,
            b + (b - bb) * amount,
            a,
        )
        .to_rgba_u32();
    }
}
