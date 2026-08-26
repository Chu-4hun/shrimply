use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::{resolve_color, resolve_scalar};
use shrimply_video_modifiers::chroma_key::ChromaKeyModifier;

#[kernel]
fn chroma_key(_: *const u32, _: DisjointSlice<u32>, _: shrimply_render_core::ChromaKeyParams) {}

struct Resolved {
    key: shrimply_math_color::Color,
    similarity: f32,
    softness: f32,
    spill: f32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Chroma key"
    }
    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let count = input.width() as usize * input.height() as usize;
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Matte)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: chroma_key,
                stream: context.stream(), module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [pass.input_ptr(), slice_mut(pass.output_buffer()), shrimply_render_core::ChromaKeyParams {
                    key: self.key,
                    similarity: self.similarity,
                    softness: self.softness,
                    spill: self.spill,
                }]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}
impl RasterModifierRuntime for ChromaKeyModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let key = resolve_color(&self.key_color, context.evaluation, context.expressions);
        let resolved = Resolved {
            key: key.into(),
            similarity: resolve_scalar(&self.similarity, context.evaluation, context.expressions)
                .clamp(0.0, 1.0),
            softness: resolve_scalar(&self.softness, context.evaluation, context.expressions)
                .clamp(0.0, 1.0),
            spill: resolve_scalar(
                &self.spill_suppression,
                context.evaluation,
                context.expressions,
            )
            .clamp(0.0, 1.0),
        };
        input.push_pixel(Box::new(resolved));
        Ok(input)
    }
}
