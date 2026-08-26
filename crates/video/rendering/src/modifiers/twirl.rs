use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_render_core::TwirlParams;
use shrimply_video_modifiers::twirl::TwirlModifier;
#[kernel]
fn twirl(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: TwirlParams) {}
struct Resolved {
    center: glam::Vec2,
    radius: f32,
    angle: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Twirl"
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
        let params = TwirlParams {
            center: self.center,
            radius: self.radius * w.min(h) as f32,
            angle: self.angle,
        };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: twirl,
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
impl RasterModifierRuntime for TwirlModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let center = resolve_vec2(&self.center, c.evaluation, c.expressions)
            .clamp(glam::Vec2::ZERO, glam::Vec2::ONE);
        input.push_pixel(Box::new(Resolved {
            center,
            radius: resolve_scalar(&self.radius, c.evaluation, c.expressions).clamp(0.0, 1.0),
            angle: resolve_scalar(&self.angle_degrees, c.evaluation, c.expressions).to_radians(),
        }));
        Ok(input)
    }
}
