use super::{RasterModifierRuntime, VectorModifierRuntime};
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::opacity::OpacityModifier;

fn opacity(effect: &OpacityModifier, context: &mut VisualModifierContext<'_>) -> f32 {
    resolve_scalar(&effect.opacity, context.evaluation, context.expressions).clamp(0.0, 1.0)
}

impl VectorModifierRuntime for OpacityModifier {
    fn apply_vector(
        &self,
        mut input: crate::layer::VectorVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<crate::layer::VectorVisual, String> {
        input.push(crate::layer::VectorOperation::Opacity(opacity(
            self, context,
        )));
        Ok(input)
    }
}

impl RasterModifierRuntime for OpacityModifier {
    fn apply_raster(
        &self,
        mut input: crate::layer::RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<crate::layer::RasterVisual, String> {
        let opacity = opacity(self, context);
        input.push_spatial(move |state| state.compositing.opacity *= opacity);
        Ok(input)
    }
}
