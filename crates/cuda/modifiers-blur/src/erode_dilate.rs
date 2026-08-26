use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, device, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::MorphologyOperation;

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
    pub fn erode_horizontal(
        input: *const u32,
        width: u32,
        mut output: DisjointSlice<f32>,
        radius: u32,
    ) {
        morphology_horizontal(
            input,
            width,
            &mut output,
            radius,
            MorphologyOperation::Erode,
        );
    }

    #[kernel]
    pub fn dilate_horizontal(
        input: *const u32,
        width: u32,
        mut output: DisjointSlice<f32>,
        radius: u32,
    ) {
        morphology_horizontal(
            input,
            width,
            &mut output,
            radius,
            MorphologyOperation::Dilate,
        );
    }

    #[kernel]
    pub fn erode_vertical(
        source: *const u32,
        horizontal: *const f32,
        width: u32,
        height: u32,
        mut output: DisjointSlice<u32>,
        radius: u32,
    ) {
        morphology_vertical(
            source,
            horizontal,
            width,
            height,
            &mut output,
            radius,
            MorphologyOperation::Erode,
        );
    }

    #[kernel]
    pub fn dilate_vertical(
        source: *const u32,
        horizontal: *const f32,
        width: u32,
        height: u32,
        mut output: DisjointSlice<u32>,
        radius: u32,
    ) {
        morphology_vertical(
            source,
            horizontal,
            width,
            height,
            &mut output,
            radius,
            MorphologyOperation::Dilate,
        );
    }

    #[device]
    fn morphology_horizontal(
        input: *const u32,
        width: u32,
        output: &mut DisjointSlice<f32>,
        radius: u32,
        operation: MorphologyOperation,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let x = (i as u32 % width) as i32;
        let y = i / width as usize;
        let radius = radius.min(100) as i32;
        let mut alpha = match operation {
            MorphologyOperation::Erode => 1.0_f32,
            MorphologyOperation::Dilate => 0.0_f32,
        };
        for offset in -radius..=radius {
            let sample_x = (x + offset).clamp(0, width as i32 - 1) as usize;
            let sample_alpha =
                math::Color::from_rgba_u32(unsafe { *input.add(y * width as usize + sample_x) }).a;
            alpha = match operation {
                MorphologyOperation::Erode => alpha.min(sample_alpha),
                MorphologyOperation::Dilate => alpha.max(sample_alpha),
            };
        }
        *output_pixel = alpha;
    }

    #[device]
    fn morphology_vertical(
        source: *const u32,
        horizontal: *const f32,
        width: u32,
        height: u32,
        output: &mut DisjointSlice<u32>,
        radius: u32,
        operation: MorphologyOperation,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output_pixel) = output.get_mut(index) else {
            return;
        };
        let x = i % width as usize;
        let y = (i / width as usize) as i32;
        let radius = radius.min(100) as i32;
        let mut alpha = match operation {
            MorphologyOperation::Erode => 1.0_f32,
            MorphologyOperation::Dilate => 0.0_f32,
        };
        for offset in -radius..=radius {
            let sample_y = (y + offset).clamp(0, height as i32 - 1) as usize;
            let sample_alpha = unsafe { *horizontal.add(sample_y * width as usize + x) };
            alpha = match operation {
                MorphologyOperation::Erode => alpha.min(sample_alpha),
                MorphologyOperation::Dilate => alpha.max(sample_alpha),
            };
        }
        let [red, green, blue, _] =
            math::Color::from_rgba_u32(unsafe { *source.add(i) }).to_array();
        *output_pixel = math::Color::new(red, green, blue, alpha).to_rgba_u32();
    }
}
