use super::VectorModifierRuntime;
use crate::layer::VectorVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_video_modifiers::path_offset::PathOffsetModifier;

impl VectorModifierRuntime for PathOffsetModifier {
    fn apply_vector(
        &self,
        input: VectorVisual,
        _context: &mut VisualModifierContext<'_>,
    ) -> Result<VectorVisual, String> {
        Ok(input)
    }
}
