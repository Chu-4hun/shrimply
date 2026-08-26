use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_color, resolve_scalar};
use shrimply_video_modifiers::alpha_outline::AlphaOutlineModifier;

#[kernel]
fn alpha_outline_horizontal(_: *const u32, _: u32, _: DisjointSlice<f32>, _: u32) {}

#[kernel]
fn alpha_outline_vertical(
    _: *const u32,
    _: *const f32,
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: u32,
    _: u32,
) {
}

struct Resolved {
    radius: u32,
    color: u32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Alpha outline"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let launch =
            LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?);
        let mut pass = input.into_pass(c)?;
        let mut alpha = c.take_typed_scratch::<f32>(count)?;
        let module = c.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: alpha_outline_horizontal,
                stream: c.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), width, slice_mut(&mut alpha), self.radius]
            }
        }
        .map_err(|error| format!("launch horizontal CUDA kernel: {error:?}"))?;
        let alpha_ptr = alpha.cu_deviceptr() as usize as *const f32;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: alpha_outline_vertical,
                stream: c.stream(), module: &module, config: launch,
                args: [
                    pass.input_ptr(), alpha_ptr, width, height,
                    slice_mut(pass.output_buffer()), self.radius, self.color
                ]
            }
        }
        .map_err(|error| format!("launch vertical CUDA kernel: {error:?}"))?;
        c.recycle_typed_scratch(alpha);
        Ok(pass.finish(c))
    }
}
impl RasterModifierRuntime for AlphaOutlineModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let color = resolve_color(&self.color, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved {
            radius: resolve_scalar(&self.width, c.evaluation, c.expressions).clamp(0., 32.) as u32,
            color: color.r as u32
                | (color.g as u32) << 8
                | (color.b as u32) << 16
                | (color.a as u32) << 24,
        }));
        Ok(input)
    }
}
