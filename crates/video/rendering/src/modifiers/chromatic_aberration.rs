use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_render_core::ChromaticAberrationParams;
use shrimply_video_modifiers::chromatic_aberration::ChromaticAberrationModifier;
#[kernel]
fn chromatic_aberration(
    _: *const u32,
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: ChromaticAberrationParams,
) {
}
struct Resolved([f32; 4]);
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Chromatic aberration"
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
        let params = ChromaticAberrationParams { offsets: self.0 };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: chromatic_aberration,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    h,
                    slice_mut(p.output_buffer()),
                    params
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for ChromaticAberrationModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let mut r = |x| resolve_scalar(x, c.evaluation, c.expressions).clamp(-4096., 4096.);
        input.push_pixel(Box::new(Resolved([
            r(&self.red_offset_x),
            r(&self.red_offset_y),
            r(&self.blue_offset_x),
            r(&self.blue_offset_y),
        ])));
        Ok(input)
    }
}
