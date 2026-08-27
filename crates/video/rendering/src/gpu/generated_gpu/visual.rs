use std::sync::Arc;

use cuda_core::CudaContext;
use shrimply_evaluation::{TransformEvaluation, TransformExpressionCache};
use shrimply_visual_frame::{VisualFormat, VisualFrame, VisualPlane};
use skia_safe::Canvas;

use super::super::CompositedVideoFrame;

pub(crate) fn visual_frame_from_canvas(
    context: Arc<CudaContext>,
    frame: CompositedVideoFrame,
) -> Result<VisualFrame, String> {
    let buffer = frame
        .buffer
        .cast_chunks::<u8>()
        .map_err(|_| "generated CUDA output buffer cannot be viewed as bytes".to_string())?;
    let plane = VisualPlane {
        device_ptr: buffer.cu_deviceptr(),
        pitch_bytes: frame.width as usize * 4,
        width_bytes: frame.width as usize * 4,
        height: frame.height as usize,
    };
    unsafe {
        VisualFrame::from_owned_gpu_buffers(
            context,
            VisualFormat::Rgba8,
            frame.width,
            frame.height,
            &[plane],
            vec![buffer],
        )
    }
}

pub(crate) trait GeneratedVisual {
    fn draw(
        &self,
        canvas: &Canvas,
        evaluation: &TransformEvaluation,
        expressions: &mut TransformExpressionCache,
        path_effect: Option<&skia_safe::PathEffect>,
    );
}
