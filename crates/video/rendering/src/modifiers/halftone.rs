use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_render_core::HalftoneParams;
use shrimply_video_modifiers::halftone::{HalftoneMode, HalftoneModifier};
#[kernel]
fn halftone(_: *const u32, _: u32, _: DisjointSlice<u32>, _: HalftoneParams) {}
struct Resolved {
    size: f32,
    angle: f32,
    contrast: f32,
    mode: HalftoneMode,
    channel_offset: f32,
    channel_angle_offset: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Halftone"
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
        let params = HalftoneParams {
            size: self.size,
            angle: self.angle,
            contrast: self.contrast,
            mode: self.mode as u32,
            channel_offset: self.channel_offset,
            channel_angle_offset: self.channel_angle_offset,
        };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: halftone,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [p.input_ptr(), w, slice_mut(p.output_buffer()), params]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for HalftoneModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let mut r = |x| resolve_scalar(x, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved {
            size: r(&self.size).clamp(1., 1024.),
            angle: r(&self.angle_degrees),
            contrast: r(&self.contrast).clamp(0., 10.),
            mode: self.mode.value_at(c.evaluation.local_time()),
            channel_offset: r(&self.rgb_distance).clamp(0.0, 1024.0),
            channel_angle_offset: r(&self.channel_angle_offset),
        }));
        Ok(input)
    }
}
