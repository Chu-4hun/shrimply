use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use shrimply_asset::{Asset, AssetSnapshot};
use shrimply_evaluation::{TransformExpressionCache, VisualEvaluation, resolve_bool};
use shrimply_gpu_memory::{ResourceKey, global as gpu_memory};
use shrimply_layered_image::LayeredImage;
use shrimply_project::project::{CanvasSize, LayeredImageItem, VideoItem, VideoItemContent};
use uuid::Uuid;

use crate::gpu::layered_image::LayeredImageGpuLayer;
use crate::gpu::{CudaVideoCompositor, VisualFrame};
use crate::layer::{GpuFrame, RasterVisual, Visual};
use crate::visual_source::{
    VisualElement, VisualPrepareRequest, VisualRender, VisualRenderRequest, VisualSourceCache,
};

const RGBA_BYTES_PER_PIXEL: usize = 4;
struct LoadedLayeredImage {
    snapshot: AssetSnapshot,
    document: Arc<LayeredImage>,
    source_bytes: u64,
}

pub struct LayeredImageRenderSession {
    file: Asset,
    expressions: TransformExpressionCache,
}

pub(crate) struct LayeredImageAsset {
    file: Asset,
    snapshot: Option<AssetSnapshot>,
    document_key: Option<ResourceKey>,
    pending: Option<Receiver<Result<LoadedLayeredImage, String>>>,
    last_reload_error: Option<String>,
}

impl LayeredImageRenderSession {
    pub fn new(item: &VideoItem) -> Result<Self, String> {
        if !matches!(item.content, VideoItemContent::LayeredImage(_)) {
            return Err("layered image renderer received another item type".to_string());
        }
        Ok(Self {
            file: item.file.clone(),
            expressions: TransformExpressionCache::default(),
        })
    }
}

impl LayeredImageAsset {
    pub(crate) fn new(file: Asset) -> Self {
        Self {
            file,
            snapshot: None,
            document_key: None,
            pending: None,
            last_reload_error: None,
        }
    }

    pub(crate) fn preload(&mut self) -> Result<(), String> {
        if self.pending.is_some()
            || self
                .document_key
                .as_ref()
                .is_some_and(|key| gpu_memory().contains_resource(key))
        {
            return Ok(());
        }
        if self.document_key.is_none()
            && let Some(error) = &self.last_reload_error
        {
            return Err(error.clone());
        }
        let file = self.file.clone();
        let (reply, response) = mpsc::sync_channel(1);
        rayon::spawn(move || {
            let _measurement = shrimply_benchmarking::measure("Layered image / Preload");
            let _ = reply.send(load_layered_image(&file));
        });
        self.pending = Some(response);
        shrimply_benchmarking::increment("Layered image / Preloads submitted");
        Ok(())
    }

    fn reload(&mut self, exact: bool, block: bool) -> Result<(bool, bool), String> {
        if let Some(error) = &self.last_reload_error
            && exact
        {
            return Err(error.clone());
        }
        if let Some(pending) = self.pending.take() {
            let result = if block {
                pending
                    .recv()
                    .map_err(|_| "layered image preload stopped before returning a document")?
            } else {
                match pending.try_recv() {
                    Ok(result) => result,
                    Err(TryRecvError::Empty) => {
                        self.pending = Some(pending);
                        return Ok((self.document_key.is_some(), true));
                    }
                    Err(TryRecvError::Disconnected) => {
                        return Err(
                            "layered image preload stopped before returning a document".to_string()
                        );
                    }
                }
            };
            match result {
                Ok(LoadedLayeredImage {
                    snapshot,
                    document,
                    source_bytes,
                }) => {
                    let key = layered_document_key(&snapshot);
                    gpu_memory().insert_resource(key.clone(), source_bytes, document)?;
                    self.document_key = Some(key);
                    self.snapshot = Some(snapshot);
                    self.last_reload_error = None;
                    shrimply_benchmarking::increment("Layered image / Preloads consumed");
                    return Ok((true, false));
                }
                Err(error) => {
                    self.last_reload_error = Some(error.clone());
                    return Err(error);
                }
            }
        }

        if self
            .document_key
            .as_ref()
            .is_none_or(|key| !gpu_memory().contains_resource(key))
        {
            self.document_key = None;
            self.preload()?;
            return self.reload(exact, block);
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(AssetSnapshot::is_current)
        {
            return Ok((true, false));
        }
        self.last_reload_error = None;
        self.preload()?;
        if block {
            self.reload(exact, true)
        } else {
            Ok((true, true))
        }
    }

    fn composite(
        &mut self,
        request: &VisualRenderRequest<'_>,
        compositor: &mut CudaVideoCompositor,
        expressions: &mut TransformExpressionCache,
    ) -> Result<(Option<Rc<VisualFrame>>, bool), String> {
        let (loaded, refreshing) = {
            let _measurement = shrimply_benchmarking::measure("Layered image / Reload check");
            self.reload(
                request.accuracy.content_accurate(),
                request.decode_control.is_none(),
            )?
        };
        if !loaded {
            return Ok((None, refreshing));
        }
        let key = self
            .document_key
            .clone()
            .expect("layered image document source was loaded");
        let Some(document) = gpu_memory().get_resource::<Arc<LayeredImage>>(&key)? else {
            self.document_key = None;
            self.preload()?;
            return Ok((None, true));
        };
        let document: &LayeredImage = document.as_ref();
        let VideoItemContent::LayeredImage(item) = &request.item.content else {
            unreachable!();
        };
        let evaluation = VisualEvaluation::for_item_with_audio(
            request.project,
            request.item,
            request.position,
            request.audio_analysis,
        );
        let visible = {
            let _measurement = shrimply_benchmarking::measure("Layered image / Resolve visibility");
            document
                .layers
                .iter()
                .map(|layer| {
                    resolved_visibility(&layer.path, layer.visible, item, &evaluation, expressions)
                        && parents_visible(document, layer.parent, item, &evaluation, expressions)
                })
                .collect::<Vec<_>>()
        };
        let source_layers = document
            .layers
            .iter()
            .map(|source| {
                compositor
                    .upload_rgba_layer(document.width.max(1), document.height.max(1), &source.rgba)
                    .map(Rc::new)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut gpu_layers = Vec::new();
        let mut clipping_base = None::<(Option<u32>, usize, f32)>;
        for (layer_index, layer) in document.layers.iter().enumerate().rev() {
            if !visible[layer_index] {
                if !layer.clipped {
                    clipping_base = None;
                }
                continue;
            }
            let opacity = f32::from(layer.opacity) / 255.0 * parent_opacity(document, layer.parent);
            if !layer.clipped {
                clipping_base = Some((layer.parent, layer_index, opacity));
            }
            let clipping_base = layer
                .clipped
                .then_some(clipping_base)
                .flatten()
                .filter(|(parent, _, _)| *parent == layer.parent)
                .map(|(_, index, opacity)| (&*source_layers[index], opacity));
            if layer.clipped && clipping_base.is_none() {
                continue;
            }
            gpu_layers.push(LayeredImageGpuLayer {
                source: &source_layers[layer_index],
                clipping_base,
                mode: layer.blend_mode,
                opacity,
                noise_seed: (layer_index as u32).wrapping_mul(0x85eb_ca6b),
            });
        }
        let layer = {
            let _measurement =
                shrimply_benchmarking::measure("Layered image / Composite submission");
            Rc::new(compositor.composite_layered_image_layers(
                document.width.max(1),
                document.height.max(1),
                &gpu_layers,
            )?)
        };
        Ok((Some(layer), refreshing))
    }
}

fn load_layered_image(file: &Asset) -> Result<LoadedLayeredImage, String> {
    let snapshot = file.snapshot()?;
    let document = shrimply_layered_image::load(file)?;
    snapshot.verify_current()?;
    let width = document.width.max(1);
    let height = document.height.max(1);
    let expected_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(RGBA_BYTES_PER_PIXEL))
        .ok_or_else(|| "layered image byte size overflow".to_string())?;
    let mut source_bytes = 0_u64;
    for layer in &document.layers {
        if layer.rgba.len() != expected_bytes {
            return Err(format!(
                "layer {} has {} RGBA bytes, expected {expected_bytes}",
                layer.name,
                layer.rgba.len(),
            ));
        }
        source_bytes = source_bytes
            .checked_add(
                u64::try_from(layer.rgba.len())
                    .map_err(|_| "layered image source size exceeds u64".to_string())?,
            )
            .ok_or_else(|| "layered image source byte size overflow".to_string())?;
    }
    Ok(LoadedLayeredImage {
        snapshot,
        document,
        source_bytes,
    })
}

fn layered_document_key(snapshot: &AssetSnapshot) -> ResourceKey {
    let mut discriminator = Vec::new();
    discriminator.extend_from_slice(b"layered-document");
    discriminator.extend_from_slice(snapshot.cache_key().as_bytes());
    ResourceKey::new(snapshot.path().to_path_buf(), discriminator)
}

impl VisualElement for LayeredImageRenderSession {
    fn matches(&self, item: &VideoItem, _canvas_size: CanvasSize) -> bool {
        matches!(item.content, VideoItemContent::LayeredImage(_)) && self.file == item.file
    }

    fn prepare(
        &mut self,
        _request: VisualPrepareRequest<'_>,
        _track_id: Uuid,
        cache: &mut VisualSourceCache,
    ) -> Result<(), String> {
        cache.layered_image(&self.file).preload()
    }

    fn draw(
        &mut self,
        request: VisualRenderRequest<'_>,
        compositor: &mut CudaVideoCompositor,
        _track_id: Uuid,
        cache: &mut VisualSourceCache,
    ) -> Result<VisualRender, String> {
        let (layer, refreshing) = cache.layered_image(&self.file).composite(
            &request,
            compositor,
            &mut self.expressions,
        )?;
        let Some(layer) = layer else {
            return Ok(VisualRender::Loading(CanvasSize {
                width: request.item.source_width.max(1),
                height: request.item.source_height.max(1),
            }));
        };
        let visual = Visual::Raster(RasterVisual::materialized(
            GpuFrame::Rgba(layer),
            request.state,
        ));
        Ok(if refreshing {
            VisualRender::LoadingPlaceholder(visual)
        } else {
            VisualRender::Ready(visual)
        })
    }
}

fn resolved_visibility(
    path: &str,
    source_visible: bool,
    item: &LayeredImageItem,
    evaluation: &VisualEvaluation,
    expressions: &mut TransformExpressionCache,
) -> bool {
    item.layers
        .iter()
        .find(|entry| entry.path == path)
        .and_then(|entry| entry.visibility.as_ref())
        .map_or(source_visible, |value| {
            resolve_bool(value, evaluation, expressions)
        })
}

fn parents_visible(
    document: &LayeredImage,
    mut parent: Option<u32>,
    item: &LayeredImageItem,
    evaluation: &VisualEvaluation,
    expressions: &mut TransformExpressionCache,
) -> bool {
    while let Some(id) = parent {
        let Some(group) = document.groups.iter().find(|group| group.id == id) else {
            break;
        };
        if !resolved_visibility(&group.path, group.visible, item, evaluation, expressions) {
            return false;
        }
        parent = group.parent;
    }
    true
}

fn parent_opacity(document: &LayeredImage, mut parent: Option<u32>) -> f32 {
    let mut opacity = 1.0;
    while let Some(id) = parent {
        let Some(group) = document.groups.iter().find(|group| group.id == id) else {
            break;
        };
        opacity *= f32::from(group.opacity) / 255.0;
        parent = group.parent;
    }
    opacity
}
