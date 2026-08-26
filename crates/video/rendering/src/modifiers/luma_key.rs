use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::luma_key::LumaKeyModifier;

#[kernel]
fn luma_key(_: *const u32, _: DisjointSlice<u32>, _: f32, _: f32, _: bool) {}

struct Resolved {
    threshold: f32,
    softness: f32,
    invert: bool,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Luma key"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let count = input.width() as usize * input.height() as usize;
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Matte)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: luma_key,
                stream: context.stream(),
                module: &module,
                config: LaunchConfig::for_num_elems(
                    u32::try_from(count).map_err(|_| "canvas is too large")?
                ),
                args: [
                    pass.input_ptr(),
                    slice_mut(pass.output_buffer()),
                    self.threshold,
                    self.softness,
                    self.invert
                ]
            }
        }
        .map_err(|error| format!("launch luma key CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for LumaKeyModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        input.push_pixel(Box::new(Resolved {
            threshold: resolve_scalar(&self.threshold, context.evaluation, context.expressions)
                .clamp(0.0, 1.0),
            softness: resolve_scalar(&self.softness, context.evaluation, context.expressions)
                .clamp(0.0, 1.0),
            invert: self.invert,
        }));
        Ok(input)
    }
}
