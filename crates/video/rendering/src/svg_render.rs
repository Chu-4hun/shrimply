use std::rc::Rc;

use shrimply_asset::{Asset, AssetSnapshot};
use shrimply_gpu_memory::HostReservation;
use skia_safe::{Canvas, FontMgr, svg::Dom};
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
    svg: String,
    dom: Rc<Dom>,
    root_width: u32,
    root_height: u32,
    surface_width: u32,
    surface_height: u32,
    color_overrides: Vec<shrimply_project::project::SvgColorOverride>,
    _host_reservation: Option<HostReservation>,
}

struct DeferredSvgFrame {
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
        svg,
        dom,
        root_size,
        surface_size,
        canvas_size,
        evaluation,
        transition,
    } = params;
    let frame = Box::new(DeferredSvgFrame {
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
        let source_bytes = u64::try_from(svg.len())
            .map_err(|_| format!("SVG {} source size exceeds u64", item.file.display()))?;
        let memory = shrimply_gpu_memory::global();
        let host_reservation = (memory.telemetry().host_budget_bytes != 0)
            .then(|| memory.reserve_host(source_bytes))
            .transpose()?;
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
            _host_reservation: host_reservation,
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
        let evaluation = shrimply_evaluation::VisualEvaluation::for_item_with_audio(
            request.project,
            request.item,
            request.position,
            request.audio_analysis,
        );
        svg_vector_visual(
            SvgVectorVisualParams {
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
