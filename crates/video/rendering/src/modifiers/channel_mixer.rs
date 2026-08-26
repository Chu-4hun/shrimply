use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_render_core::ChannelMixerParams;
use shrimply_video_modifiers::channel_mixer::ChannelMixerModifier;
#[kernel]
fn channel_mixer(_: *const u32, _: DisjointSlice<u32>, _: ChannelMixerParams) {}
struct Resolved(glam::Mat3);
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Channel mixer"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let n = input.width() as usize * input.height() as usize;
        let mut p = input.into_pass(c)?;
        let m = c.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        let params = ChannelMixerParams { matrix: self.0 };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: channel_mixer,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    slice_mut(p.output_buffer()),
                    params
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for ChannelMixerModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let mut r = |x| resolve_scalar(x, c.evaluation, c.expressions).clamp(-2., 2.);
        input.push_pixel(Box::new(Resolved(glam::Mat3::from_cols_array(&[
            r(&self.rr),
            r(&self.gr),
            r(&self.br),
            r(&self.rg),
            r(&self.gg),
            r(&self.bg),
            r(&self.rb),
            r(&self.gb),
            r(&self.bb),
        ]))));
        Ok(input)
    }
}
