use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_color;
use shrimply_render_core::ColorizeDuotoneParams;
use shrimply_video_modifiers::colorize_duotone::ColorizeDuotoneModifier;
#[kernel]
fn colorize_duotone(_: *const u32, _: DisjointSlice<u32>, _: ColorizeDuotoneParams) {}
struct Resolved(ColorizeDuotoneParams);
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Colorize / duotone"
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
                kernel: colorize_duotone,
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
impl RasterModifierRuntime for ColorizeDuotoneModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let shadow = resolve_color(&self.shadow_color, c.evaluation, c.expressions);
        let highlight = resolve_color(&self.highlight_color, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved(ColorizeDuotoneParams {
            shadow: shadow.into(),
            highlight: highlight.into(),
        })));
        Ok(input)
    }
}
