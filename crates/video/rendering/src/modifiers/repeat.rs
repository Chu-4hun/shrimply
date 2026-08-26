use super::VectorModifierRuntime;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::{resolve_scalar, resolve_vec2};
use shrimply_video_modifiers::repeat::RepeatModifier;
use shrimply_video_modifiers::repeat::RepeatOffsetAxis;

impl VectorModifierRuntime for RepeatModifier {
    fn apply_vector(
        &self,
        mut input: crate::layer::VectorVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<crate::layer::VectorVisual, String> {
        let step = resolve_vec2(&self.step, context.evaluation, context.expressions);
        let copies_x = resolve_scalar(&self.copies_x, context.evaluation, context.expressions)
            .round()
            .max(1.0) as u32;
        let copies_y = resolve_scalar(&self.copies_y, context.evaluation, context.expressions)
            .round()
            .max(1.0) as u32;
        let row_offset = resolve_scalar(&self.row_offset, context.evaluation, context.expressions);
        let row_offset = match self
            .row_offset_axis
            .value_at(context.evaluation.local_time())
        {
            RepeatOffsetAxis::X => glam::Vec2::new(row_offset, 0.0),
            RepeatOffsetAxis::Y => glam::Vec2::new(0.0, row_offset),
        };
        input.push(crate::layer::VectorOperation::Repeat {
            copies_x,
            copies_y,
            step,
            row_offset,
        });
        Ok(input)
    }
}
