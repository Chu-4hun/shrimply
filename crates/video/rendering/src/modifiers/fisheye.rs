use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_video_modifiers::fisheye::FisheyeModifier;

#[kernel]
fn fisheye(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: f32, _: glam::Vec2) {}

struct Resolved {
    intensity: f32,
    center: glam::Vec2,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Fisheye"
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
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Geometry)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: fisheye,
                stream: context.stream(), module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [pass.input_ptr(), width, height, slice_mut(pass.output_buffer()), self.intensity, self.center]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for FisheyeModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let center = resolve_vec2(&self.center, context.evaluation, context.expressions)
            .clamp(glam::Vec2::ZERO, glam::Vec2::ONE);
        input.push_pixel(Box::new(Resolved {
            intensity: resolve_scalar(&self.intensity, context.evaluation, context.expressions)
                .clamp(-1.0, 1.0),
            center,
        }));
        Ok(input)
    }
}
