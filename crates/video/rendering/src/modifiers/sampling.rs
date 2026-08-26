use super::RasterModifierRuntime;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve;
use shrimply_project::project::VideoSampleMethod;
use shrimply_video_modifiers::sampling::SamplingModifier;

impl RasterModifierRuntime for SamplingModifier {
    fn apply_raster(
        &self,
        mut input: crate::layer::RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<crate::layer::RasterVisual, String> {
        let method = resolve(&self.method, context.evaluation, context.expressions);
        let method = if context.accuracy.content_accurate() {
            method
        } else {
            if matches!(method, VideoSampleMethod::Nearest) {
                VideoSampleMethod::Nearest
            } else {
                VideoSampleMethod::Bilinear
            }
        };
        input.push_spatial(move |state| state.sampling = method);
        Ok(input)
    }
}
