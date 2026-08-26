use super::VectorModifierRuntime;
use crate::layer::{TextMaskOperation, VectorOperation, VectorVisual};
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::text_mask::TextMaskModifier;

impl VectorModifierRuntime for TextMaskModifier {
    fn apply_vector(
        &self,
        mut input: VectorVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<VectorVisual, String> {
        let amount =
            resolve_scalar(&self.amount, context.evaluation, context.expressions).clamp(0.0, 1.0);
        if amount >= 1.0 {
            return Ok(input);
        }
        input.push(VectorOperation::TextMask(TextMaskOperation {
            amount,
            partial_mode: self.partial_mode,
            direction: self.direction,
        }));
        Ok(input)
    }
}
