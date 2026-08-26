use super::VectorModifierRuntime;
use crate::layer::{VectorOperation, VectorVisual};
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::hsv::HsvModifier;

impl VectorModifierRuntime for HsvModifier {
    fn apply_vector(
        &self,
        mut input: VectorVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<VectorVisual, String> {
        let mut resolve = |value| resolve_scalar(value, context.evaluation, context.expressions);
        input.push(VectorOperation::Hsv {
            hue_turns: resolve(&self.hue_degrees) / 360.0,
            saturation: resolve(&self.saturation).clamp(0.0, 2.0),
            value: resolve(&self.value).clamp(0.0, 2.0),
        });
        Ok(input)
    }
}
