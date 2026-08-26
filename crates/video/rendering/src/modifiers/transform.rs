use super::{RasterModifierRuntime, VectorModifierRuntime};
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_video_modifiers::transform::TransformModifier;

fn resolved(
    effect: &TransformModifier,
    context: &mut VisualModifierContext<'_>,
) -> shrimply_math_geometry::ResolvedTransform2D {
    let position = resolve_vec2(effect.position(), context.evaluation, context.expressions);
    let scale = resolve_vec2(effect.scale(), context.evaluation, context.expressions);
    shrimply_math_geometry::ResolvedTransform2D {
        position,
        anchor: resolve_vec2(effect.anchor(), context.evaluation, context.expressions),
        scale,
        shear: resolve_vec2(effect.shear(), context.evaluation, context.expressions),
        rotation_degrees: resolve_scalar(
            effect.rotation_degrees(),
            context.evaluation,
            context.expressions,
        ),
    }
}

impl VectorModifierRuntime for TransformModifier {
    fn apply_vector(
        &self,
        mut input: crate::layer::VectorVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<crate::layer::VectorVisual, String> {
        input.push(crate::layer::VectorOperation::Transform(
            resolved(self, context).composed(),
        ));
        Ok(input)
    }
}

impl RasterModifierRuntime for TransformModifier {
    fn apply_raster(
        &self,
        mut input: crate::layer::RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<crate::layer::RasterVisual, String> {
        let modifier = resolved(self, context);
        input.push_spatial(move |state| {
            state.transform = modifier.composed().compose(state.transform);
        });
        Ok(input)
    }
}
