use std::rc::Rc;

use shrimply_asset::{Asset, AssetSnapshot};
use shrimply_gpu_memory::{ResourceKey, global as gpu_memory};
use shrimply_project::project::{CanvasSize, VideoItem, VideoItemContent};
use uuid::Uuid;

use crate::gpu::{CudaVideoCompositor, VisualFrame};
use crate::layer::{GpuFrame, RasterVisual, Visual};
use crate::visual_source::{
    VisualElement, VisualPrepareRequest, VisualRender, VisualRenderRequest, VisualSourceCache,
};

pub struct PdfRenderSession {
    file: Asset,
    snapshot: AssetSnapshot,
    page: u32,
    width: u32,
    height: u32,
}

impl PdfRenderSession {
    pub fn new(item: &VideoItem) -> Result<Self, String> {
        let VideoItemContent::Pdf(pdf) = &item.content else {
            return Err("PDF renderer received a non-PDF item".to_string());
        };
        Ok(Self {
            file: item.file.clone(),
            snapshot: item.file.snapshot()?,
            page: pdf.page,
            width: item.source_width.max(1),
            height: item.source_height.max(1),
        })
    }

    fn image_key(&self) -> ResourceKey {
        let mut discriminator = Vec::new();
        discriminator.extend_from_slice(b"pdf-rgba\0");
        discriminator.extend_from_slice(self.snapshot.cache_key().as_bytes());
        discriminator.extend_from_slice(&self.page.to_le_bytes());
        discriminator.extend_from_slice(&self.width.to_le_bytes());
        discriminator.extend_from_slice(&self.height.to_le_bytes());
        ResourceKey::new(self.snapshot.path().to_path_buf(), discriminator)
    }

    fn request_page(&self) {
        let key = self.image_key();
        if !gpu_memory().begin_resource_load(key.clone()) {
            return;
        }
        let file = self.file.clone();
        let snapshot = self.snapshot.clone();
        let page = self.page;
        rayon::spawn(move || {
            let result = snapshot.read().and_then(|bytes| {
                let rendered = shrimply_pdf::render_page(bytes, page)?;
                snapshot.ensure_current()?;
                VisualFrame::from_rgba_bytes(
                    rendered.size.width,
                    rendered.size.height,
                    rendered.rgba,
                )
            });
            let bytes = result.as_ref().map_or(0, VisualFrame::bytes);
            if let Err(error) = gpu_memory().finish_resource_load(key, bytes, result) {
                tracing::error!(file = %file.display(), %error, "could not finish loading PDF page");
            }
        });
        shrimply_benchmarking::increment("PDF raster cache / Miss");
    }
}

impl VisualElement for PdfRenderSession {
    fn matches(&self, item: &VideoItem, _canvas_size: CanvasSize) -> bool {
        let VideoItemContent::Pdf(pdf) = &item.content else {
            return false;
        };
        self.file == item.file
            && self.snapshot.is_current()
            && self.page == pdf.page
            && self.width == item.source_width.max(1)
            && self.height == item.source_height.max(1)
    }

    fn prepare(
        &mut self,
        _request: VisualPrepareRequest<'_>,
        _track_id: Uuid,
        _cache: &mut VisualSourceCache,
    ) -> Result<(), String> {
        self.request_page();
        Ok(())
    }

    fn draw(
        &mut self,
        request: VisualRenderRequest<'_>,
        compositor: &mut CudaVideoCompositor,
        _track_id: Uuid,
        _cache: &mut VisualSourceCache,
    ) -> Result<VisualRender, String> {
        if let Some(frame) = gpu_memory().get_resource::<VisualFrame>(&self.image_key())? {
            shrimply_benchmarking::increment("PDF raster cache / Hit");
            let frame = Rc::new(compositor.upload_frame(&frame)?);
            return Ok(VisualRender::Ready(Visual::Raster(
                RasterVisual::materialized(GpuFrame::Rgba(frame), request.state),
            )));
        }
        self.request_page();
        Ok(VisualRender::Loading(CanvasSize {
            width: self.width,
            height: self.height,
        }))
    }
}
