use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_math_color::Color;
use shrimply_video_modifiers::glow_bloom::GlowBloomModifier;

#[kernel]
fn glow_bloom_horizontal(_: *const u32, _: u32, _: DisjointSlice<Color>, _: f32, _: u32) {}

#[kernel]
fn glow_bloom_vertical(
    _: *const u32,
    _: *const Color,
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: u32,
    _: f32,
) {
}

struct Resolved {
    threshold: f32,
    radius: u32,
    intensity: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Glow / bloom"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let launch =
            LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?);
        let mut pass = input.into_pass(c)?;
        let mut glow = c.take_typed_scratch::<Color>(count)?;
        let module = c.modifier_module(crate::gpu::modifiers::ModifierModule::Blur)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: glow_bloom_horizontal,
                stream: c.stream(), module: &module, config: launch,
                args: [
                    pass.input_ptr(), width, slice_mut(&mut glow),
                    self.threshold, self.radius
                ]
            }
        }
        .map_err(|error| format!("launch horizontal CUDA kernel: {error:?}"))?;
        let glow_ptr = glow.cu_deviceptr() as usize as *const Color;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: glow_bloom_vertical,
                stream: c.stream(), module: &module, config: launch,
                args: [
                    pass.input_ptr(), glow_ptr, width, height,
                    slice_mut(pass.output_buffer()), self.radius, self.intensity
                ]
            }
        }
        .map_err(|error| format!("launch vertical CUDA kernel: {error:?}"))?;
        c.recycle_typed_scratch(glow);
        Ok(pass.finish(c))
    }
}
impl RasterModifierRuntime for GlowBloomModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        input.push_pixel(Box::new(Resolved {
            threshold: resolve_scalar(&self.threshold, c.evaluation, c.expressions).clamp(0., 1.),
            radius: resolve_scalar(&self.radius, c.evaluation, c.expressions).clamp(0., 32.) as u32,
            intensity: resolve_scalar(&self.intensity, c.evaluation, c.expressions).clamp(0., 5.),
        }));
        Ok(input)
    }
}
