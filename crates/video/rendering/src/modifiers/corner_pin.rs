use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_render_core::CornerPinParams;
use shrimply_video_modifiers::corner_pin::CornerPinModifier;

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;

#[kernel]
fn corner_pin(_: CornerPinParams, _: DisjointSlice<u32>) {}

struct Resolved {
    inverse_homography: glam::Mat3,
    corners: [glam::Vec2; 4],
    perspective: f32,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Corner pin"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let pixel_count = width as usize * height as usize;
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Geometry)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: corner_pin,
                stream: context.stream(), module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(pixel_count).map_err(|_| "canvas is too large")?),
                args: [CornerPinParams {
                    input: pass.input_ptr(),
                    width,
                    height,
                    inverse_homography: self.inverse_homography,
                    corners: self.corners,
                    perspective: self.perspective,
                }, slice_mut(pass.output_buffer())]
            }
        }
        .map_err(|error| format!("launch CUDA corner pin kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for CornerPinModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let resolve_corner = |value, context: &mut VisualModifierContext<'_>| {
            resolve_vec2(value, context.evaluation, context.expressions)
                .clamp(glam::Vec2::ZERO, glam::Vec2::ONE)
        };
        let targets = [
            resolve_corner(&self.top_left, context),
            resolve_corner(&self.top_right, context),
            resolve_corner(&self.bottom_right, context),
            resolve_corner(&self.bottom_left, context),
        ];
        let perspective =
            resolve_scalar(&self.perspective, context.evaluation, context.expressions)
                .clamp(0.0, 1.0);
        let identity = [
            glam::Vec2::ZERO,
            glam::Vec2::X,
            glam::Vec2::ONE,
            glam::Vec2::Y,
        ];
        if targets == identity {
            return Ok(input);
        }
        let inverse_homography = shrimply_math_geometry::corner_pin_inverse(targets)
            .ok_or("corner pin destination must be a non-degenerate convex quadrilateral")?;
        input.push_pixel(Box::new(Resolved {
            inverse_homography,
            corners: targets,
            perspective,
        }));
        Ok(input)
    }
}
