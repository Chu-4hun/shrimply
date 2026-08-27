use std::cell::RefCell;
use std::rc::Rc;

use shrimply_evaluation::{
    TransformEvaluation, TransformExpressionCache, VisualEvaluation, resolve_paint_fill_options,
    resolve_paint_stroke_options, resolve_paint_texture_options,
};
use shrimply_project::project::{
    CanvasSize, DrawingFillMode, PaintTextureOptions, ResolvedPaintStrokeOptions, TransitionSide,
    VideoItem, VideoItemContent, VisualTransitionKind,
};
use skia_safe::Canvas;
use uuid::Uuid;

use crate::gpu::CudaVideoCompositor;
use crate::gpu::generated_gpu::GeneratedVisual;
use crate::layer::{VectorVisual, Visual, VisualData};
use crate::visual_source::{VisualElement, VisualRender, VisualRenderRequest, VisualSourceCache};

const DRAWING_STROKE_PHASE: f32 = 0.8;

pub(crate) struct PaintElement {
    item_id: Uuid,
    expressions: TransformExpressionCache,
    cache: Rc<RefCell<shrimply_paint_skia::PaintCache>>,
}

impl PaintElement {
    pub(crate) fn new(item: &VideoItem) -> Self {
        Self {
            item_id: item.id,
            expressions: TransformExpressionCache::default(),
            cache: Rc::new(RefCell::new(shrimply_paint_skia::PaintCache::default())),
        }
    }
}

impl VisualElement for PaintElement {
    fn matches(&self, item: &VideoItem, _canvas_size: CanvasSize) -> bool {
        self.item_id == item.id && matches!(&item.content, VideoItemContent::Paint(_))
    }

    fn draw(
        &mut self,
        request: VisualRenderRequest<'_>,
        _compositor: &mut CudaVideoCompositor,
        _track_id: Uuid,
        _cache: &mut VisualSourceCache,
    ) -> Result<VisualRender, String> {
        let Some(local_time) =
            shrimply_project::project::generated_item_time(request.item, request.position)
        else {
            return Ok(VisualRender::Empty);
        };
        let VideoItemContent::Paint(paint) = &request.item.content else {
            return Err("paint renderer received a non-paint visual".to_string());
        };
        let evaluation = VisualEvaluation::for_item_at_local_time_with_audio(
            request.project,
            request.item,
            request.position,
            local_time,
            request.audio_analysis,
        );
        let stroke_transform = shrimply_evaluation::resolve_transform(
            &paint.stroke_transform,
            &evaluation,
            &mut self.expressions,
        );
        let palette: Vec<_> = paint
            .palette
            .iter()
            .map(|entry| {
                let color = shrimply_evaluation::resolve_color(
                    &entry.color,
                    &evaluation,
                    &mut self.expressions,
                );
                let texture =
                    resolve_texture(entry.texture.as_ref(), &evaluation, &mut self.expressions);
                shrimply_paint_skia::ResolvedPaintPaletteEntry { color, texture }
            })
            .collect();
        let stroke_options =
            resolve_paint_stroke_options(&paint.stroke, &evaluation, &mut self.expressions);
        let fill_options =
            resolve_paint_fill_options(&paint.fill, &evaluation, &mut self.expressions);
        let drawing = shrimply_evaluation::resolve_paint_drawing(
            &paint.drawing,
            &evaluation,
            &mut self.expressions,
            paint.palette.len(),
        );
        let path_offsets: Vec<_> = request
            .item
            .modifiers
            .iter()
            .filter(|modifier| modifier.enabled)
            .filter_map(|modifier| {
                let shrimply_video_modifiers::ModifierEffect::Vector(effect) = &modifier.effect
                else {
                    return None;
                };
                let shrimply_video_modifiers::VectorModifierEffect::PathOffset(effect) = &**effect
                else {
                    return None;
                };
                Some(shrimply_evaluation::resolve_path_offset_modifier(
                    effect,
                    &evaluation,
                    &mut self.expressions,
                ))
            })
            .collect();
        let canvas_size = request.project.canvas_size;
        let canvas = glam::Vec2::new(
            canvas_size.width.max(1) as f32,
            canvas_size.height.max(1) as f32,
        );
        let (prepared, texture_fingerprints) = {
            let mut cache = self.cache.borrow_mut();
            let texture_fingerprints = palette
                .iter()
                .map(|entry| preflight_texture(&mut cache, entry.texture.as_ref()))
                .collect::<Result<Vec<_>, _>>()?;
            let prepared = shrimply_paint_skia::prepare_frame(
                &mut cache,
                (&drawing, paint.revision),
                &stroke_options,
                fill_options,
                &path_offsets,
                stroke_transform,
                canvas,
            );
            (prepared, texture_fingerprints)
        };
        let transition = request.generated_transition.map(|value| {
            (
                value.kind,
                value.side == shrimply_project::project::TransitionSide::Intro,
                value.progress.to_bits(),
                value.interpolation,
                value.ordering,
                value.drawing_stroke_overlap.to_bits(),
                value.drawing_stroke_length_weight.to_bits(),
                value.drawing_fill_mode,
                value.effect_amount.to_bits(),
                value.effect_detail.to_bits(),
                value.effect_angle_degrees.to_bits(),
                value.effect_fade,
                value.effect_seed,
            )
        });
        let cache_key = serde_json::to_vec(&(
            request.item.id,
            paint.revision,
            prepared.geometry.key.centerlines.content_hash,
            transform_bits(stroke_transform),
            stroke_options_bits(stroke_options),
            fill_options.closure_tolerance.to_bits(),
            path_offset_bits(&path_offsets),
            palette.iter().map(|entry| entry.color).collect::<Vec<_>>(),
            canvas_size,
            request.render_canvas,
            palette
                .iter()
                .zip(texture_fingerprints)
                .map(|(entry, fingerprint)| {
                    resolved_texture_key(entry.texture.as_ref(), fingerprint)
                })
                .collect::<Vec<_>>(),
            transition,
        ))
        .map_err(|error| format!("serialize paint raster cache key: {error}"))?;

        let reveal = request
            .generated_transition
            .filter(|transition| transition.kind == VisualTransitionKind::Drawing)
            .map(|transition| drawing_reveal(&prepared, transition));
        Ok(VisualRender::Ready(Visual::Vector(VectorVisual::prepared(
            Box::new(DeferredPaintFrame {
                cache_key,
                canvas_size,
                surface_size: request.render_canvas,
                cache: Rc::clone(&self.cache),
                prepared,
                evaluation,
                palette,
                reveal,
                draw_error: RefCell::new(None),
            }),
            request.state,
        ))))
    }
}

struct DeferredPaintFrame {
    cache_key: Vec<u8>,
    canvas_size: CanvasSize,
    surface_size: CanvasSize,
    cache: Rc<RefCell<shrimply_paint_skia::PaintCache>>,
    prepared: shrimply_paint_skia::PreparedPaintFrame,
    evaluation: VisualEvaluation,
    palette: Vec<shrimply_paint_skia::ResolvedPaintPaletteEntry>,
    reveal: Option<PaintReveal>,
    draw_error: RefCell<Option<String>>,
}

struct PaintReveal {
    stroke_progress: Vec<f32>,
    fill_opacity: Vec<f32>,
}

impl GeneratedVisual for DeferredPaintFrame {
    fn draw(
        &self,
        canvas: &Canvas,
        _evaluation: &TransformEvaluation,
        _expressions: &mut TransformExpressionCache,
        path_effect: Option<&skia_safe::PathEffect>,
    ) {
        let result = shrimply_paint_skia::draw(
            &mut self.cache.borrow_mut(),
            canvas,
            &self.prepared,
            shrimply_paint_skia::ResolvedPaintAppearance {
                palette: &self.palette,
                reveal: self
                    .reveal
                    .as_ref()
                    .map(|reveal| shrimply_paint_skia::PaintReveal {
                        stroke_progress: &reveal.stroke_progress,
                        fill_opacity: &reveal.fill_opacity,
                    }),
            },
            path_effect,
        );
        if let Err(error) = result {
            *self.draw_error.borrow_mut() = Some(error.to_string());
        }
    }
}

impl DeferredPaintFrame {
    fn prepared_morph_scene(&self) -> Option<crate::vector_morph::MorphScene> {
        let mut cache = self.cache.borrow_mut();
        let fill_paths =
            shrimply_paint_skia::prepare_fill_paths(&mut cache, &self.prepared.geometry);
        let stroke_paths =
            shrimply_paint_skia::prepare_stroke_paths(&mut cache, &self.prepared.outlines);
        let mut objects = Vec::with_capacity(fill_paths.len() + stroke_paths.len());
        for (fill, path) in self.prepared.geometry.fills.iter().zip(fill_paths.iter()) {
            let paint =
                shrimply_paint_skia::morph_paint(&mut cache, self.palette.get(fill.color_index)?)
                    .ok()?;
            objects.push(crate::vector_morph::MorphObject {
                path: crate::vector_morph::skia_path_to_morph(path),
                appearance: vec![crate::vector_morph::MorphPaintLayer {
                    paint,
                    offset: glam::Vec2::ZERO,
                }],
            });
        }
        for (outline, path) in self
            .prepared
            .outlines
            .outlines
            .iter()
            .zip(stroke_paths.iter())
        {
            let paint = shrimply_paint_skia::morph_paint(
                &mut cache,
                self.palette.get(outline.color_index)?,
            )
            .ok()?;
            objects.push(crate::vector_morph::MorphObject {
                path: crate::vector_morph::skia_path_to_morph(path),
                appearance: vec![crate::vector_morph::MorphPaintLayer {
                    paint,
                    offset: glam::Vec2::ZERO,
                }],
            });
        }
        Some(crate::vector_morph::MorphScene {
            objects,
            evaluation: self.evaluation.clone(),
            canvas_size: self.canvas_size,
        })
    }
}

fn drawing_reveal(
    frame: &shrimply_paint_skia::PreparedPaintFrame,
    transition: crate::visual_source::GeneratedTransition,
) -> PaintReveal {
    let reveal_progress = match transition.side {
        TransitionSide::Intro => transition.progress,
        TransitionSide::Outro => 1.0 - transition.progress,
    }
    .clamp(0.0, 1.0);
    let has_strokes = !frame.geometry.centerlines.is_empty();
    let has_fills = !frame.geometry.fills.is_empty();
    let fades = transition.drawing_fill_mode != DrawingFillMode::Direct;
    let stroke_phase_end = if has_strokes && has_fills && fades {
        DRAWING_STROKE_PHASE
    } else {
        1.0
    };
    let stroke_phase = (reveal_progress / stroke_phase_end).clamp(0.0, 1.0);
    let lengths: Vec<_> = frame
        .geometry
        .centerlines
        .iter()
        .map(|centerline| {
            centerline
                .stroke_points
                .last()
                .map_or(0.0, |point| point.running_length)
                .max(centerline.width.abs())
        })
        .collect();
    let stroke_progress = crate::math::drawing_stroke_progresses(
        stroke_phase,
        &lengths,
        transition.drawing_stroke_length_weight,
        transition.drawing_stroke_overlap,
    )
    .into_iter()
    .map(|progress| transition.interpolation.value(f64::from(progress)) as f32)
    .collect();

    let fill_opacity = match transition.drawing_fill_mode {
        DrawingFillMode::Direct => {
            let threshold = if has_strokes { 1.0 } else { 0.0 };
            let opacity = if reveal_progress >= threshold {
                1.0
            } else {
                0.0
            };
            vec![opacity; frame.geometry.fills.len()]
        }
        DrawingFillMode::FadeTogether | DrawingFillMode::FadeSequentially => {
            let fill_start = if has_strokes && has_fills {
                DRAWING_STROKE_PHASE
            } else {
                0.0
            };
            let fill_progress =
                ((reveal_progress - fill_start) / (1.0 - fill_start)).clamp(0.0, 1.0);
            (0..frame.geometry.fills.len())
                .map(|index| {
                    let progress = match transition.drawing_fill_mode {
                        DrawingFillMode::FadeTogether => fill_progress,
                        DrawingFillMode::FadeSequentially => {
                            crate::math::lagged_transition_progress(
                                fill_progress,
                                index,
                                frame.geometry.fills.len(),
                                1.0,
                            )
                            .clamp(0.0, 1.0)
                        }
                        DrawingFillMode::Direct => unreachable!(),
                    };
                    transition.interpolation.value(f64::from(progress)) as f32
                })
                .collect()
        }
    };
    PaintReveal {
        stroke_progress,
        fill_opacity,
    }
}

impl VisualData for DeferredPaintFrame {
    fn cache_key(&self) -> &[u8] {
        &self.cache_key
    }

    fn morph_scene(&self) -> Option<crate::vector_morph::MorphScene> {
        self.prepared_morph_scene()
    }

    fn rasterize(
        &self,
        compositor: &mut CudaVideoCompositor,
        drawing_strategy: shrimply_project::project::SkiaDrawingStrategy,
        operations: &[crate::layer::VectorOperation],
    ) -> Result<Rc<crate::gpu::VisualFrame>, String> {
        *self.draw_error.borrow_mut() = None;
        let rendered = compositor.render_generated_visual(
            self.surface_size,
            self.canvas_size,
            self,
            &self.evaluation,
            operations,
            drawing_strategy,
        );
        if let Some(error) = self.draw_error.borrow_mut().take() {
            return Err(error);
        }
        rendered.map(Rc::new)
    }
}

fn resolve_texture(
    texture: Option<&PaintTextureOptions>,
    evaluation: &VisualEvaluation,
    expressions: &mut TransformExpressionCache,
) -> Option<shrimply_paint_skia::ResolvedPaintTexture> {
    texture.map(|texture| shrimply_paint_skia::ResolvedPaintTexture {
        image_path: texture.image_path.clone(),
        options: resolve_paint_texture_options(texture, evaluation, expressions),
    })
}

type TextureKey = String;
type StrokeEndKey = (bool, shrimply_project::project::PaintTaper, u32);
type StrokeOptionsKey = (u32, u32, u32, u32, u32, u32, StrokeEndKey, StrokeEndKey);

fn resolved_texture_key(
    texture: Option<&shrimply_paint_skia::ResolvedPaintTexture>,
    fingerprint: Option<TextureKey>,
) -> Option<(TextureKey, u32, u32)> {
    texture.zip(fingerprint).map(|(texture, fingerprint)| {
        (
            fingerprint,
            texture.options.repeat_scale.to_bits(),
            texture.options.rotation_degrees.to_bits(),
        )
    })
}

fn stroke_options_bits(options: ResolvedPaintStrokeOptions) -> StrokeOptionsKey {
    (
        options.width.to_bits(),
        options.thinning.to_bits(),
        options.smoothing.to_bits(),
        options.streamline.to_bits(),
        options.simplification_tolerance.to_bits(),
        options.maximum_subdivision_spacing.to_bits(),
        (
            options.start.cap,
            options.start.taper,
            options.start.taper_distance.to_bits(),
        ),
        (
            options.end.cap,
            options.end.taper,
            options.end.taper_distance.to_bits(),
        ),
    )
}

fn path_offset_bits(offsets: &[shrimply_paint_skia::ResolvedPathOffset]) -> Vec<[u32; 4]> {
    offsets
        .iter()
        .map(|offset| {
            [
                offset.amplitude.to_bits(),
                offset.spacing.to_bits(),
                offset.seed.to_bits(),
                offset.evolution.to_bits(),
            ]
        })
        .collect()
}

fn preflight_texture(
    cache: &mut shrimply_paint_skia::PaintCache,
    texture: Option<&shrimply_paint_skia::ResolvedPaintTexture>,
) -> Result<Option<TextureKey>, String> {
    texture
        .map(|texture| {
            shrimply_paint_skia::prepare_texture(cache, texture)
                .map(|prepared| fingerprint_key(&prepared.fingerprint))
                .map_err(|error| error.to_string())
        })
        .transpose()
}

fn fingerprint_key(fingerprint: &shrimply_paint_skia::TextureFingerprint) -> TextureKey {
    fingerprint.cache_key()
}

fn transform_bits(
    transform: shrimply_project::project::ResolvedTransform,
) -> ([u32; 2], [u32; 2], [u32; 2], [u32; 2], u32) {
    (
        transform.position.to_array().map(f32::to_bits),
        transform.anchor.to_array().map(f32::to_bits),
        transform.scale.to_array().map(f32::to_bits),
        transform.shear.to_array().map(f32::to_bits),
        transform.rotation_degrees.to_bits(),
    )
}
