use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_render_core::ColorCorrectionParams;

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::color_correction::ColorCorrectionModifier;

#[kernel]
fn color_correction(_: *const u32, _: DisjointSlice<u32>, _: ColorCorrectionParams) {}

struct Resolved(ColorCorrectionParams);

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Color correction"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let count = input.width() as usize * input.height() as usize;
        let launch =
            LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?);
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: color_correction,
                stream: context.stream(),
                module: &module,
                config: launch,
                args: [pass.input_ptr(), slice_mut(pass.output_buffer()), self.0]
            }
        }
        .map_err(|error| format!("launch color-correction CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for ColorCorrectionModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let mut resolve = |value| resolve_scalar(value, context.evaluation, context.expressions);
        input.push_pixel(Box::new(Resolved(ColorCorrectionParams {
            exposure: resolve(&self.exposure).clamp(-10.0, 10.0),
            gamma: resolve(&self.gamma).clamp(0.01, 10.0),
            temperature: resolve(&self.temperature).clamp(-1.0, 1.0),
            tint: resolve(&self.tint).clamp(-1.0, 1.0),
            brightness: resolve(&self.brightness).clamp(-1.0, 1.0),
            contrast: resolve(&self.contrast).clamp(-1.0, 1.0),
            hue_turns: resolve(&self.hue_degrees) / 360.0,
            saturation: resolve(&self.saturation).clamp(0.0, 2.0),
            value: resolve(&self.value).clamp(0.0, 2.0),
        })));
        Ok(input)
    }
}
