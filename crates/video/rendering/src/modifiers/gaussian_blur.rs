use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_vec2;
use shrimply_video_modifiers::gaussian_blur::{GaussianBlurChannels, GaussianBlurModifier};

#[kernel]
fn gaussian_blur_horizontal(_: *const u32, _: u32, _: DisjointSlice<u32>, _: u32) {}

#[kernel]
#[expect(
    clippy::too_many_arguments,
    reason = "parameters must match the compiled CUDA kernel ABI"
)]
fn gaussian_blur_vertical(
    _: *const u32,
    _: *const u32,
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: u32,
    _: bool,
    _: bool,
) {
}

struct Resolved {
    radius_x: u32,
    radius_y: u32,
    blur_rgb: bool,
    blur_alpha: bool,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Gaussian blur"
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
                kernel: gaussian_blur_horizontal,
                stream: context.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), width, slice_mut(&mut scratch), self.radius_x]
            }
        }
        .map_err(|error| format!("launch horizontal CUDA kernel: {error:?}"))?;
        let scratch_ptr = scratch.cu_deviceptr() as usize as *const u32;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: gaussian_blur_vertical,
                stream: context.stream(), module: &module, config: launch,
                args: [
                    scratch_ptr,
                    pass.input_ptr(),
                    width,
                    height,
                    slice_mut(pass.output_buffer()),
                    self.radius_y,
                    self.blur_rgb,
                    self.blur_alpha
                ]
            }
        }
        .map_err(|error| format!("launch vertical CUDA kernel: {error:?}"))?;
        context.recycle_scratch(scratch);
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for GaussianBlurModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let radius = resolve_vec2(&self.radius, context.evaluation, context.expressions)
            .clamp(glam::Vec2::ZERO, glam::Vec2::splat(100.0));
        let (blur_rgb, blur_alpha) = match self.channels {
            GaussianBlurChannels::Rgba => (true, true),
            GaussianBlurChannels::Rgb => (true, false),
            GaussianBlurChannels::Alpha => (false, true),
        };
        input.push_pixel(Box::new(Resolved {
            radius_x: radius.x as u32,
            radius_y: radius.y as u32,
            blur_rgb,
            blur_alpha,
        }));
        Ok(input)
    }
}
