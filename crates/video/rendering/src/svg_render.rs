use std::rc::Rc;

use cached::{Cached, stores::LruCache};
use serde::Serialize;
use shrimply_asset::{Asset, AssetSnapshot};
use shrimply_visual_frame::Device;
use skia_safe::{Canvas, FontMgr, svg::Dom};
use uuid::Uuid;

use crate::gpu::CudaVideoCompositor;
use crate::gpu::generated_gpu::GeneratedVisual;
use crate::layer::{VectorOperation, VectorVisual, Visual, VisualData, VisualState};
use crate::svg_color;
use crate::visual_source::VisualSourceCache;
use crate::visual_source::{GeneratedTransition, VisualElement, VisualRender, VisualRenderRequest};
use shrimply_project::project::{CanvasSize, VideoItem};

pub struct SvgRenderSession {
    file: Asset,
    snapshot: AssetSnapshot,
    svg: String,
    dom: Rc<Dom>,
    root_width: u32,
    root_height: u32,
    surface_width: u32,
    surface_height: u32,
    color_overrides: Vec<shrimply_project::project::SvgColorOverride>,
}

const MAX_SVG_RASTER_CACHE_ENTRIES: usize = 8;

pub(crate) struct SvgRasterCache(LruCache<Vec<u8>, Rc<crate::gpu::VisualFrame>>);

impl Default for SvgRasterCache {
    fn default() -> Self {
        Self(
            LruCache::builder()
                .max_size(MAX_SVG_RASTER_CACHE_ENTRIES)
                .build()
                .expect("valid SVG raster cache size"),
        )
    }
}

impl SvgRasterCache {
    fn get(&mut self, key: &[u8]) -> Option<Rc<crate::gpu::VisualFrame>> {
        self.0.cache_get(key).map(Rc::clone)
    }

    fn set(&mut self, key: Vec<u8>, frame: Rc<crate::gpu::VisualFrame>) {
        debug_assert_eq!(frame.device(), Device::Cpu);
        self.0.cache_set(key, frame);
        shrimply_benchmarking::set_counter(
            "SVG raster cache / CPU bytes retained",
            self.0.value_order().iter().map(|frame| frame.bytes()).sum(),
        );
    }
}

struct DeferredSvgFrame {
    cache_key: Vec<u8>,
    svg: String,
    dom: Option<Rc<Dom>>,
    root_width: u32,
    root_height: u32,
    surface_width: u32,
    surface_height: u32,
    canvas_size: CanvasSize,
    evaluation: shrimply_evaluation::VisualEvaluation,
    transition: Option<GeneratedTransition>,
}

pub(crate) struct SvgVectorVisualParams {
    pub item_id: Uuid,
    pub svg: String,
    pub dom: Option<Rc<Dom>>,
    pub root_size: CanvasSize,
    pub surface_size: CanvasSize,
    pub canvas_size: CanvasSize,
    pub evaluation: shrimply_evaluation::VisualEvaluation,
    pub transition: Option<GeneratedTransition>,
}

pub(crate) fn svg_vector_visual(
    params: SvgVectorVisualParams,
    state: VisualState,
) -> Result<Visual, String> {
    let SvgVectorVisualParams {
        item_id,
        svg,
        dom,
        root_size,
        surface_size,
        canvas_size,
        evaluation,
        transition,
    } = params;
    let cache_key = serde_json::to_vec(&(
        item_id,
        &svg,
        root_size.width,
        root_size.height,
        surface_size.width,
        surface_size.height,
        canvas_size,
        transition.map(|value| {
            (
                value.kind,
                value.side == shrimply_project::project::TransitionSide::Intro,
                value.progress.to_bits(),
                value.interpolation,
                value.ordering,
                value.drawing_stroke_overlap.to_bits(),
                value.drawing_stroke_length_weight.to_bits(),
                value.drawing_fill_mode,
                value.morph_unit,
                value.effect_amount.to_bits(),
                value.effect_detail.to_bits(),
                value.effect_angle_degrees.to_bits(),
                value.effect_fade,
                value.effect_seed,
            )
        }),
    ))
    .map_err(|error| format!("serialize SVG raster cache key: {error}"))?;

    let frame = Box::new(DeferredSvgFrame {
        cache_key,
        svg,
        dom,
        root_width: root_size.width,
        root_height: root_size.height,
        surface_width: surface_size.width,
        surface_height: surface_size.height,
        canvas_size,
        evaluation,
        transition,
    });
    Ok(Visual::Vector(VectorVisual::prepared(frame, state)))
}

#[derive(Serialize)]
enum SvgVectorOperationKey {
    Transform([u32; 9]),
    MotionBlur(Vec<[u32; 9]>),
    Opacity(u32),
    Hsv {
        hue_turns: u32,
        saturation: u32,
        value: u32,
    },
    Repeat {
        copies_x: u32,
        copies_y: u32,
        step: [u32; 2],
        row_offset: [u32; 2],
    },
    ShakyPath {
        amplitude: u32,
        step_size: u32,
        seed: u32,
    },
    TextMask {
        amount: u32,
        partial_mode: shrimply_video_modifiers::text_mask::TextMaskPartialMode,
        direction: shrimply_video_modifiers::text_mask::TextMaskDirection,
    },
}

fn transform_key(transform: shrimply_math_geometry::ComposedTransform2D) -> [u32; 9] {
    transform.matrix.to_cols_array().map(f32::to_bits)
}

fn operation_key(operation: &VectorOperation) -> SvgVectorOperationKey {
    match operation {
        VectorOperation::Transform(transform) => {
            SvgVectorOperationKey::Transform(transform_key(*transform))
        }
        VectorOperation::MotionBlur(transforms) => SvgVectorOperationKey::MotionBlur(
            transforms.iter().copied().map(transform_key).collect(),
        ),
        VectorOperation::Opacity(opacity) => SvgVectorOperationKey::Opacity(opacity.to_bits()),
        VectorOperation::Hsv {
            hue_turns,
            saturation,
            value,
        } => SvgVectorOperationKey::Hsv {
            hue_turns: hue_turns.to_bits(),
            saturation: saturation.to_bits(),
            value: value.to_bits(),
        },
        VectorOperation::Repeat {
            copies_x,
            copies_y,
            step,
            row_offset,
        } => SvgVectorOperationKey::Repeat {
            copies_x: *copies_x,
            copies_y: *copies_y,
            step: [step.x.to_bits(), step.y.to_bits()],
            row_offset: [row_offset.x.to_bits(), row_offset.y.to_bits()],
        },
        VectorOperation::ShakyPath {
            amplitude,
            step_size,
            seed,
        } => SvgVectorOperationKey::ShakyPath {
            amplitude: amplitude.to_bits(),
            step_size: step_size.to_bits(),
            seed: *seed,
        },
        VectorOperation::TextMask(mask) => SvgVectorOperationKey::TextMask {
            amount: mask.amount.to_bits(),
            partial_mode: mask.partial_mode,
            direction: mask.direction,
        },
    }
}

impl GeneratedVisual for DeferredSvgFrame {
    fn draw(
        &self,
        canvas: &Canvas,
        _evaluation: &shrimply_evaluation::TransformEvaluation,
        _expressions: &mut shrimply_evaluation::TransformExpressionCache,
        path_effect: Option<&skia_safe::PathEffect>,
    ) {
        let dom = self.dom.clone().unwrap_or_else(|| {
            Rc::new(
                Dom::from_str(&self.svg, FontMgr::new())
                    .expect("validated SVG should remain parseable while rendering"),
            )
        });
        let mut root = dom.root();
        root.set_width(skia_safe::svg::Length::new(
            self.root_width as f32,
            skia_safe::svg::LengthUnit::PX,
        ));
        root.set_height(skia_safe::svg::Length::new(
            self.root_height as f32,
            skia_safe::svg::LengthUnit::PX,
        ));

        let Some(transition) = self.transition else {
            if let Some(path_effect) = path_effect
                && crate::svg_transition::draw_shaky(
                    &dom,
                    &root,
                    canvas,
                    path_effect,
                    self.root_width as f32,
                    self.root_height as f32,
                )
            {
                return;
            }
            dom.render(canvas);
            return;
        };
        if path_effect.is_none()
            && ((transition.side == shrimply_project::project::TransitionSide::Intro
                && transition.progress >= 1.0)
                || (transition.side == shrimply_project::project::TransitionSide::Outro
                    && transition.progress <= 0.0))
        {
            dom.render(canvas);
            return;
        }
        if !crate::svg_transition::draw(
            &dom,
            &root,
            canvas,
            transition,
            self.root_width as f32,
            self.root_height as f32,
            path_effect,
        ) {
            dom.render(canvas);
        }
    }
}

impl SvgRenderSession {
    pub fn new(item: &VideoItem, canvas_size: CanvasSize) -> Result<Self, String> {
        let snapshot = item.file.snapshot()?;
        let source = snapshot.read_to_string()?;
        let color_overrides = item.svg_color_overrides.clone();
        let svg = svg_color::apply_overrides(&source, &color_overrides);
        let dom = Dom::from_str(&svg, FontMgr::new())
            .map_err(|error| format!("could not parse SVG {}: {error}", item.file.display()))?;
        Ok(Self {
            file: item.file.clone(),
            snapshot,
            svg,
            dom: Rc::new(dom),
            root_width: item.source_width.max(1),
            root_height: item.source_height.max(1),
            surface_width: canvas_size.width.max(1),
            surface_height: canvas_size.height.max(1),
            color_overrides,
        })
    }

    fn matches_item(&self, item: &VideoItem, canvas_size: CanvasSize) -> bool {
        self.file == item.file
            && self.snapshot.is_current()
            && self.root_width == item.source_width.max(1)
            && self.root_height == item.source_height.max(1)
            && self.surface_width == canvas_size.width.max(1)
            && self.surface_height == canvas_size.height.max(1)
            && self.color_overrides == item.svg_color_overrides
    }
}

impl VisualData for DeferredSvgFrame {
    fn cache_key(&self) -> &[u8] {
        &self.cache_key
    }

    fn morph_scene(&self) -> Option<crate::vector_morph::MorphScene> {
        let dom = self
            .dom
            .clone()
            .or_else(|| Dom::from_str(&self.svg, FontMgr::new()).ok().map(Rc::new))?;
        let mut root = dom.root();
        root.set_width(skia_safe::svg::Length::new(
            self.root_width as f32,
            skia_safe::svg::LengthUnit::PX,
        ));
        root.set_height(skia_safe::svg::Length::new(
            self.root_height as f32,
            skia_safe::svg::LengthUnit::PX,
        ));
        let objects = crate::svg_transition::svg_paths(
            &root,
            self.root_width as f32,
            self.root_height as f32,
        )
        .into_iter()
        .map(|path| {
            let mut appearance = Vec::new();
            if path.fill {
                let mut paint = skia_safe::Paint::default();
                paint.set_anti_alias(true);
                paint.set_color(path.fill_color);
                paint.set_alpha_f(paint.alpha_f() * path.fill_opacity);
                appearance.push(crate::vector_morph::MorphPaintLayer {
                    paint,
                    offset: glam::Vec2::ZERO,
                });
            }
            if path.stroke_width > 0.0 {
                let mut paint = skia_safe::Paint::default();
                paint.set_anti_alias(true);
                paint.set_style(skia_safe::PaintStyle::Stroke);
                paint.set_stroke_width(path.stroke_width);
                paint.set_color(path.stroke_color);
                paint.set_alpha_f(paint.alpha_f() * path.stroke_opacity);
                appearance.push(crate::vector_morph::MorphPaintLayer {
                    paint,
                    offset: glam::Vec2::ZERO,
                });
            }
            crate::vector_morph::MorphObject {
                path: crate::vector_morph::skia_path_to_morph(&path.path),
                appearance,
            }
        })
        .collect();
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
        let operation_keys = operations.iter().map(operation_key).collect::<Vec<_>>();
        let cache_key = |renderer_generation| {
            serde_json::to_vec(&(
                &self.cache_key,
                drawing_strategy,
                renderer_generation,
                &operation_keys,
            ))
            .map_err(|error| format!("serialize SVG raster cache key: {error}"))
        };
        let key = cache_key(compositor.generated_renderer_generation())?;
        if let Some(frame) = compositor.svg_rasters.get(&key) {
            shrimply_benchmarking::increment("SVG raster cache / Hit");
            return compositor.upload_frame(&frame).map(Rc::new);
        }

        shrimply_benchmarking::increment("SVG raster cache / Miss");
        let frame = Rc::new(compositor.render_generated_visual(
            CanvasSize {
                width: self.surface_width,
                height: self.surface_height,
            },
            self.canvas_size,
            self,
            &self.evaluation,
            operations,
            drawing_strategy,
        )?);
        let key = cache_key(compositor.generated_renderer_generation())?;
        compositor
            .svg_rasters
            .set(key, Rc::new(frame.copy_to(Device::Cpu)?));
        Ok(frame)
    }
}

impl VisualElement for SvgRenderSession {
    fn matches(&self, item: &VideoItem, canvas_size: CanvasSize) -> bool {
        self.matches_item(item, canvas_size)
    }

    fn draw(
        &mut self,
        request: VisualRenderRequest<'_>,
        _compositor: &mut CudaVideoCompositor,
        _track_id: Uuid,
        _cache: &mut VisualSourceCache,
    ) -> Result<VisualRender, String> {
        let evaluation = shrimply_evaluation::VisualEvaluation::for_item_with_audio(
            request.project,
            request.item,
            request.position,
            request.audio_analysis,
        );
        svg_vector_visual(
            SvgVectorVisualParams {
                item_id: request.item.id,
                svg: self.svg.clone(),
                dom: Some(self.dom.clone()),
                root_size: CanvasSize {
                    width: self.root_width,
                    height: self.root_height,
                },
                surface_size: CanvasSize {
                    width: self.surface_width,
                    height: self.surface_height,
                },
                canvas_size: request.project.canvas_size,
                evaluation,
                transition: request.generated_transition,
            },
            request.state,
        )
        .map(VisualRender::Ready)
    }
}
