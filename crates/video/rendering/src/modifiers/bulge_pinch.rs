use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_render_core::BulgePinchParams;
use shrimply_video_modifiers::bulge_pinch::BulgePinchModifier;
#[kernel]
fn bulge_pinch(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: BulgePinchParams) {}
struct Resolved {
    center: glam::Vec2,
    radius: f32,
    strength: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Bulge / pinch"
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
        let params = BulgePinchParams {
            center: self.center,
            radius: self.radius * w.min(h) as f32,
            strength: self.strength,
        };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: bulge_pinch,
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
impl RasterModifierRuntime for BulgePinchModifier {
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
            strength: resolve_scalar(&self.strength, c.evaluation, c.expressions).clamp(-1., 1.),
        }));
        Ok(input)
    }
}
