use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::directional_blur::DirectionalBlurModifier;
#[kernel]
fn directional_blur(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32, _: f32) {}
struct Resolved {
    radius: u32,
    angle: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Directional blur"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let w = input.width();
        let h = input.height();
        let count = w as usize * h as usize;
        let mut p = input.into_pass(c)?;
        let m = c.modifier_module(crate::gpu::modifiers::ModifierModule::Blur)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: directional_blur,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    h,
                    slice_mut(p.output_buffer()),
                    self.radius,
                    self.angle
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for DirectionalBlurModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        input.push_pixel(Box::new(Resolved {
            radius: resolve_scalar(&self.radius, c.evaluation, c.expressions).clamp(0., 100.)
                as u32,
            angle: resolve_scalar(&self.angle_degrees, c.evaluation, c.expressions).to_radians(),
        }));
        Ok(input)
    }
}
