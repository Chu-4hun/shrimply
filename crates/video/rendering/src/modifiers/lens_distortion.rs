use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_video_modifiers::lens_distortion::LensDistortionModifier;
#[kernel]
fn lens_distortion(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: f32, _: glam::Vec2) {}
struct Resolved {
    distortion: f32,
    center: glam::Vec2,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Lens distortion"
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
                kernel: lens_distortion,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    h,
                    slice_mut(p.output_buffer()),
                    self.distortion,
                    self.center
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for LensDistortionModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let center = resolve_vec2(&self.center, c.evaluation, c.expressions)
            .clamp(glam::Vec2::ZERO, glam::Vec2::ONE);
        input.push_pixel(Box::new(Resolved {
            distortion: resolve_scalar(&self.distortion, c.evaluation, c.expressions)
                .clamp(-2., 2.),
            center,
        }));
        Ok(input)
    }
}
