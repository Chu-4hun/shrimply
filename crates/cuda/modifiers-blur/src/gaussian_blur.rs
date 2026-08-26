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
    pub fn gaussian_blur_horizontal(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        *output = unsafe { math::gaussian_horizontal_rgba(input, i, width, radius) };
    }

    #[kernel]
    pub fn gaussian_blur_vertical(
        input: *const u32,
        original: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
        blur_rgb: bool,
        blur_alpha: bool,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let blurred = math::Color::from_rgba_u32(unsafe {
            math::gaussian_vertical_rgba(input, i, width, height, radius)
        });
        let source = math::Color::from_rgba_u32(unsafe { *original.add(i) });
        *output = math::Color::new(
            if blur_rgb { blurred.r } else { source.r },
            if blur_rgb { blurred.g } else { source.g },
            if blur_rgb { blurred.b } else { source.b },
            if blur_alpha { blurred.a } else { source.a },
        )
        .to_rgba_u32();
    }
}
