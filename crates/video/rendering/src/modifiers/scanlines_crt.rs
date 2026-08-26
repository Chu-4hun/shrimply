use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_render_core::ScanlinesCrtParams;
use shrimply_video_modifiers::scanlines_crt::ScanlinesCrtModifier;
#[kernel]
fn scanlines_crt(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: ScanlinesCrtParams) {}
struct Resolved {
    spacing: f32,
    intensity: f32,
    curvature: f32,
    mask: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Scanlines / CRT"
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
        let params = ScanlinesCrtParams {
            spacing: self.spacing,
            intensity: self.intensity,
            curvature: self.curvature,
            mask: self.mask,
        };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: scanlines_crt,
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
impl RasterModifierRuntime for ScanlinesCrtModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let mut r = |x| resolve_scalar(x, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved {
            spacing: r(&self.spacing).clamp(1., 100.),
            intensity: r(&self.intensity).clamp(0., 1.),
            curvature: r(&self.curvature).clamp(0., 2.),
            mask: r(&self.mask_strength).clamp(0., 1.),
        }));
        Ok(input)
    }
}
