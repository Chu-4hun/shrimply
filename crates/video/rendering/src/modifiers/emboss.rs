use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::emboss::EmbossModifier;
#[kernel]
fn emboss(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: f32, _: f32, _: f32) {}
struct Resolved {
    direction: f32,
    depth: f32,
    amount: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Emboss"
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
                kernel: emboss,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    h,
                    slice_mut(p.output_buffer()),
                    self.direction,
                    self.depth,
                    self.amount
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for EmbossModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let mut r = |x| resolve_scalar(x, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved {
            direction: r(&self.direction_degrees),
            depth: r(&self.depth).clamp(0., 10.),
            amount: r(&self.amount).clamp(0., 1.),
        }));
        Ok(input)
    }
}
