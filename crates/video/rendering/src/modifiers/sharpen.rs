use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::sharpen::SharpenModifier;

#[kernel]
fn sharpen_blur_horizontal(_: *const u32, _: u32, _: DisjointSlice<u32>, _: u32) {}

#[kernel]
fn sharpen_blur_vertical(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32) {}

#[kernel]
fn unsharp_mask(_: *const u32, _: *const u32, _: DisjointSlice<u32>, _: f32) {}

struct Resolved {
    amount: f32,
    radius: u32,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Sharpen"
    }
    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let launch =
            LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?);
        let mut pass = input.into_pass(context)?;
        let mut scratch = context.take_scratch(count)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Blur)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: sharpen_blur_horizontal,
                stream: context.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), width, slice_mut(&mut scratch), self.radius]
            }
        }
        .map_err(|error| format!("launch horizontal CUDA kernel: {error:?}"))?;
        let scratch_ptr = scratch.cu_deviceptr() as usize as *const u32;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: sharpen_blur_vertical,
                stream: context.stream(), module: &module, config: launch,
                args: [scratch_ptr, width, height, slice_mut(pass.output_buffer()), self.radius]
            }
        }
        .map_err(|error| format!("launch vertical CUDA kernel: {error:?}"))?;
        let blurred = pass.output_buffer().cu_deviceptr() as usize as *const u32;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: unsharp_mask,
                stream: context.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), blurred, slice_mut(&mut scratch), self.amount]
            }
        }
        .map_err(|error| format!("launch unsharp-mask CUDA kernel: {error:?}"))?;
        pass.swap_output(&mut scratch);
        context.recycle_scratch(scratch);
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for SharpenModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let amount =
            resolve_scalar(&self.amount, context.evaluation, context.expressions).clamp(0.0, 2.0);
        let radius = resolve_scalar(&self.radius, context.evaluation, context.expressions)
            .clamp(0.0, 20.0) as u32;
        input.push_pixel(Box::new(Resolved { amount, radius }));
        Ok(input)
    }
}
