use glam::Vec2;
use shrimply_paint_model::PaintPoint;
use shrimply_perfect_freehand::StrokePoint;

const CAPSULE_BOUNDARY_ITERATIONS: usize = 32;
const DISTANCE_EPSILON: f32 = 1.0e-6;
const PATH_OFFSET_MINIMUM_SPACING: f32 = 0.1;
const HASH_MIX_A: u64 = 0xbf58_476d_1ce4_e5b9;
const HASH_MIX_B: u64 = 0x94d0_49bb_1331_11eb;

pub fn partial_stroke_points(points: &[StrokePoint], progress: f32) -> Vec<StrokePoint> {
    if points.is_empty() || progress <= 0.0 {
        return Vec::new();
    }
    if progress >= 1.0 || points.len() == 1 {
        return points.to_vec();
    }
    let total_length = points
        .last()
        .expect("stroke points are not empty")
        .running_length;
    if total_length <= f32::EPSILON {
        return points.to_vec();
    }
    let target_length = total_length * progress;
    let end = points.partition_point(|point| point.running_length < target_length);
    if end == 0 || end == points.len() {
        return points[..end].to_vec();
    }
    let previous = points[end - 1];
    let next = points[end];
    let segment_length = next.running_length - previous.running_length;
    let amount = if segment_length > f32::EPSILON {
        (target_length - previous.running_length) / segment_length
    } else {
        0.0
    };
    let previous_position = previous.point;
    let position = previous_position.lerp(next.point, amount);
    let distance = position.distance(previous_position);
    let mut partial = points[..end].to_vec();
    partial.push(StrokePoint {
        point: position,
        pressure: previous.pressure + (next.pressure - previous.pressure) * amount,
        distance,
        vector: (previous_position - position).normalize_or_zero(),
        running_length: target_length,
    });
    partial
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPathOffset {
    pub amplitude: f32,
    pub spacing: f32,
    pub seed: f32,
    pub evolution: f32,
}

pub fn pressure_diameter_scale(thinning: f32, pressure: Option<f32>) -> f32 {
    pressure.map_or(1.0, |pressure| {
        (1.0 - thinning + 2.0 * thinning * pressure.clamp(0.0, 1.0)).max(0.0)
    })
}

pub fn point_in_even_odd_loops(point: Vec2, loops: &[Vec<Vec2>]) -> bool {
    point.is_finite()
        && loops
            .iter()
            .filter(|boundary| point_in_polygon(point, boundary))
            .count()
            % 2
            == 1
}

pub fn polyline_intersects_even_odd_loops(points: &[Vec2], loops: &[Vec<Vec2>]) -> bool {
    points
        .iter()
        .any(|point| point_in_even_odd_loops(*point, loops))
        || points.windows(2).any(|path| {
            loops.iter().any(|boundary| {
                boundary
                    .iter()
                    .zip(boundary.iter().cycle().skip(1))
                    .take(boundary.len())
                    .any(|(&start, &end)| segments_intersect(path[0], path[1], start, end))
            })
        })
}

fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 || polygon.iter().any(|point| !point.is_finite()) {
        return false;
    }
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        if (current.y > point.y) != (previous.y > point.y) {
            let crossing = (previous.x - current.x) * (point.y - current.y)
                / (previous.y - current.y)
                + current.x;
            if point.x < crossing {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

/// Applies deterministic smooth normal displacement to a prepared polyline.
pub fn apply_path_offset(points: &mut [Vec2], options: ResolvedPathOffset) {
    let amplitude = options.amplitude.max(0.0);
    if points.len() < 2
        || points.iter().any(|point| !point.is_finite())
        || !amplitude.is_finite()
        || amplitude <= f32::EPSILON
        || !options.spacing.is_finite()
        || !options.seed.is_finite()
        || !options.evolution.is_finite()
    {
        return;
    }
    let spacing = options.spacing.max(PATH_OFFSET_MINIMUM_SPACING);
    let source = points.to_vec();
    let mut distance = 0.0;
    for index in 0..points.len() {
        if index > 0 {
            distance += source[index - 1].distance(source[index]);
        }
        let previous = source[index.saturating_sub(1)];
        let next = source[(index + 1).min(source.len() - 1)];
        let tangent = (next - previous).normalize_or_zero();
        if tangent == Vec2::ZERO {
            continue;
        }
        let coordinate = distance / spacing + options.evolution;
        let lattice = coordinate.floor();
        let amount = coordinate - lattice;
        let start = path_offset_noise(lattice as i64, options.seed);
        let end = path_offset_noise(lattice as i64 + 1, options.seed);
        let smooth = amount * amount * (3.0 - 2.0 * amount);
        let normal = Vec2::new(-tangent.y, tangent.x);
        points[index] += normal * (start + (end - start) * smooth) * amplitude;
    }
}

pub fn apply_path_offsets(points: &mut [Vec2], offsets: &[ResolvedPathOffset]) {
    for &offset in offsets {
        apply_path_offset(points, offset);
    }
}

fn path_offset_noise(lattice: i64, seed: f32) -> f32 {
    let mut value = (lattice as u64) ^ u64::from(seed.to_bits());
    value = (value ^ (value >> 30)).wrapping_mul(HASH_MIX_A);
    value = (value ^ (value >> 27)).wrapping_mul(HASH_MIX_B);
    value ^= value >> 31;
    (value as u32) as f32 / u32::MAX as f32 * 2.0 - 1.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolylineHit {
    pub segment_index: Option<usize>,
    pub segment_t: f32,
    pub point: Vec2,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EraserInterval {
    pub segment_index: usize,
    pub start: f32,
    pub end: f32,
}

/// Simplifies raw samples while retaining the original pressure-bearing points.
pub fn simplify_points(points: &[PaintPoint], tolerance: f32) -> Vec<PaintPoint> {
    simplified_point_indices(points, tolerance)
        .into_iter()
        .map(|index| points[index])
        .collect()
}

pub(crate) fn interpolate_missing_pressures(points: &mut [PaintPoint]) {
    let Some(first) = points.iter().position(|point| point.pressure.is_some()) else {
        return;
    };
    let first_pressure = points[first].pressure;
    for point in &mut points[..first] {
        point.pressure = first_pressure;
    }

    let mut left = first;
    while let Some(offset) = points[left + 1..]
        .iter()
        .position(|point| point.pressure.is_some())
    {
        let right = left + offset + 1;
        let start_pressure = points[left].pressure.expect("pressure endpoint is present");
        let end_pressure = points[right]
            .pressure
            .expect("pressure endpoint is present");
        let total_distance: f32 = points[left..=right]
            .windows(2)
            .map(|pair| pair[0].position.distance(pair[1].position))
            .sum();
        let mut distance = 0.0;
        for index in left + 1..right {
            distance += points[index - 1].position.distance(points[index].position);
            let amount = if total_distance > 0.0 {
                distance / total_distance
            } else {
                (index - left) as f32 / (right - left) as f32
            };
            points[index].pressure =
                Some(start_pressure + (end_pressure - start_pressure) * amount);
        }
        left = right;
    }

    let last_pressure = points[left].pressure;
    for point in &mut points[left + 1..] {
        point.pressure = last_pressure;
    }
}

/// Returns the raw sample indices retained by Ramer-Douglas-Peucker simplification.
///
/// Non-finite samples are omitted. A non-positive or non-finite tolerance retains
/// every finite sample.
pub fn simplified_point_indices(points: &[PaintPoint], tolerance: f32) -> Vec<usize> {
    let finite: Vec<_> = points
        .iter()
        .enumerate()
        .filter_map(|(index, point)| point.position.is_finite().then_some(index))
        .collect();
    if finite.len() < 3 || !tolerance.is_finite() || tolerance <= 0.0 {
        return finite;
    }

    let mut retained = vec![false; finite.len()];
    retained[0] = true;
    retained[finite.len() - 1] = true;
    let mut ranges = vec![(0, finite.len() - 1)];
    let tolerance_squared = tolerance * tolerance;

    while let Some((start, end)) = ranges.pop() {
        let mut farthest = None;
        let mut farthest_distance = tolerance_squared;
        for index in start + 1..end {
            let distance = point_to_segment_distance_squared(
                points[finite[index]].position,
                points[finite[start]].position,
                points[finite[end]].position,
            );
            if distance > farthest_distance {
                farthest = Some(index);
                farthest_distance = distance;
            }
        }
        if let Some(index) = farthest {
            retained[index] = true;
            ranges.push((start, index));
            ranges.push((index, end));
        }
    }

    finite
        .into_iter()
        .zip(retained)
        .filter_map(|(index, retained)| retained.then_some(index))
        .collect()
}

/// Inserts linearly interpolated samples so no segment is longer than `spacing`.
pub fn subdivide_points(points: &[PaintPoint], spacing: f32) -> Vec<PaintPoint> {
    if points.len() < 2 || !spacing.is_finite() || spacing <= 0.0 {
        return points.to_vec();
    }

    let mut subdivided = Vec::with_capacity(points.len());
    subdivided.push(points[0]);
    for pair in points.windows(2) {
        let distance = pair[0].position.distance(pair[1].position);
        let segments = (distance / spacing).ceil().max(1.0) as usize;
        for index in 1..=segments {
            let amount = index as f32 / segments as f32;
            subdivided.push(PaintPoint {
                position: pair[0].position.lerp(pair[1].position, amount),
                pressure: interpolate_pressure(pair[0].pressure, pair[1].pressure, amount),
            });
        }
    }
    subdivided
}

pub fn point_to_polyline(point: Vec2, polyline: &[Vec2]) -> Option<PolylineHit> {
    if !point.is_finite() {
        return None;
    }
    match polyline {
        [] => None,
        [only] => only.is_finite().then(|| PolylineHit {
            segment_index: None,
            segment_t: 0.0,
            point: *only,
            distance: point.distance(*only),
        }),
        _ => polyline
            .windows(2)
            .enumerate()
            .filter(|(_, pair)| pair[0].is_finite() && pair[1].is_finite())
            .map(|(segment_index, pair)| {
                let (segment_t, nearest) = closest_point_on_segment(point, pair[0], pair[1]);
                PolylineHit {
                    segment_index: Some(segment_index),
                    segment_t,
                    point: nearest,
                    distance: point.distance(nearest),
                }
            })
            .min_by(|left, right| left.distance.total_cmp(&right.distance)),
    }
}

pub fn point_to_polyline_distance(point: Vec2, polyline: &[Vec2]) -> Option<f32> {
    point_to_polyline(point, polyline).map(|hit| hit.distance)
}

pub fn point_hits_eraser_sweep(
    point: Vec2,
    sweep_start: Vec2,
    sweep_end: Vec2,
    radius: f32,
) -> bool {
    radius.is_finite()
        && radius >= 0.0
        && point_to_segment_distance_squared(point, sweep_start, sweep_end) <= radius * radius
}

/// Finds the portions of each polyline segment covered by a swept circular eraser.
///
/// `start` and `end` are local parameters in the segment identified by
/// `segment_index`. The returned intervals are ordered and inclusive.
pub fn eraser_sweep_intervals(
    polyline: &[Vec2],
    sweep_start: Vec2,
    sweep_end: Vec2,
    radius: f32,
) -> Vec<EraserInterval> {
    if radius < 0.0 || !radius.is_finite() || !sweep_start.is_finite() || !sweep_end.is_finite() {
        return Vec::new();
    }

    polyline
        .windows(2)
        .enumerate()
        .filter_map(|(segment_index, pair)| {
            capsule_interval(pair[0], pair[1], sweep_start, sweep_end, radius).map(
                |(start, end)| EraserInterval {
                    segment_index,
                    start,
                    end,
                },
            )
        })
        .collect()
}

pub(crate) fn point_to_segment_distance_squared(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let (_, closest) = closest_point_on_segment(point, start, end);
    point.distance_squared(closest)
}

fn interpolate_pressure(start: Option<f32>, end: Option<f32>, amount: f32) -> Option<f32> {
    match (start, end) {
        (Some(start), Some(end)) => Some(start + (end - start) * amount),
        _ => None,
    }
}

fn closest_point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> (f32, Vec2) {
    let segment = end - start;
    let length_squared = segment.length_squared();
    let amount = if length_squared <= DISTANCE_EPSILON * DISTANCE_EPSILON {
        0.0
    } else {
        ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0)
    };
    (amount, start + segment * amount)
}

fn segments_intersect(start: Vec2, end: Vec2, other_start: Vec2, other_end: Vec2) -> bool {
    if !start.is_finite() || !end.is_finite() || !other_start.is_finite() || !other_end.is_finite()
    {
        return false;
    }
    let direction = (end - start).as_dvec2();
    let other_direction = (other_end - other_start).as_dvec2();
    let offset = (other_start - start).as_dvec2();
    let denominator = direction.perp_dot(other_direction);
    let parallel_epsilon = f64::EPSILON.sqrt() * direction.length() * other_direction.length();
    if denominator.abs() <= parallel_epsilon {
        if offset.perp_dot(direction).abs() > parallel_epsilon {
            return false;
        }
        let length_squared = direction.length_squared();
        if length_squared == 0.0 {
            return start == other_start || start == other_end;
        }
        let first = offset.dot(direction) / length_squared;
        let second = (offset + other_direction).dot(direction) / length_squared;
        return first.max(second) >= 0.0 && first.min(second) <= 1.0;
    }
    let amount = offset.perp_dot(other_direction) / denominator;
    let other_amount = offset.perp_dot(direction) / denominator;
    (0.0..=1.0).contains(&amount) && (0.0..=1.0).contains(&other_amount)
}

fn capsule_interval(
    start: Vec2,
    end: Vec2,
    sweep_start: Vec2,
    sweep_end: Vec2,
    radius: f32,
) -> Option<(f32, f32)> {
    if !start.is_finite() || !end.is_finite() {
        return None;
    }

    let radius_squared = radius * radius;
    let minimum = closest_parameter_between_segments(start, end, sweep_start, sweep_end);
    let inside = |amount: f32| {
        point_to_segment_distance_squared(start.lerp(end, amount), sweep_start, sweep_end)
            <= radius_squared
    };
    if !inside(minimum) {
        return None;
    }

    let interval_start = if inside(0.0) {
        0.0
    } else {
        boundary(0.0, minimum, &inside, false)
    };
    let interval_end = if inside(1.0) {
        1.0
    } else {
        boundary(minimum, 1.0, &inside, true)
    };
    Some((interval_start, interval_end))
}

fn boundary(first: f32, second: f32, inside: &impl Fn(f32) -> bool, first_is_inside: bool) -> f32 {
    let (mut inside_amount, mut outside_amount) = if first_is_inside {
        (first, second)
    } else {
        (second, first)
    };
    for _ in 0..CAPSULE_BOUNDARY_ITERATIONS {
        let middle = (inside_amount + outside_amount) * 0.5;
        if inside(middle) {
            inside_amount = middle;
        } else {
            outside_amount = middle;
        }
    }
    (inside_amount + outside_amount) * 0.5
}

fn closest_parameter_between_segments(
    start: Vec2,
    end: Vec2,
    other_start: Vec2,
    other_end: Vec2,
) -> f32 {
    let direction = end - start;
    let other_direction = other_end - other_start;
    let offset = start - other_start;
    let length_squared = direction.length_squared();
    let other_length_squared = other_direction.length_squared();

    if length_squared <= DISTANCE_EPSILON * DISTANCE_EPSILON {
        return 0.0;
    }
    if other_length_squared <= DISTANCE_EPSILON * DISTANCE_EPSILON {
        return ((other_start - start).dot(direction) / length_squared).clamp(0.0, 1.0);
    }

    let product = direction.dot(other_direction);
    let first_offset = direction.dot(offset);
    let second_offset = other_direction.dot(offset);
    let denominator = length_squared * other_length_squared - product * product;
    let mut amount = if denominator.abs() <= DISTANCE_EPSILON {
        0.0
    } else {
        ((product * second_offset - first_offset * other_length_squared) / denominator)
            .clamp(0.0, 1.0)
    };
    let other_amount = (product * amount + second_offset) / other_length_squared;

    if other_amount < 0.0 {
        amount = (-first_offset / length_squared).clamp(0.0, 1.0);
    } else if other_amount > 1.0 {
        amount = ((product - first_offset) / length_squared).clamp(0.0, 1.0);
    }
    amount
}
