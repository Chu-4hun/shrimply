use std::rc::Rc;
use std::sync::Mutex;

use shrimply_asset::{Asset, AssetSnapshot};
use shrimply_gpu_memory::{ResidentResource, ResourceKey, global as gpu_memory};
use skia_safe::{Canvas, ConditionallySend, FontMgr, Sendable, svg::Dom};
use uuid::Uuid;

use crate::gpu::CudaVideoCompositor;
use crate::gpu::generated_gpu::GeneratedVisual;
use crate::layer::{VectorVisual, Visual, VisualData, VisualState};
use crate::svg_color;
use crate::visual_source::VisualSourceCache;
use crate::visual_source::{GeneratedTransition, VisualElement, VisualRender, VisualRenderRequest};
use shrimply_project::project::{CanvasSize, VideoItem};

pub struct SvgRenderSession {
    file: Asset,
    snapshot: AssetSnapshot,
    source_key: ResourceKey,
    root_width: u32,
    root_height: u32,
    surface_width: u32,
    surface_height: u32,
    color_overrides: Vec<shrimply_project::project::SvgColorOverride>,
}

struct DeferredSvgFrame {
    cache_key: Vec<u8>,
    prepared_svg: ResidentResource<PreparedSvg>,
    root_width: u32,
    root_height: u32,
    surface_width: u32,
    surface_height: u32,
    canvas_size: CanvasSize,
    evaluation: shrimply_evaluation::VisualEvaluation,
    transition: Option<GeneratedTransition>,
}

pub(crate) struct SvgVectorVisualParams {
    pub cache_key: Vec<u8>,
    pub prepared_svg: ResidentResource<PreparedSvg>,
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
        cache_key,
        prepared_svg,
        root_size,
        surface_size,
        canvas_size,
        evaluation,
        transition,
    } = params;
    let cache_key = serde_json::to_vec(&(
        cache_key,
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
        prepared_svg,
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

pub(crate) struct PreparedSvg {
    _source: String,
    dom: Mutex<Option<Sendable<Dom>>>,
}

impl PreparedSvg {
    pub(crate) fn new(source: String) -> Result<Self, String> {
        let dom = Dom::from_str(&source, FontMgr::new())
            .map_err(|error| format!("could not parse SVG: {error}"))?;
        let dom = dom
            .wrap_send()
            .map_err(|_| "parsed SVG was not uniquely owned".to_string())?;
        Ok(Self {
            _source: source,
            dom: Mutex::new(Some(dom)),
        })
    }

    fn with_dom<T>(&self, operation: impl FnOnce(&Dom) -> T) -> T {
        let mut stored = self.dom.lock().expect("parsed SVG mutex poisoned");
        let dom = stored
            .take()
            .expect("parsed SVG disappeared from its residency entry")
            .into_inner();
        let result = operation(&dom);
        *stored = Some(
            dom.wrap_send()
                .unwrap_or_else(|_| panic!("parsed SVG escaped while rendering")),
        );
        result
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
        self.prepared_svg.with_dom(|dom| {
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
                        dom,
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
                dom,
                &root,
                canvas,
                transition,
                self.root_width as f32,
                self.root_height as f32,
                path_effect,
            ) {
                dom.render(canvas);
            }
        });
    }
}

impl SvgRenderSession {
    pub fn new(item: &VideoItem, canvas_size: CanvasSize) -> Result<Self, String> {
        let snapshot = item.file.snapshot()?;
        let source = snapshot.read_to_string()?;
        let color_overrides = item.svg_color_overrides.clone();
        let svg = svg_color::apply_overrides(&source, &color_overrides);
        let source_bytes = u64::try_from(svg.len())
            .map_err(|_| format!("SVG {} source size exceeds u64", item.file.display()))?;
        let source_key = svg_source_key(&snapshot, &color_overrides)?;
        gpu_memory().insert_resource(
            source_key.clone(),
            source_bytes,
            PreparedSvg::new(svg).map_err(|error| format!("{} {}", item.file.display(), error))?,
        )?;
        Ok(Self {
            file: item.file.clone(),
            snapshot,
            source_key,
            root_width: item.source_width.max(1),
            root_height: item.source_height.max(1),
            surface_width: canvas_size.width.max(1),
            surface_height: canvas_size.height.max(1),
            color_overrides,
        })
    }

    fn source(&mut self) -> Result<ResidentResource<PreparedSvg>, String> {
        if let Some(source) = gpu_memory().get_resource(&self.source_key)? {
            return Ok(source);
        }
        let source = self.snapshot.read_to_string()?;
        self.snapshot.verify_current()?;
        let svg = svg_color::apply_overrides(&source, &self.color_overrides);
        let source_bytes = u64::try_from(svg.len())
            .map_err(|_| format!("SVG {} source size exceeds u64", self.file.display()))?;
        gpu_memory().insert_resource(
            self.source_key.clone(),
            source_bytes,
            PreparedSvg::new(svg).map_err(|error| format!("{} {}", self.file.display(), error))?,
        )?;
        gpu_memory()
            .get_resource(&self.source_key)?
            .ok_or_else(|| "reconstructed SVG source disappeared".to_string())
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
        self.prepared_svg.with_dom(|dom| {
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
        })
    }

    fn rasterize(
        &self,
        compositor: &mut CudaVideoCompositor,
        drawing_strategy: shrimply_project::project::SkiaDrawingStrategy,
        operations: &[crate::layer::VectorOperation],
    ) -> Result<Rc<crate::gpu::VisualFrame>, String> {
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
        let source = self.source()?;
        let evaluation = shrimply_evaluation::VisualEvaluation::for_item_with_audio(
            request.project,
            request.item,
            request.position,
            request.audio_analysis,
        );
        svg_vector_visual(
            SvgVectorVisualParams {
                cache_key: serde_json::to_vec(&(self.snapshot.cache_key(), &self.color_overrides))
                    .map_err(|error| format!("serialize SVG source cache key: {error}"))?,
                prepared_svg: source,
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

fn svg_source_key(
    snapshot: &AssetSnapshot,
    color_overrides: &[shrimply_project::project::SvgColorOverride],
) -> Result<ResourceKey, String> {
    let mut discriminator = b"svg-source\0".to_vec();
    discriminator.extend_from_slice(snapshot.cache_key().as_bytes());
    discriminator.extend_from_slice(
        &serde_json::to_vec(color_overrides)
            .map_err(|error| format!("serialize SVG color overrides: {error}"))?,
    );
    Ok(ResourceKey::new(
        snapshot.path().to_path_buf(),
        discriminator,
    ))
}
