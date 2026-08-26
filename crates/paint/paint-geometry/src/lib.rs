mod math;

use std::hash::{DefaultHasher, Hash, Hasher};

pub use math::{
    EraserInterval, PolylineHit, ResolvedPathOffset, apply_path_offset, apply_path_offsets,
    eraser_sweep_intervals, partial_stroke_points, point_hits_eraser_sweep,
    point_in_even_odd_loops, point_to_polyline, point_to_polyline_distance,
    polyline_intersects_even_odd_loops, pressure_diameter_scale, simplified_point_indices,
    simplify_points, subdivide_points,
};
pub use shrimply_paint_topology::{Face, Topology};

use glam::{Mat3, UVec2, Vec2};
use rayon::prelude::*;
use shrimply_math_geometry::ResolvedTransform2D;
use shrimply_paint_model::{
    PaintDrawing, PaintFill, PaintStroke, PaintTaper, ResolvedPaintFillOptions,
    ResolvedPaintStrokeEndOptions, ResolvedPaintStrokeOptions,
};
use shrimply_perfect_freehand::{
    InputPoint, StrokeEndOptions, StrokeOptions, StrokePoint, Taper, get_stroke_outline_points,
    get_stroke_points,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PreprocessingKey {
    pub simplification_tolerance: u32,
    pub maximum_subdivision_spacing: u32,
    pub streamline: u32,
    pub stroke_width: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TransformKey {
    pub position: UVec2,
    pub anchor: UVec2,
    pub scale: UVec2,
    pub shear: UVec2,
    pub rotation_degrees: u32,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CenterlineKey {
    pub revision: u64,
    pub content_hash: u64,
    pub preprocessing: PreprocessingKey,
    pub transform: TransformKey,
    pub canvas_size: UVec2,
    pub path_offsets: Vec<PathOffsetKey>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GeometryKey {
    pub centerlines: CenterlineKey,
    pub closure_tolerance: u32,
    pub stroke_topology: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PathOffsetKey {
    pub amplitude: u32,
    pub spacing: u32,
    pub seed: u32,
    pub evolution: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StrokeShapeKey {
    pub width: u32,
    pub thinning: u32,
    pub smoothing: u32,
    pub start: StrokeEndKey,
    pub end: StrokeEndKey,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StrokeEndKey {
    pub cap: bool,
    pub taper: TaperKey,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaperKey {
    None,
    Full,
    Distance(u32),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OutlineKey {
    pub centerlines: CenterlineKey,
    pub shape: StrokeShapeKey,
}

#[derive(Clone, Debug)]
pub struct PreparedCenterline {
    pub stroke_id: Uuid,
    pub width: f32,
    pub color_index: usize,
    pub stroke_points: Vec<StrokePoint>,
    pub simulate_pressure: bool,
    pub completed: bool,
}

impl PreparedCenterline {
    pub fn points(&self) -> impl ExactSizeIterator<Item = Vec2> + '_ {
        self.stroke_points.iter().map(|point| point.point)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFill {
    pub fill_id: Uuid,
    pub color_index: usize,
    pub seed: Vec2,
    pub loops: Vec<Vec<Vec2>>,
}

#[derive(Clone, Debug)]
pub struct PreparedGeometry {
    pub key: GeometryKey,
    pub centerlines: Vec<PreparedCenterline>,
    pub topology: Topology,
    pub fills: Vec<PreparedFill>,
}

#[derive(Clone, Debug, Default)]
pub struct ObjectSweepHits {
    pub stroke_ids: Vec<Uuid>,
    pub fill_ids: Vec<Uuid>,
}

#[derive(Clone, Debug)]
pub struct PressureOutline {
    pub stroke_id: Uuid,
    pub color_index: usize,
    pub points: Vec<Vec2>,
}

#[derive(Clone, Debug)]
pub struct PreparedOutlines {
    pub key: OutlineKey,
    pub outlines: Vec<PressureOutline>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeHit {
    pub stroke_index: usize,
    pub stroke_id: Uuid,
    pub polyline: PolylineHit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleHit {
    pub stroke_index: usize,
    pub stroke_id: Uuid,
    pub sample_index: usize,
    pub position: Vec2,
    pub distance: f32,
}

pub fn centerline_key(
    drawing: &PaintDrawing,
    revision: u64,
    options: &ResolvedPaintStrokeOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
) -> CenterlineKey {
    centerline_key_with_path_offsets(drawing, revision, options, transform, canvas_size, &[])
}

pub fn centerline_key_with_path_offsets(
    drawing: &PaintDrawing,
    revision: u64,
    options: &ResolvedPaintStrokeOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
    path_offsets: &[ResolvedPathOffset],
) -> CenterlineKey {
    CenterlineKey {
        revision,
        content_hash: paint_content_hash(drawing, revision),
        preprocessing: PreprocessingKey {
            simplification_tolerance: options.simplification_tolerance.to_bits(),
            maximum_subdivision_spacing: options.maximum_subdivision_spacing.to_bits(),
            streamline: options.streamline.to_bits(),
            stroke_width: options.width.to_bits(),
        },
        transform: TransformKey::from(transform),
        canvas_size: UVec2::from_array(canvas_size.to_array().map(f32::to_bits)),
        path_offsets: path_offsets
            .iter()
            .copied()
            .map(PathOffsetKey::from)
            .collect(),
    }
}

pub fn paint_content_hash(drawing: &PaintDrawing, revision: u64) -> u64 {
    let mut hash = DefaultHasher::new();
    revision.hash(&mut hash);
    drawing.strokes.len().hash(&mut hash);
    for stroke in &drawing.strokes {
        stroke.id.hash(&mut hash);
        stroke.width_scale.to_bits().hash(&mut hash);
        stroke.color_index.hash(&mut hash);
        stroke.points.len().hash(&mut hash);
        for point in &stroke.points {
            point.position.x.to_bits().hash(&mut hash);
            point.position.y.to_bits().hash(&mut hash);
            point.pressure.map(f32::to_bits).hash(&mut hash);
        }
    }
    drawing.fills.len().hash(&mut hash);
    for fill in &drawing.fills {
        fill.id.hash(&mut hash);
        fill.seed.x.to_bits().hash(&mut hash);
        fill.seed.y.to_bits().hash(&mut hash);
        fill.color_index.hash(&mut hash);
        fill.loops.len().hash(&mut hash);
        for boundary in &fill.loops {
            boundary.len().hash(&mut hash);
            for point in boundary {
                point.x.to_bits().hash(&mut hash);
                point.y.to_bits().hash(&mut hash);
            }
        }
    }
    hash.finish()
}

pub fn geometry_key(
    drawing: &PaintDrawing,
    revision: u64,
    stroke: &ResolvedPaintStrokeOptions,
    fill: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
) -> GeometryKey {
    geometry_key_with_topology(
        (drawing, revision),
        stroke,
        fill,
        transform,
        canvas_size,
        true,
        &[],
    )
}

pub fn render_geometry_key(
    drawing: &PaintDrawing,
    revision: u64,
    stroke: &ResolvedPaintStrokeOptions,
    fill: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
) -> GeometryKey {
    geometry_key_with_topology(
        (drawing, revision),
        stroke,
        fill,
        transform,
        canvas_size,
        drawing.fills.iter().any(|fill| fill.loops.is_empty()),
        &[],
    )
}

pub fn render_geometry_key_with_path_offsets(
    drawing: &PaintDrawing,
    revision: u64,
    stroke: &ResolvedPaintStrokeOptions,
    fill: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
    path_offsets: &[ResolvedPathOffset],
) -> GeometryKey {
    geometry_key_with_topology(
        (drawing, revision),
        stroke,
        fill,
        transform,
        canvas_size,
        drawing.fills.iter().any(|fill| fill.loops.is_empty()),
        path_offsets,
    )
}

fn geometry_key_with_topology(
    content: (&PaintDrawing, u64),
    stroke: &ResolvedPaintStrokeOptions,
    fill: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
    stroke_topology: bool,
    path_offsets: &[ResolvedPathOffset],
) -> GeometryKey {
    let (drawing, revision) = content;
    GeometryKey {
        centerlines: centerline_key_with_path_offsets(
            drawing,
            revision,
            stroke,
            transform,
            canvas_size,
            path_offsets,
        ),
        closure_tolerance: if stroke_topology {
            fill.closure_tolerance.to_bits()
        } else {
            0
        },
        stroke_topology,
    }
}

pub fn outline_key(
    drawing: &PaintDrawing,
    revision: u64,
    options: &ResolvedPaintStrokeOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
) -> OutlineKey {
    OutlineKey {
        centerlines: centerline_key(drawing, revision, options, transform, canvas_size),
        shape: StrokeShapeKey::from(options),
    }
}

/// Prepares transformed centerlines, fill topology, and transformed fill seeds.
pub fn prepare_geometry(
    drawing: &PaintDrawing,
    revision: u64,
    stroke_options: &ResolvedPaintStrokeOptions,
    fill_options: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
) -> PreparedGeometry {
    prepare_geometry_with_path_offsets(
        drawing,
        revision,
        stroke_options,
        fill_options,
        transform,
        canvas_size,
        &[],
    )
}

pub fn prepare_geometry_with_path_offsets(
    drawing: &PaintDrawing,
    revision: u64,
    stroke_options: &ResolvedPaintStrokeOptions,
    fill_options: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
    path_offsets: &[ResolvedPathOffset],
) -> PreparedGeometry {
    prepare_geometry_with_topology(
        (drawing, revision),
        stroke_options,
        fill_options,
        transform,
        canvas_size,
        true,
        path_offsets,
    )
}

/// Prepares render geometry without constructing stroke topology until a fill exists.
///
/// Editing uses [`prepare_geometry`] so the Fill tool can discover a first region.
pub fn prepare_render_geometry(
    drawing: &PaintDrawing,
    revision: u64,
    stroke_options: &ResolvedPaintStrokeOptions,
    fill_options: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
) -> PreparedGeometry {
    prepare_render_geometry_with_path_offsets(
        drawing,
        revision,
        stroke_options,
        fill_options,
        transform,
        canvas_size,
        &[],
    )
}

pub fn prepare_render_geometry_with_path_offsets(
    drawing: &PaintDrawing,
    revision: u64,
    stroke_options: &ResolvedPaintStrokeOptions,
    fill_options: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
    path_offsets: &[ResolvedPathOffset],
) -> PreparedGeometry {
    prepare_geometry_with_topology(
        (drawing, revision),
        stroke_options,
        fill_options,
        transform,
        canvas_size,
        drawing.fills.iter().any(|fill| fill.loops.is_empty()),
        path_offsets,
    )
}

fn prepare_geometry_with_topology(
    content: (&PaintDrawing, u64),
    stroke_options: &ResolvedPaintStrokeOptions,
    fill_options: ResolvedPaintFillOptions,
    transform: ResolvedTransform2D,
    canvas_size: Vec2,
    stroke_topology: bool,
    path_offsets: &[ResolvedPathOffset],
) -> PreparedGeometry {
    let (drawing, revision) = content;
    let key = geometry_key_with_topology(
        (drawing, revision),
        stroke_options,
        fill_options,
        transform,
        canvas_size,
        stroke_topology,
        path_offsets,
    );
    let matrix = transform.matrix();
    let centerlines: Vec<_> = drawing
        .strokes
        .par_iter()
        .filter_map(|stroke| {
            prepare_centerline_with_path_offsets(stroke, stroke_options, matrix, true, path_offsets)
        })
        .collect();
    let topology_lines: Vec<_> = drawing
        .strokes
        .par_iter()
        .filter(|stroke| stroke_topology && stroke.points.len() > 1)
        .map(|stroke| {
            let mut points: Vec<_> = stroke
                .points
                .iter()
                .map(|point| matrix.transform_point2(point.position))
                .collect();
            apply_path_offsets(&mut points, path_offsets);
            points
        })
        .collect();
    let topology = if stroke_topology {
        shrimply_paint_topology::build(&topology_lines, canvas_size, fill_options.closure_tolerance)
    } else {
        Topology::empty(canvas_size)
    };
    let fills = drawing
        .fills
        .par_iter()
        .map(|fill| prepare_fill(fill, matrix, &topology))
        .collect();

    PreparedGeometry {
        key,
        centerlines,
        topology,
        fills,
    }
}

/// Prepares a single stored stroke. Pass `completed = false` for the live active stroke.
pub fn prepare_centerline(
    stroke: &PaintStroke,
    options: &ResolvedPaintStrokeOptions,
    transform: Mat3,
    completed: bool,
) -> Option<PreparedCenterline> {
    prepare_centerline_with_path_offsets(stroke, options, transform, completed, &[])
}

pub fn prepare_centerline_with_path_offsets(
    stroke: &PaintStroke,
    options: &ResolvedPaintStrokeOptions,
    transform: Mat3,
    completed: bool,
    path_offsets: &[ResolvedPathOffset],
) -> Option<PreparedCenterline> {
    let finite: Vec<_> = stroke
        .points
        .iter()
        .copied()
        .filter(|point| point.position.is_finite())
        .map(|mut point| {
            point.pressure = point
                .pressure
                .filter(|pressure| pressure.is_finite() && *pressure >= 0.0)
                .map(|pressure| pressure.clamp(0.0, 1.0));
            point
        })
        .collect();
    if finite.is_empty() {
        return None;
    }

    let mut simplified = simplify_points(&finite, options.simplification_tolerance);
    let simulate_pressure = simplified.iter().all(|point| point.pressure.is_none());
    if !simulate_pressure {
        math::interpolate_missing_pressures(&mut simplified);
    }
    let subdivided = subdivide_points(&simplified, options.maximum_subdivision_spacing);
    let input: Vec<_> = subdivided
        .iter()
        .map(|point| InputPoint {
            point: point.position,
            pressure: point.pressure,
        })
        .collect();
    let effective_options = ResolvedPaintStrokeOptions {
        width: stroke.width_scale * options.width,
        ..*options
    };
    let freehand_options = freehand_options(&effective_options, simulate_pressure, completed);
    let mut stroke_points = get_stroke_points(&input, &freehand_options);
    transform_stroke_points(&mut stroke_points, transform);
    offset_stroke_points(&mut stroke_points, path_offsets);

    Some(PreparedCenterline {
        stroke_id: stroke.id,
        width: effective_options.width,
        color_index: stroke.color_index,
        stroke_points,
        simulate_pressure,
        completed,
    })
}

pub fn prepare_outlines(
    centerlines: &[PreparedCenterline],
    key: CenterlineKey,
    options: &ResolvedPaintStrokeOptions,
) -> PreparedOutlines {
    let outlines = centerlines
        .par_iter()
        .map(|centerline| prepare_outline(centerline, options))
        .collect();
    PreparedOutlines {
        key: OutlineKey {
            centerlines: key,
            shape: StrokeShapeKey::from(options),
        },
        outlines,
    }
}

pub fn prepare_outline(
    centerline: &PreparedCenterline,
    options: &ResolvedPaintStrokeOptions,
) -> PressureOutline {
    let options = freehand_options(
        &ResolvedPaintStrokeOptions {
            width: centerline.width,
            ..*options
        },
        centerline.simulate_pressure,
        centerline.completed,
    );
    PressureOutline {
        stroke_id: centerline.stroke_id,
        color_index: centerline.color_index,
        points: get_stroke_outline_points(&centerline.stroke_points, &options)
            .into_iter()
            .collect(),
    }
}

pub fn hit_test_strokes(
    centerlines: &[PreparedCenterline],
    point: Vec2,
    maximum_distance: f32,
) -> Option<StrokeHit> {
    if !point.is_finite() || !maximum_distance.is_finite() || maximum_distance < 0.0 {
        return None;
    }
    centerlines
        .iter()
        .enumerate()
        .filter_map(|(stroke_index, centerline)| {
            let points: Vec<_> = centerline.points().collect();
            let polyline = point_to_polyline(point, &points)?;
            (polyline.distance <= maximum_distance).then_some(StrokeHit {
                stroke_index,
                stroke_id: centerline.stroke_id,
                polyline,
            })
        })
        .min_by(|left, right| left.polyline.distance.total_cmp(&right.polyline.distance))
}

pub fn hit_test_objects_sweep(
    geometry: &PreparedGeometry,
    path: &[Vec2],
    stroke_radii: &[f32],
) -> ObjectSweepHits {
    if path.is_empty()
        || path.len() != stroke_radii.len()
        || path.iter().any(|point| !point.is_finite())
        || stroke_radii
            .iter()
            .any(|radius| !radius.is_finite() || *radius < 0.0)
    {
        return ObjectSweepHits::default();
    }
    let stroke_ids = geometry
        .centerlines
        .iter()
        .filter_map(|centerline| {
            let points: Vec<_> = centerline.points().collect();
            let hit = if path.len() == 1 {
                point_to_polyline_distance(path[0], &points)
                    .is_some_and(|distance| distance <= stroke_radii[0])
            } else if points.len() == 1 {
                path.windows(2).enumerate().any(|(index, sweep)| {
                    point_hits_eraser_sweep(
                        points[0],
                        sweep[0],
                        sweep[1],
                        stroke_radii[index].max(stroke_radii[index + 1]),
                    )
                })
            } else {
                path.windows(2).enumerate().any(|(index, sweep)| {
                    !eraser_sweep_intervals(
                        &points,
                        sweep[0],
                        sweep[1],
                        stroke_radii[index].max(stroke_radii[index + 1]),
                    )
                    .is_empty()
                })
            };
            hit.then_some(centerline.stroke_id)
        })
        .collect();
    let fill_ids = geometry
        .fills
        .iter()
        .filter(|fill| polyline_intersects_even_odd_loops(path, &fill.loops))
        .map(|fill| fill.fill_id)
        .collect();
    ObjectSweepHits {
        stroke_ids,
        fill_ids,
    }
}

pub fn hit_test_samples(
    drawing: &PaintDrawing,
    transform: Mat3,
    point: Vec2,
    maximum_distance: f32,
) -> Option<SampleHit> {
    if !point.is_finite() || !maximum_distance.is_finite() || maximum_distance < 0.0 {
        return None;
    }
    drawing
        .strokes
        .iter()
        .enumerate()
        .flat_map(|(stroke_index, stroke)| {
            stroke
                .points
                .iter()
                .enumerate()
                .filter_map(move |(sample_index, sample)| {
                    let position = transform.transform_point2(sample.position);
                    let distance = point.distance(position);
                    position.is_finite().then_some(SampleHit {
                        stroke_index,
                        stroke_id: stroke.id,
                        sample_index,
                        position,
                        distance,
                    })
                })
        })
        .filter(|hit| hit.distance <= maximum_distance)
        .min_by(|left, right| left.distance.total_cmp(&right.distance))
}

pub fn hit_test_simplified_samples(
    drawing: &PaintDrawing,
    transform: Mat3,
    point: Vec2,
    maximum_distance: f32,
    simplification_tolerance: f32,
) -> Option<SampleHit> {
    if !point.is_finite() || !maximum_distance.is_finite() || maximum_distance < 0.0 {
        return None;
    }
    drawing
        .strokes
        .iter()
        .enumerate()
        .flat_map(|(stroke_index, stroke)| {
            simplified_point_indices(&stroke.points, simplification_tolerance)
                .into_iter()
                .filter_map(move |sample_index| {
                    let position = transform.transform_point2(stroke.points[sample_index].position);
                    let distance = point.distance(position);
                    position.is_finite().then_some(SampleHit {
                        stroke_index,
                        stroke_id: stroke.id,
                        sample_index,
                        position,
                        distance,
                    })
                })
        })
        .filter(|hit| hit.distance <= maximum_distance)
        .min_by(|left, right| left.distance.total_cmp(&right.distance))
}

pub fn transform_fill_seed(fill: &PaintFill, transform: Mat3) -> Vec2 {
    transform.transform_point2(fill.seed)
}

impl From<ResolvedTransform2D> for TransformKey {
    fn from(transform: ResolvedTransform2D) -> Self {
        Self {
            position: UVec2::from_array(transform.position.to_array().map(f32::to_bits)),
            anchor: UVec2::from_array(transform.anchor.to_array().map(f32::to_bits)),
            scale: UVec2::from_array(transform.scale.to_array().map(f32::to_bits)),
            shear: UVec2::from_array(transform.shear.to_array().map(f32::to_bits)),
            rotation_degrees: transform.rotation_degrees.to_bits(),
        }
    }
}

impl From<ResolvedPathOffset> for PathOffsetKey {
    fn from(options: ResolvedPathOffset) -> Self {
        Self {
            amplitude: options.amplitude.to_bits(),
            spacing: options.spacing.to_bits(),
            seed: options.seed.to_bits(),
            evolution: options.evolution.to_bits(),
        }
    }
}

impl From<&ResolvedPaintStrokeOptions> for StrokeShapeKey {
    fn from(options: &ResolvedPaintStrokeOptions) -> Self {
        Self {
            width: options.width.to_bits(),
            thinning: options.thinning.to_bits(),
            smoothing: options.smoothing.to_bits(),
            start: StrokeEndKey::from(options.start),
            end: StrokeEndKey::from(options.end),
        }
    }
}

impl From<ResolvedPaintStrokeEndOptions> for StrokeEndKey {
    fn from(options: ResolvedPaintStrokeEndOptions) -> Self {
        Self {
            cap: options.cap,
            taper: match options.taper {
                PaintTaper::None => TaperKey::None,
                PaintTaper::Full => TaperKey::Full,
                PaintTaper::Distance => TaperKey::Distance(options.taper_distance.to_bits()),
            },
        }
    }
}

fn prepare_fill(fill: &PaintFill, transform: Mat3, topology: &Topology) -> PreparedFill {
    let seed = transform_fill_seed(fill, transform);
    let loops = if fill.loops.is_empty() {
        topology
            .face_at(seed)
            .map(|face| face.loops().to_vec())
            .unwrap_or_default()
    } else {
        fill.loops
            .iter()
            .map(|boundary| {
                boundary
                    .iter()
                    .map(|point| transform.transform_point2(*point))
                    .collect()
            })
            .collect()
    };
    PreparedFill {
        fill_id: fill.id,
        color_index: fill.color_index,
        seed,
        loops,
    }
}

fn freehand_options(
    options: &ResolvedPaintStrokeOptions,
    simulate_pressure: bool,
    completed: bool,
) -> StrokeOptions {
    let streamline = options.streamline.clamp(0.0, 1.0);
    StrokeOptions {
        size: options.width,
        thinning: options.thinning,
        smoothing: options.smoothing,
        streamline: streamline * streamline * streamline,
        simulate_pressure,
        start: freehand_end_options(options.start, true),
        end: freehand_end_options(options.end, false),
        last: completed,
        ..StrokeOptions::default()
    }
}

fn freehand_end_options(options: ResolvedPaintStrokeEndOptions, start: bool) -> StrokeEndOptions {
    let mut result = if start {
        StrokeEndOptions::start()
    } else {
        StrokeEndOptions::end()
    };
    result.cap = options.cap;
    result.taper = match options.taper {
        PaintTaper::None => Taper::None,
        PaintTaper::Full => Taper::Full,
        PaintTaper::Distance => Taper::Distance(options.taper_distance),
    };
    result
}

fn transform_stroke_points(points: &mut [StrokePoint], transform: Mat3) {
    for point in &mut *points {
        point.point = transform.transform_point2(point.point);
    }
    let mut running_length = 0.0;
    for index in 1..points.len() {
        let current = points[index].point;
        let previous = points[index - 1].point;
        let distance = current.distance(previous);
        running_length += distance;
        points[index].distance = distance;
        points[index].running_length = running_length;
        points[index].vector = (previous - current).normalize_or_zero();
    }
    if points.len() > 1 {
        points[0].vector = points[1].vector;
    } else if let Some(point) = points.first_mut() {
        point.vector = Vec2::ZERO;
    }
}

fn offset_stroke_points(points: &mut [StrokePoint], path_offsets: &[ResolvedPathOffset]) {
    if path_offsets.is_empty() {
        return;
    }
    let mut positions: Vec<_> = points.iter().map(|point| point.point).collect();
    apply_path_offsets(&mut positions, path_offsets);
    for (point, position) in points.iter_mut().zip(positions) {
        point.point = position;
    }
    transform_stroke_points(points, Mat3::IDENTITY);
}
