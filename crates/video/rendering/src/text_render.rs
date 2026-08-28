use std::rc::Rc;

use skia_safe::{
    BlendMode, BlurStyle, Canvas, Color as SkiaColor, MaskFilter, Paint, PaintStyle, PathBuilder,
    Point, RRect, Rect, canvas::SaveLayerRec,
};

use crate::gpu::CudaVideoCompositor;
use crate::gpu::generated_gpu::GeneratedVisual;
use crate::layer::{TextMaskOperation, VectorOperation, VectorVisual, Visual, VisualData};
use crate::visual_source::VisualSourceCache;
use crate::visual_source::{GeneratedTransition, VisualElement, VisualRender, VisualRenderRequest};
use shrimply_core::timeline_value::*;
use shrimply_evaluation::{TransformEvaluation, TransformExpressionCache};
use shrimply_project::project::{CanvasSize, Color, TextItem, VideoItem};
use shrimply_video_modifiers::text_mask::{SNAP_THRESHOLD, TextMaskDirection, TextMaskPartialMode};
use uuid::Uuid;

pub struct TextElement {
    canvas_size: CanvasSize,
    expressions: TransformExpressionCache,
}

impl TextElement {
    pub fn new(canvas_size: CanvasSize) -> Self {
        Self {
            canvas_size,
            expressions: Default::default(),
        }
    }
}

impl VisualElement for TextElement {
    fn matches(&self, item: &VideoItem, canvas_size: CanvasSize) -> bool {
        self.canvas_size == canvas_size
            && matches!(
                &item.content,
                shrimply_project::project::VideoItemContent::Text(_)
            )
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
        let shrimply_project::project::VideoItemContent::Text(text) = &request.item.content else {
            return Err("text renderer received a non-text visual".to_string());
        };
        let evaluation = shrimply_evaluation::VisualEvaluation::for_item_at_local_time_with_audio(
            request.project,
            request.item,
            request.position,
            local_time,
            request.audio_analysis,
        );
        let rotation_degrees = shrimply_evaluation::resolve_scalar(
            &request.item.transform.rotation_degrees,
            &evaluation,
            &mut self.expressions,
        );
        let font_size = shrimply_evaluation::resolve_scalar(
            &text.font_size,
            &evaluation,
            &mut self.expressions,
        )
        .max(1.0);
        let font_weight = shrimply_evaluation::resolve_scalar(
            &text.font_weight,
            &evaluation,
            &mut self.expressions,
        );
        let tracking =
            shrimply_evaluation::resolve_scalar(&text.tracking, &evaluation, &mut self.expressions);
        let line_height = shrimply_evaluation::resolve_scalar(
            &text.line_height,
            &evaluation,
            &mut self.expressions,
        )
        .max(f32::EPSILON);
        let content =
            shrimply_evaluation::resolve_text(&text.text, &evaluation, &mut self.expressions);
        let layout = crate::text_layout::layout(
            text,
            &content,
            font_size,
            font_weight,
            tracking,
            line_height,
            evaluation.local_time(),
        );
        let layout_anchor = crate::text_layout::anchor(
            layout.size,
            text.h_align.value_at(evaluation.local_time()),
            text.v_align.value_at(evaluation.local_time()),
        );
        let mut decoration =
            decoration_outset(text, &evaluation, &mut self.expressions, rotation_degrees);
        if request.generated_transition.is_some_and(|transition| {
            matches!(
                transition.kind,
                shrimply_project::project::VisualTransitionKind::Morph
                    | shrimply_project::project::VisualTransitionKind::Write
                    | shrimply_project::project::VisualTransitionKind::Create
                    | shrimply_project::project::VisualTransitionKind::FacetAssembly
                    | shrimply_project::project::VisualTransitionKind::Coalesce
                    | shrimply_project::project::VisualTransitionKind::ContourCurrent
                    | shrimply_project::project::VisualTransitionKind::SoftRefraction
                    | shrimply_project::project::VisualTransitionKind::MorphologicalResolve
                    | shrimply_project::project::VisualTransitionKind::LivingFill
                    | shrimply_project::project::VisualTransitionKind::Diffusion
                    | shrimply_project::project::VisualTransitionKind::ReverseDiffusion
            )
        }) {
            let canvas_extent = request
                .project
                .canvas_size
                .width
                .max(request.project.canvas_size.height)
                .max(1) as f32;
            let trace_outset = match request.generated_transition.map(|value| value.kind) {
                Some(shrimply_project::project::VisualTransitionKind::FacetAssembly) => {
                    canvas_extent * 0.3
                }
                Some(
                    shrimply_project::project::VisualTransitionKind::Coalesce
                    | shrimply_project::project::VisualTransitionKind::SoftRefraction
                    | shrimply_project::project::VisualTransitionKind::MorphologicalResolve
                    | shrimply_project::project::VisualTransitionKind::Diffusion
                    | shrimply_project::project::VisualTransitionKind::ReverseDiffusion,
                ) => canvas_extent * 0.05,
                _ => request.project.canvas_size.height.max(1) as f32 / 1080.0,
            };
            for edge in &mut decoration {
                *edge = edge.max(trace_outset);
            }
        }
        let decoration_offset = glam::Vec2::new(decoration[3], decoration[0]);
        let layout_offset = shrimply_math_geometry::ComposedTransform2D {
            matrix: glam::Mat3::from_translation(-(layout_anchor + decoration_offset)),
        };
        let mut state = request.state;
        state.transform = state.transform.compose(layout_offset);
        Ok(VisualRender::Ready(Visual::Vector(VectorVisual::prepared(
            Box::new(DeferredTextFrame {
                canvas_size: request.project.canvas_size,
                surface_size: self.canvas_size,
                content_offset: decoration_offset,
                text: (**text).clone(),
                content,
                evaluation,
                rotation_degrees,
                transition: request.generated_transition,
            }),
            state,
        ))))
    }
}

pub fn decoration_outset(
    text: &TextItem,
    evaluation: &TransformEvaluation,
    expressions: &mut TransformExpressionCache,
    rotation_degrees: f32,
) -> [f32; 4] {
    let outline_width = number_value(&text.outline_width, evaluation, expressions).max(0.0);
    let shadow_distance = number_value(&text.shadow_distance, evaluation, expressions).max(0.0);
    let shadow_direction = number_value(&text.shadow_direction_degrees, evaluation, expressions);
    let shadow_width = number_value(&text.shadow_width, evaluation, expressions).max(0.0);
    let shadow_blur = number_value(&text.shadow_blur, evaluation, expressions).max(0.0);
    let shadow_offset = crate::math::rotate_degrees(
        crate::math::polar_degrees(shadow_distance, shadow_direction),
        -rotation_degrees,
    );
    let padding =
        shrimply_evaluation::resolve_vec2(&text.background_padding, evaluation, expressions)
            .max(glam::Vec2::ZERO);
    let mut outset =
        crate::math::decoration_outset(outline_width, shadow_offset, shadow_width, shadow_blur);
    outset[0] = outset[0].max(padding.y);
    outset[1] = outset[1].max(padding.x);
    outset[2] = outset[2].max(padding.y);
    outset[3] = outset[3].max(padding.x);
    outset
}

struct DeferredTextFrame {
    canvas_size: CanvasSize,
    surface_size: CanvasSize,
    content_offset: glam::Vec2,
    text: TextItem,
    content: String,
    evaluation: shrimply_evaluation::VisualEvaluation,
    rotation_degrees: f32,
    transition: Option<GeneratedTransition>,
}

impl GeneratedVisual for DeferredTextFrame {
    fn draw(
        &self,
        canvas: &Canvas,
        evaluation: &TransformEvaluation,
        expressions: &mut TransformExpressionCache,
        path_effect: Option<&skia_safe::PathEffect>,
    ) {
        self.draw_with_masks(canvas, evaluation, expressions, path_effect, &[]);
    }
}

impl DeferredTextFrame {
    fn draw_with_masks(
        &self,
        canvas: &Canvas,
        evaluation: &TransformEvaluation,
        expressions: &mut TransformExpressionCache,
        path_effect: Option<&skia_safe::PathEffect>,
        text_masks: &[TextMaskOperation],
    ) {
        canvas.save();
        canvas.translate((self.content_offset.x, self.content_offset.y));
        draw_text(
            canvas,
            &self.text,
            &self.content,
            self.rotation_degrees,
            evaluation,
            expressions,
            self.transition,
            self.canvas_size.height.max(1) as f32 * (2.0 / 1080.0),
            path_effect,
            text_masks,
        );
        canvas.restore();
    }
}

struct MaskedTextFrame<'a> {
    frame: &'a DeferredTextFrame,
    masks: &'a [TextMaskOperation],
}

impl GeneratedVisual for MaskedTextFrame<'_> {
    fn draw(
        &self,
        canvas: &Canvas,
        evaluation: &TransformEvaluation,
        expressions: &mut TransformExpressionCache,
        path_effect: Option<&skia_safe::PathEffect>,
    ) {
        self.frame
            .draw_with_masks(canvas, evaluation, expressions, path_effect, self.masks);
    }
}

impl VisualData for DeferredTextFrame {
    fn morph_scene(&self) -> Option<crate::vector_morph::MorphScene> {
        let mut expressions = TransformExpressionCache::default();
        let font_size =
            number_value(&self.text.font_size, &self.evaluation, &mut expressions).max(1.0);
        if self.content.is_empty() {
            return Some(crate::vector_morph::MorphScene {
                objects: Vec::new(),
                evaluation: self.evaluation.clone(),
                canvas_size: self.canvas_size,
            });
        }
        let font_weight = number_value(&self.text.font_weight, &self.evaluation, &mut expressions);
        let tracking = number_value(&self.text.tracking, &self.evaluation, &mut expressions);
        let line_height = number_value(&self.text.line_height, &self.evaluation, &mut expressions)
            .max(f32::EPSILON);
        let layout = crate::text_layout::layout(
            &self.text,
            &self.content,
            font_size,
            font_weight,
            tracking,
            line_height,
            self.evaluation.local_time(),
        );
        let fill = shrimply_evaluation::resolve_color(
            &self.text.color,
            &self.evaluation,
            &mut expressions,
        );
        let outline = shrimply_evaluation::resolve_color(
            &self.text.outline_color,
            &self.evaluation,
            &mut expressions,
        );
        let outline_width =
            number_value(&self.text.outline_width, &self.evaluation, &mut expressions).max(0.0);
        let shadow = shrimply_evaluation::resolve_color(
            &self.text.shadow_color,
            &self.evaluation,
            &mut expressions,
        );
        let shadow_width =
            number_value(&self.text.shadow_width, &self.evaluation, &mut expressions).max(0.0);
        let shadow_blur =
            number_value(&self.text.shadow_blur, &self.evaluation, &mut expressions).max(0.0);
        let shadow_offset = crate::math::rotate_degrees(
            crate::math::polar_degrees(
                number_value(
                    &self.text.shadow_distance,
                    &self.evaluation,
                    &mut expressions,
                )
                .max(0.0),
                number_value(
                    &self.text.shadow_direction_degrees,
                    &self.evaluation,
                    &mut expressions,
                ),
            ),
            -self.rotation_degrees,
        );
        let translate =
            skia_safe::Matrix::translate((self.content_offset.x, self.content_offset.y));
        let appearance = || {
            let mut layers = Vec::new();
            if shadow_offset.length_squared() > f32::EPSILON
                || shadow_width > 0.0
                || shadow_blur > 0.0
            {
                let mut paint = fill_paint(shadow);
                if shadow_width > 0.0 {
                    paint.set_style(PaintStyle::StrokeAndFill);
                    paint.set_stroke_width(shadow_width);
                }
                if shadow_blur > 0.0 {
                    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, shadow_blur, true));
                }
                layers.push(crate::vector_morph::MorphPaintLayer {
                    paint,
                    offset: shadow_offset,
                });
            }
            if outline_width > 0.0 {
                let mut paint = fill_paint(outline);
                paint.set_style(PaintStyle::Stroke);
                paint.set_stroke_width(outline_width);
                layers.push(crate::vector_morph::MorphPaintLayer {
                    paint,
                    offset: glam::Vec2::ZERO,
                });
            }
            layers.push(crate::vector_morph::MorphPaintLayer {
                paint: fill_paint(fill),
                offset: glam::Vec2::ZERO,
            });
            layers
        };
        let mut objects = Vec::new();
        let background = shrimply_evaluation::resolve_color(
            &self.text.background_color,
            &self.evaluation,
            &mut expressions,
        );
        if background.a > 0 {
            let padding = shrimply_evaluation::resolve_vec2(
                &self.text.background_padding,
                &self.evaluation,
                &mut expressions,
            )
            .max(glam::Vec2::ZERO);
            let roundness = number_value(
                &self.text.background_roundness,
                &self.evaluation,
                &mut expressions,
            )
            .max(0.0);
            let path = skia_safe::Path::rrect(
                RRect::new_rect_xy(
                    Rect::from_xywh(
                        -padding.x,
                        -padding.y,
                        layout.size.x + padding.x * 2.0,
                        layout.size.y + padding.y * 2.0,
                    ),
                    roundness,
                    roundness,
                ),
                None,
            )
            .with_transform(&translate);
            objects.push(crate::vector_morph::MorphObject {
                path: crate::vector_morph::skia_path_to_morph(&path),
                appearance: vec![crate::vector_morph::MorphPaintLayer {
                    paint: fill_paint(background),
                    offset: glam::Vec2::ZERO,
                }],
            });
        }
        objects.extend(layout.subpaths.iter().map(|path| {
            let path = path.with_transform(&translate);
            crate::vector_morph::MorphObject {
                path: crate::vector_morph::skia_path_to_morph(&path),
                appearance: appearance(),
            }
        }));
        Some(crate::vector_morph::MorphScene {
            objects,
            evaluation: self.evaluation.clone(),
            canvas_size: self.canvas_size,
        })
    }

    fn rasterize(
        &self,
        compositor: &mut CudaVideoCompositor,
        drawing_strategy: shrimply_project::project::SkiaDrawingStrategy,
        operations: &[crate::layer::VectorOperation],
    ) -> Result<Rc<crate::gpu::VisualFrame>, String> {
        let masks = operations
            .iter()
            .filter_map(|operation| match operation {
                VectorOperation::TextMask(mask) => Some(*mask),
                _ => None,
            })
            .collect::<Vec<_>>();
        if masks.is_empty() {
            return compositor
                .render_generated_visual(
                    self.surface_size,
                    self.canvas_size,
                    self,
                    &self.evaluation,
                    operations,
                    drawing_strategy,
                )
                .map(Rc::new);
        }
        let operations = operations
            .iter()
            .filter(|operation| !matches!(operation, VectorOperation::TextMask(_)))
            .cloned()
            .collect::<Vec<_>>();
        compositor
            .render_generated_visual(
                self.surface_size,
                self.canvas_size,
                &MaskedTextFrame {
                    frame: self,
                    masks: &masks,
                },
                &self.evaluation,
                &operations,
                drawing_strategy,
            )
            .map(Rc::new)
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    canvas: &Canvas,
    text: &TextItem,
    content: &str,
    rotation: f32,
    evaluation: &TransformEvaluation,
    expressions: &mut TransformExpressionCache,
    transition: Option<GeneratedTransition>,
    fallback_trace_width: f32,
    path_effect: Option<&skia_safe::PathEffect>,
    text_masks: &[TextMaskOperation],
) {
    let font_size = number_value(&text.font_size, evaluation, expressions).max(1.0);
    let font_weight = number_value(&text.font_weight, evaluation, expressions);
    let tracking = number_value(&text.tracking, evaluation, expressions);
    let line_height = number_value(&text.line_height, evaluation, expressions).max(f32::EPSILON);
    if content.is_empty() || font_size <= 0.0 {
        return;
    }
    let outline_width = number_value(&text.outline_width, evaluation, expressions).max(0.0);
    let shadow_distance = number_value(&text.shadow_distance, evaluation, expressions).max(0.0);
    let shadow_direction = number_value(&text.shadow_direction_degrees, evaluation, expressions);
    let shadow_width = number_value(&text.shadow_width, evaluation, expressions).max(0.0);
    let shadow_blur = number_value(&text.shadow_blur, evaluation, expressions).max(0.0);
    let layout = crate::text_layout::layout(
        text,
        content,
        font_size,
        font_weight,
        tracking,
        line_height,
        evaluation.local_time(),
    );
    let background_roundness =
        number_value(&text.background_roundness, evaluation, expressions).max(0.0);
    let padding =
        shrimply_evaluation::resolve_vec2(&text.background_padding, evaluation, expressions)
            .max(glam::Vec2::ZERO);
    let subpaths = if transition.is_some_and(|transition| {
        transition.kind == shrimply_project::project::VisualTransitionKind::Morph
            && transition.morph_unit == shrimply_project::project::MorphUnit::Word
    }) {
        &layout.word_subpaths
    } else {
        &layout.subpaths
    };
    let affected = path_effect.map(|effect| {
        let cull = layout.path.bounds().with_outset((font_size, font_size));
        subpaths
            .iter()
            .map(|path| crate::shaky_path::apply(path, effect, cull))
            .collect::<Vec<_>>()
    });
    let paths = affected.as_deref().unwrap_or(subpaths);
    let shadow_offset = crate::math::rotate_degrees(
        crate::math::polar_degrees(shadow_distance, shadow_direction),
        -rotation,
    );
    let origin = glam::Vec2::ZERO;

    let fill_color = shrimply_evaluation::resolve_color(&text.color, evaluation, expressions);
    let background_color =
        shrimply_evaluation::resolve_color(&text.background_color, evaluation, expressions);
    let outline_color =
        shrimply_evaluation::resolve_color(&text.outline_color, evaluation, expressions);
    let shadow_color =
        shrimply_evaluation::resolve_color(&text.shadow_color, evaluation, expressions);
    let color_glyphs = layout.color_glyphs(fill_color);

    if background_color.a > 0 {
        canvas.draw_rrect(
            RRect::new_rect_xy(
                Rect::from_xywh(
                    -padding.x,
                    -padding.y,
                    layout.size.x + padding.x * 2.0,
                    layout.size.y + padding.y * 2.0,
                ),
                background_roundness,
                background_roundness,
            ),
            &fill_paint(background_color),
        );
    }

    let masked = !text_masks.is_empty();
    if masked {
        canvas.save_layer(&SaveLayerRec::default());
    }

    if let Some(transition) = transition {
        draw_text_transition(
            canvas,
            paths,
            transition,
            fill_color,
            outline_color,
            outline_width,
            shadow_color,
            shadow_offset,
            shadow_width,
            shadow_blur,
            fallback_trace_width,
            color_glyphs.as_ref(),
        );
    } else {
        let affected_path = affected.as_ref().map(|paths| {
            let mut combined = PathBuilder::new();
            for path in paths {
                combined.add_path(path, None);
            }
            combined.detach()
        });
        let draw = |canvas: &Canvas, origin: glam::Vec2, paint: &Paint| {
            if let Some(path) = &affected_path {
                canvas.save();
                canvas.translate((origin.x, origin.y));
                canvas.draw_path(path, paint);
                canvas.restore();
            } else {
                draw_layout(canvas, &layout, origin, paint);
            }
        };

        if shadow_distance > 0.0 || shadow_width > 0.0 || shadow_blur > 0.0 {
            let mut paint = fill_paint(shadow_color);
            if shadow_width > 0.0 {
                paint.set_style(PaintStyle::StrokeAndFill);
                paint.set_stroke_width(shadow_width);
            }
            if shadow_blur > 0.0 {
                paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, shadow_blur, true));
            }
            draw(canvas, origin + shadow_offset, &paint);
        }
        if outline_width > 0.0 {
            let mut paint = fill_paint(outline_color);
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(outline_width);
            draw(canvas, origin, &paint);
        }
        draw_text_fill(
            canvas,
            affected_path.as_ref().unwrap_or(&layout.path),
            origin,
            fill_color,
            color_glyphs.as_ref(),
            1.0,
        );
    }

    if masked {
        draw_text_mask(
            canvas,
            &layout,
            text_masks,
            path_effect,
            font_size,
            outline_width,
            shadow_offset,
            shadow_width,
            shadow_blur,
        );
        canvas.restore();
    }
}

fn draw_text_fill(
    canvas: &Canvas,
    path: &skia_safe::Path,
    origin: glam::Vec2,
    fill_color: Color<u8>,
    color_glyphs: Option<&crate::text_layout::ColorGlyphImage>,
    opacity: f32,
) {
    if opacity <= 0.0 {
        return;
    }
    canvas.save();
    canvas.translate((origin.x, origin.y));

    let Some(color_glyphs) = color_glyphs else {
        canvas.draw_path(path, &fill_paint(fill_color.alpha_multiply(opacity)));
        canvas.restore();
        return;
    };

    canvas.save_layer(&SaveLayerRec::default());
    canvas.draw_path(path, &fill_paint(fill_color.alpha_multiply(opacity)));
    canvas.clip_path(path, None, true);
    let mut clear = Paint::default();
    clear.set_blend_mode(BlendMode::Clear);
    canvas.draw_path(&color_glyphs.silhouette, &clear);

    let mut paint = Paint::default();
    paint.set_alpha_f(f32::from(fill_color.a) / f32::from(u8::MAX) * opacity);
    canvas.draw_image(
        color_glyphs.image.as_ref(),
        (color_glyphs.offset.x, color_glyphs.offset.y),
        Some(&paint),
    );
    canvas.restore();
    canvas.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_text_mask(
    canvas: &Canvas,
    layout: &crate::text_layout::TextLayout,
    masks: &[TextMaskOperation],
    path_effect: Option<&skia_safe::PathEffect>,
    font_size: f32,
    outline_width: f32,
    shadow_offset: glam::Vec2,
    shadow_width: f32,
    shadow_blur: f32,
) {
    let mut destination_in = Paint::default();
    destination_in.set_blend_mode(BlendMode::DstIn);
    canvas.save_layer(&SaveLayerRec::default().paint(&destination_in));
    let count = layout.mask_units.len();
    let cull = layout.path.bounds().with_outset((font_size, font_size));
    for (index, unit) in layout.mask_units.iter().enumerate() {
        if unit.path.is_empty() {
            continue;
        }
        let affected = path_effect.map(|effect| crate::shaky_path::apply(&unit.path, effect, cull));
        let path = affected.as_ref().unwrap_or(&unit.path);
        let bounds = path.compute_tight_bounds();
        let [top, right, bottom, left] =
            crate::math::decoration_outset(outline_width, shadow_offset, shadow_width, shadow_blur);
        let bounds = Rect::from_ltrb(
            bounds.left - left,
            bounds.top - top,
            bounds.right + right,
            bounds.bottom + bottom,
        );
        let mut alpha = 1.0;
        let mut clip: Option<Rect> = None;
        let mut visible = true;
        for mask in masks {
            let ordered_index = if matches!(
                mask.direction,
                TextMaskDirection::RightToLeft | TextMaskDirection::BottomToTop
            ) {
                count.saturating_sub(index + 1)
            } else {
                index
            };
            let progress =
                crate::math::lagged_transition_progress(mask.amount, ordered_index, count, 1.0)
                    .clamp(0.0, 1.0);
            match mask.partial_mode {
                TextMaskPartialMode::Fade => alpha *= progress,
                TextMaskPartialMode::Snap => visible &= progress >= SNAP_THRESHOLD,
                TextMaskPartialMode::Clip if progress <= 0.0 => visible = false,
                TextMaskPartialMode::Clip if progress < 1.0 => {
                    let partial = match mask.direction {
                        TextMaskDirection::LeftToRight => Rect::from_ltrb(
                            bounds.left,
                            bounds.top,
                            crate::math::lerp(bounds.left, bounds.right, progress),
                            bounds.bottom,
                        ),
                        TextMaskDirection::RightToLeft => Rect::from_ltrb(
                            crate::math::lerp(bounds.right, bounds.left, progress),
                            bounds.top,
                            bounds.right,
                            bounds.bottom,
                        ),
                        TextMaskDirection::TopToBottom => Rect::from_ltrb(
                            bounds.left,
                            bounds.top,
                            bounds.right,
                            crate::math::lerp(bounds.top, bounds.bottom, progress),
                        ),
                        TextMaskDirection::BottomToTop => Rect::from_ltrb(
                            bounds.left,
                            crate::math::lerp(bounds.bottom, bounds.top, progress),
                            bounds.right,
                            bounds.bottom,
                        ),
                    };
                    clip = match clip {
                        Some(current) => {
                            let mut intersection = current;
                            intersection.intersect(partial).then_some(intersection)
                        }
                        None => Some(partial),
                    };
                    visible &= clip.is_some();
                }
                TextMaskPartialMode::Clip => {}
            }
            if !visible || alpha <= 0.0 {
                break;
            }
        }
        if !visible || alpha <= 0.0 {
            continue;
        }
        canvas.save();
        if let Some(clip) = clip {
            canvas.clip_rect(clip, skia_safe::ClipOp::Intersect, true);
        }
        draw_text_mask_path(
            canvas,
            path,
            outline_width,
            shadow_offset,
            shadow_width,
            shadow_blur,
            alpha,
        );
        canvas.restore();
    }
    canvas.restore();
}

fn draw_text_mask_path(
    canvas: &Canvas,
    path: &skia_safe::Path,
    outline_width: f32,
    shadow_offset: glam::Vec2,
    shadow_width: f32,
    shadow_blur: f32,
    alpha: f32,
) {
    canvas.save_layer_alpha_f(None, alpha);
    let paint = || {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(SkiaColor::WHITE);
        paint
    };
    if shadow_offset.length_squared() > f32::EPSILON || shadow_width > 0.0 || shadow_blur > 0.0 {
        let mut shadow = paint();
        if shadow_width > 0.0 {
            shadow.set_style(PaintStyle::StrokeAndFill);
            shadow.set_stroke_width(shadow_width);
        }
        if shadow_blur > 0.0 {
            shadow.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, shadow_blur, true));
        }
        canvas.save();
        canvas.translate((shadow_offset.x, shadow_offset.y));
        canvas.draw_path(path, &shadow);
        canvas.restore();
    }
    if outline_width > 0.0 {
        let mut outline = paint();
        outline.set_style(PaintStyle::Stroke);
        outline.set_stroke_width(outline_width);
        canvas.draw_path(path, &outline);
    }
    canvas.draw_path(path, &paint());
    canvas.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_text_transition(
    canvas: &Canvas,
    paths: &[skia_safe::Path],
    transition: GeneratedTransition,
    fill_color: Color<u8>,
    outline_color: Color<u8>,
    outline_width: f32,
    shadow_color: Color<u8>,
    shadow_offset: glam::Vec2,
    shadow_width: f32,
    shadow_blur: f32,
    fallback_trace_width: f32,
    color_glyphs: Option<&crate::text_layout::ColorGlyphImage>,
) {
    match transition.kind {
        shrimply_project::project::VisualTransitionKind::Morph => {
            const MORPH_GAP_PORTION: f32 = 0.15;

            let Some(first) = paths.first() else { return };
            let progress = match transition.side {
                shrimply_project::project::TransitionSide::Intro => transition.progress,
                shrimply_project::project::TransitionSide::Outro => 1.0 - transition.progress,
            }
            .clamp(0.0, 1.0);
            if progress >= 1.0 - f32::EPSILON {
                for path in paths {
                    draw_finished_text(
                        canvas,
                        path,
                        fill_color,
                        outline_color,
                        outline_width,
                        shadow_color,
                        shadow_offset,
                        shadow_width,
                        shadow_blur,
                        color_glyphs,
                        1.0,
                    );
                }
                return;
            }

            let stage_progress = progress * paths.len() as f32;
            let stage = stage_progress.floor() as usize;
            let local_progress = transition
                .interpolation
                .value(f64::from(stage_progress.fract())) as f32;
            if stage == 0 {
                draw_finished_text(
                    canvas,
                    first,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    local_progress,
                );
                return;
            }

            let target_index = stage;
            for path in &paths[..target_index] {
                draw_finished_text(
                    canvas,
                    path,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    1.0,
                );
            }
            let source = &paths[target_index - 1];
            let local_progress = transition.interpolation.value(f64::from(
                ((stage_progress.fract() - MORPH_GAP_PORTION) / (1.0 - MORPH_GAP_PORTION))
                    .clamp(0.0, 1.0),
            )) as f32;
            let head =
                crate::path_transition::morphed_path(source, &paths[target_index], local_progress);
            draw_finished_text(
                canvas,
                &head,
                fill_color,
                outline_color,
                outline_width,
                shadow_color,
                shadow_offset,
                shadow_width,
                shadow_blur,
                color_glyphs,
                1.0,
            );
        }
        shrimply_project::project::VisualTransitionKind::Write => {
            let trace_color = if outline_width > 0.0 {
                outline_color
            } else {
                fill_color
            };
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                if progress < 0.5 {
                    let partial = crate::path_transition::partial_path(
                        path,
                        progress * 2.0,
                        transition.side == shrimply_project::project::TransitionSide::Outro,
                    );
                    let mut trace = fill_paint(trace_color);
                    trace.set_style(PaintStyle::Stroke);
                    trace.set_stroke_width(fallback_trace_width.max(1.0));
                    canvas.draw_path(&partial, &trace);
                    continue;
                }

                let style_progress = (progress * 2.0 - 1.0).clamp(0.0, 1.0);
                if shadow_offset.length_squared() > f32::EPSILON
                    || shadow_width > 0.0
                    || shadow_blur > 0.0
                {
                    let mut shadow = fill_paint(shadow_color.alpha_multiply(style_progress));
                    if shadow_width > 0.0 {
                        shadow.set_style(PaintStyle::StrokeAndFill);
                        shadow.set_stroke_width(shadow_width);
                    }
                    if shadow_blur > 0.0 {
                        shadow.set_mask_filter(MaskFilter::blur(
                            BlurStyle::Normal,
                            shadow_blur,
                            true,
                        ));
                    }
                    canvas.save();
                    canvas.translate((shadow_offset.x, shadow_offset.y));
                    canvas.draw_path(path, &shadow);
                    canvas.restore();
                }
                let final_stroke_color = if outline_width > 0.0 {
                    outline_color
                } else {
                    trace_color
                };
                let mut stroke = fill_paint(
                    trace_color.mix_oklaba(final_stroke_color, f64::from(style_progress)),
                );
                stroke.set_style(PaintStyle::Stroke);
                stroke.set_stroke_width(crate::math::lerp(
                    fallback_trace_width.max(1.0),
                    outline_width,
                    style_progress,
                ));
                if stroke.stroke_width() > 0.0 {
                    canvas.draw_path(path, &stroke);
                }
                draw_text_fill(
                    canvas,
                    path,
                    glam::Vec2::ZERO,
                    fill_color,
                    color_glyphs,
                    style_progress,
                );
            }
        }
        shrimply_project::project::VisualTransitionKind::Create => {
            let mut partial = PathBuilder::new();
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                partial.add_path(
                    &crate::path_transition::partial_path(path, progress, false),
                    None,
                );
            }
            draw_finished_text(
                canvas,
                &partial.detach(),
                fill_color,
                outline_color,
                outline_width,
                shadow_color,
                shadow_offset,
                shadow_width,
                shadow_blur,
                color_glyphs,
                1.0,
            );
        }
        shrimply_project::project::VisualTransitionKind::FacetAssembly => {
            const FACETS: usize = 7;
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                let bounds = path.bounds();
                let extent = bounds.width().max(bounds.height());
                let center = Point::new(bounds.center_x(), bounds.center_y());
                for facet in 0..FACETS {
                    let (offset, rotation, scale, facet_opacity) =
                        crate::math::facet_transform(progress, facet, FACETS, extent);
                    let clip = crate::path_transition::facet_clip(*bounds, facet, FACETS);
                    canvas.save();
                    canvas.translate((offset.x, offset.y));
                    canvas.rotate(rotation, Some(center));
                    canvas.translate((center.x, center.y));
                    canvas.scale((scale, scale));
                    canvas.translate((-center.x, -center.y));
                    canvas.clip_path(&clip, None, true);
                    draw_finished_text(
                        canvas,
                        path,
                        fill_color,
                        outline_color,
                        outline_width,
                        shadow_color,
                        shadow_offset,
                        shadow_width,
                        shadow_blur,
                        color_glyphs,
                        facet_opacity,
                    );
                    canvas.restore();
                }

                let finished_opacity = ((progress - 0.82) / 0.18).clamp(0.0, 1.0);
                draw_finished_text(
                    canvas,
                    path,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    finished_opacity,
                );

                if let Some(glint) = crate::path_transition::facet_glint_clip(*bounds, progress) {
                    canvas.save();
                    canvas.clip_path(&glint, None, true);
                    let glint_progress = ((progress - 0.62) / 0.28).clamp(0.0, 1.0);
                    let glint_opacity = (std::f32::consts::PI * glint_progress).sin() * 0.42;
                    let highlight = fill_paint(Color::<u8>::WHITE.alpha_multiply(glint_opacity));
                    canvas.draw_path(path, &highlight);
                    canvas.restore();
                }
            }
        }
        shrimply_project::project::VisualTransitionKind::Coalesce => {
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                let pools = crate::path_transition::coalesce_mask(
                    *path.bounds(),
                    progress,
                    transition.effect_detail.round() as usize,
                );
                let extent = path.bounds().width().max(path.bounds().height());
                let Some(filter) =
                    crate::path_transition::coalesce_filter(extent, transition.effect_amount)
                else {
                    continue;
                };
                canvas.save_layer(&SaveLayerRec::default());
                let mut filtered = Paint::default();
                filtered.set_image_filter(filter);
                canvas.save_layer(&SaveLayerRec::default().paint(&filtered));
                let mut mask = Paint::default();
                mask.set_anti_alias(true);
                mask.set_color(SkiaColor::WHITE);
                canvas.draw_path(&pools, &mask);
                canvas.restore();
                let mut source_in = Paint::default();
                source_in.set_blend_mode(BlendMode::SrcIn);
                canvas.save_layer(&SaveLayerRec::default().paint(&source_in));
                draw_finished_text(
                    canvas,
                    path,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    1.0,
                );
                canvas.restore();
                canvas.restore();
            }
        }
        shrimply_project::project::VisualTransitionKind::ContourCurrent => {
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                draw_finished_text(
                    canvas,
                    path,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    crate::math::vector_reveal_opacity(progress),
                );
                canvas.draw_path(
                    path,
                    &crate::path_transition::contour_current_paint(
                        progress,
                        fallback_trace_width * 1.6 * transition.effect_amount,
                        transition.effect_detail,
                    ),
                );
            }
        }
        shrimply_project::project::VisualTransitionKind::SoftRefraction
        | shrimply_project::project::VisualTransitionKind::MorphologicalResolve => {
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                let extent = path.bounds().width().max(path.bounds().height());
                let filter = match transition.kind {
                    shrimply_project::project::VisualTransitionKind::SoftRefraction => {
                        crate::path_transition::soft_refraction_filter(
                            progress,
                            extent,
                            transition.effect_amount,
                            transition.effect_detail,
                        )
                    }
                    _ => crate::path_transition::morphological_filter(
                        progress,
                        extent,
                        transition.effect_amount,
                        transition.effect_detail,
                    ),
                };
                let Some(filter) = filter else { continue };
                let mut filtered = Paint::default();
                filtered.set_alpha_f(crate::math::vector_reveal_opacity(progress));
                filtered.set_image_filter(filter);
                canvas.save_layer(&SaveLayerRec::default().paint(&filtered));
                draw_finished_text(
                    canvas,
                    path,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    1.0,
                );
                canvas.restore();
            }
        }
        shrimply_project::project::VisualTransitionKind::LivingFill => {
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                let Some(mask) = crate::path_transition::living_fill_paint(
                    *path.bounds(),
                    progress,
                    transition.effect_amount,
                    transition.effect_detail,
                    transition.effect_angle_degrees,
                ) else {
                    continue;
                };
                canvas.save_layer(&SaveLayerRec::default());
                canvas.draw_path(path, &mask);
                let mut source_in = Paint::default();
                source_in.set_blend_mode(BlendMode::SrcIn);
                canvas.save_layer(&SaveLayerRec::default().paint(&source_in));
                draw_finished_text(
                    canvas,
                    path,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    1.0,
                );
                canvas.restore();
                canvas.restore();
            }
        }
        shrimply_project::project::VisualTransitionKind::Diffusion
        | shrimply_project::project::VisualTransitionKind::ReverseDiffusion => {
            for (index, path) in paths.iter().enumerate() {
                let progress =
                    crate::path_transition::submobject_progress(transition, index, paths.len());
                let bounds = path
                    .bounds()
                    .with_outset((path.bounds().width(), path.bounds().height()));
                let diffused = crate::path_transition::diffused_path(
                    path,
                    bounds,
                    progress,
                    (transition.kind
                        == shrimply_project::project::VisualTransitionKind::ReverseDiffusion)
                        != (transition.side == shrimply_project::project::TransitionSide::Outro),
                    transition.effect_amount,
                    transition.effect_detail,
                    transition.effect_seed,
                );
                let opacity = if transition.effect_fade {
                    crate::math::vector_reveal_opacity(progress)
                } else {
                    1.0
                };
                draw_finished_text(
                    canvas,
                    &diffused,
                    fill_color,
                    outline_color,
                    outline_width,
                    shadow_color,
                    shadow_offset,
                    shadow_width,
                    shadow_blur,
                    color_glyphs,
                    opacity,
                );
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_finished_text(
    canvas: &Canvas,
    path: &skia_safe::Path,
    fill_color: Color<u8>,
    outline_color: Color<u8>,
    outline_width: f32,
    shadow_color: Color<u8>,
    shadow_offset: glam::Vec2,
    shadow_width: f32,
    shadow_blur: f32,
    color_glyphs: Option<&crate::text_layout::ColorGlyphImage>,
    opacity: f32,
) {
    if opacity <= 0.0 {
        return;
    }
    if shadow_offset.length_squared() > f32::EPSILON || shadow_width > 0.0 || shadow_blur > 0.0 {
        let mut paint = fill_paint(shadow_color.alpha_multiply(opacity));
        if shadow_width > 0.0 {
            paint.set_style(PaintStyle::StrokeAndFill);
            paint.set_stroke_width(shadow_width);
        }
        if shadow_blur > 0.0 {
            paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, shadow_blur, true));
        }
        canvas.save();
        canvas.translate((shadow_offset.x, shadow_offset.y));
        canvas.draw_path(path, &paint);
        canvas.restore();
    }
    if outline_width > 0.0 {
        let mut paint = fill_paint(outline_color.alpha_multiply(opacity));
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(outline_width);
        canvas.draw_path(path, &paint);
    }
    draw_text_fill(
        canvas,
        path,
        glam::Vec2::ZERO,
        fill_color,
        color_glyphs,
        opacity,
    );
}

fn number_value(
    number: &TimelineValue<f32>,
    evaluation: &TransformEvaluation,
    expressions: &mut TransformExpressionCache,
) -> f32 {
    shrimply_evaluation::resolve_scalar(number, evaluation, expressions)
}

fn draw_layout(
    canvas: &Canvas,
    layout: &crate::text_layout::TextLayout,
    origin: glam::Vec2,
    paint: &Paint,
) {
    canvas.save();
    canvas.translate((origin.x, origin.y));
    canvas.draw_path(&layout.path, paint);
    canvas.restore();
}

fn fill_paint(color: Color<u8>) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Fill);
    paint.set_color(color);
    paint
}
