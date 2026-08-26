use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::vignette::VignetteModifier;

#[kernel]
fn vignette(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: f32, _: f32, _: f32) {}

struct Resolved {
    amount: f32,
    midpoint: f32,
    softness: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Vignette"
    }
    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: vignette,
                stream: context.stream(), module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [pass.input_ptr(), width, height, slice_mut(pass.output_buffer()), self.amount, self.midpoint, self.softness]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}
impl RasterModifierRuntime for VignetteModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let resolved = Resolved {
            amount: resolve_scalar(&self.amount, context.evaluation, context.expressions)
                .clamp(0.0, 1.0),
            midpoint: resolve_scalar(&self.midpoint, context.evaluation, context.expressions)
                .clamp(0.0, 1.0),
            softness: resolve_scalar(&self.softness, context.evaluation, context.expressions)
                .clamp(0.0, 1.0),
        };
        input.push_pixel(Box::new(resolved));
        Ok(input)
    }
}
