use super::VectorModifierRuntime;
use crate::layer::{VectorOperation, VectorVisual};
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::shaky_path::ShakyPathModifier;

impl VectorModifierRuntime for ShakyPathModifier {
    fn apply_vector(
        &self,
        mut input: VectorVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<VectorVisual, String> {
        let amplitude =
            resolve_scalar(&self.amplitude, context.evaluation, context.expressions).max(0.0);
        if amplitude <= f32::EPSILON {
            return Ok(input);
        }
        let step_size =
            resolve_scalar(&self.step_size, context.evaluation, context.expressions).max(0.1);
        let evolution = resolve_scalar(&self.evolution, context.evaluation, context.expressions);
        let seed = resolve_scalar(&self.seed, context.evaluation, context.expressions)
            .round()
            .clamp(0.0, u32::MAX as f32) as u32;
        input.push(VectorOperation::ShakyPath {
            amplitude,
            step_size,
            seed: crate::math::shaky_path_seed(seed, evolution),
        });
        Ok(input)
    }
}
