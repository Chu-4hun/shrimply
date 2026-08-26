use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_render_core::WaveRippleParams;
use shrimply_video_modifiers::wave_ripple::WaveRippleModifier;
#[kernel]
fn wave_ripple(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: WaveRippleParams) {}
struct Resolved {
    amplitude: f32,
    wavelength: f32,
    angle: f32,
    phase: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Wave / ripple"
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
        let params = WaveRippleParams {
            amplitude: self.amplitude,
            wavelength: self.wavelength,
            angle: self.angle,
            phase: self.phase,
        };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: wave_ripple,
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
impl RasterModifierRuntime for WaveRippleModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        input.push_pixel(Box::new(Resolved {
            amplitude: resolve_scalar(&self.amplitude, c.evaluation, c.expressions)
                .clamp(-512., 512.),
            wavelength: resolve_scalar(&self.wavelength, c.evaluation, c.expressions)
                .clamp(1., 4096.),
            angle: resolve_scalar(&self.angle_degrees, c.evaluation, c.expressions).to_radians(),
            phase: resolve_scalar(&self.phase, c.evaluation, c.expressions),
        }));
        Ok(input)
    }
}
