use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_color, resolve_scalar};
use shrimply_render_core::ThresholdParams;
use shrimply_video_modifiers::threshold::ThresholdModifier;
#[kernel]
fn threshold(_: *const u32, _: DisjointSlice<u32>, _: ThresholdParams) {}
struct Resolved(ThresholdParams);
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Threshold"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let n = input.width() as usize * input.height() as usize;
        let mut p = input.into_pass(c)?;
        let m = c.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: threshold,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    slice_mut(p.output_buffer()),
                    self.0
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for ThresholdModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let low = resolve_color(&self.low_color, c.evaluation, c.expressions);
        let high = resolve_color(&self.high_color, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved(ThresholdParams {
            threshold: resolve_scalar(&self.threshold, c.evaluation, c.expressions).clamp(0.0, 1.0),
            low: low.into(),
            high: high.into(),
        })));
        Ok(input)
    }
}
