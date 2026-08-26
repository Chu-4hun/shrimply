use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::film_grain::FilmGrainModifier;
#[kernel]
fn film_grain(_: *const u32, _: u32, _: DisjointSlice<u32>, _: f32, _: f32, _: f32, _: f32) {}
struct Resolved {
    amount: f32,
    size: f32,
    colored: f32,
    seed: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Film grain"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let w = input.width();
        let n = w as usize * input.height() as usize;
        let mut p = input.into_pass(c)?;
        let m = c.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: film_grain,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    slice_mut(p.output_buffer()),
                    self.amount,
                    self.size,
                    self.colored,
                    self.seed
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for FilmGrainModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let mut r = |x| resolve_scalar(x, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved {
            amount: r(&self.amount).clamp(0., 2.),
            size: r(&self.size).clamp(1., 256.),
            colored: r(&self.colored).clamp(0., 1.),
            seed: r(&self.seed),
        }));
        Ok(input)
    }
}
