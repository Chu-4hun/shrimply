use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_render_core::ZoomBlurParams;

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_video_modifiers::zoom_blur::ZoomBlurModifier;

#[kernel]
fn zoom_blur(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: ZoomBlurParams) {}

struct Resolved {
    center: glam::Vec2,
    strength: f32,
    samples: u32,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Zoom blur"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Blur)?;
        let params = ZoomBlurParams {
            center: self.center,
            strength: self.strength,
            samples: self.samples,
        };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: zoom_blur,
                stream: context.stream(),
                module: &module,
                config: LaunchConfig::for_num_elems(
                    u32::try_from(count).map_err(|_| "canvas is too large")?
                ),
                args: [
                    pass.input_ptr(),
                    width,
                    height,
                    slice_mut(pass.output_buffer()),
                    params
                ]
            }
        }
        .map_err(|error| format!("launch zoom blur CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for ZoomBlurModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let center = resolve_vec2(&self.center, context.evaluation, context.expressions)
            .clamp(glam::Vec2::ZERO, glam::Vec2::ONE);
        let strength = resolve_scalar(&self.strength, context.evaluation, context.expressions)
            .clamp(-1.0, 1.0);
        let samples = resolve_scalar(&self.samples, context.evaluation, context.expressions)
            .clamp(1.0, 128.0) as u32;
        input.push_pixel(Box::new(Resolved {
            center,
            strength,
            samples,
        }));
        Ok(input)
    }
}
