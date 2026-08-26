use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_color, resolve_scalar};
use shrimply_render_core::EdgeDetectionParams;
use shrimply_video_modifiers::edge_detection::EdgeDetectionModifier;
#[kernel]
fn edge_detection(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: EdgeDetectionParams) {}
struct Resolved(EdgeDetectionParams);
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Edge detection"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let (w, h) = (input.width(), input.height());
        let n = w as usize * h as usize;
        let mut p = input.into_pass(c)?;
        let m = c.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: edge_detection,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    h,
                    slice_mut(p.output_buffer()),
                    self.0
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for EdgeDetectionModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let edge = resolve_color(&self.edge_color, c.evaluation, c.expressions);
        let background = resolve_color(&self.background_color, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved(EdgeDetectionParams {
            amount: resolve_scalar(&self.amount, c.evaluation, c.expressions).clamp(0.0, 1.0),
            edge: edge.into(),
            background: background.into(),
        })));
        Ok(input)
    }
}
