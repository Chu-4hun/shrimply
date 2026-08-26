use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::displacement_map::DisplacementMapModifier;
#[kernel]
fn displacement_map(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: f32, _: f32, _: f32) {}
struct Resolved {
    amount: f32,
    scale: f32,
    phase: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Displacement map"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let w = input.width();
        let h = input.height();
        let n = w as usize * h as usize;
        let mut p = input.into_pass(c)?;
        let m = c.modifier_module(crate::gpu::modifiers::ModifierModule::Geometry)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: displacement_map,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    h,
                    slice_mut(p.output_buffer()),
                    self.amount,
                    self.scale,
                    self.phase
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for DisplacementMapModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        input.push_pixel(Box::new(Resolved {
            amount: resolve_scalar(&self.amount, c.evaluation, c.expressions).clamp(-512., 512.),
            scale: resolve_scalar(&self.scale, c.evaluation, c.expressions).clamp(1., 4096.),
            phase: resolve_scalar(&self.phase, c.evaluation, c.expressions),
        }));
        Ok(input)
    }
}
