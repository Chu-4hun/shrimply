use hashbrown::HashSet;

use glam::Vec2;
use rayon::prelude::*;

use super::{
    assignment::minimum_cost_pairs,
    deformation::angle_delta,
    geometry::{
        bounds, centroid, closed_length, loop_area, mean_abs_turn, polyline_length, ratio_distance,
        total_area,
    },
};
use crate::{PaintFill, PaintStroke};

const POSITION_WEIGHT: f32 = 3.0;
const ENDPOINT_WEIGHT: f32 = 2.0;
const LENGTH_WEIGHT: f32 = 1.0;
const EXTENT_WEIGHT: f32 = 1.0;
const CURVATURE_WEIGHT: f32 = 0.75;
const TOPOLOGY_WEIGHT: f32 = 2.5;
const MATCHED_TOPOLOGY_WEIGHT: f32 = 3.0;
const COLOR_MISMATCH_COST: f32 = 0.75;
const UNMATCHED_COST: f32 = 5.0;
const TOPOLOGY_NEIGHBORS: usize = 4;
const HIGH_CONFIDENCE_COST: f32 = 2.5;
const HIGH_CONFIDENCE_MARGIN: f32 = 0.4;

#[derive(Clone)]
struct StrokeDescriptor {
    centroid: Vec2,
    first: Vec2,
    last: Vec2,
    length: f32,
    extent: Vec2,
    curvature: f32,
    topology: [f32; TOPOLOGY_NEIGHBORS],
}

pub(super) fn match_strokes(
    from: &[PaintStroke],
    to: &[PaintStroke],
    scale: f32,
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut used_from = HashSet::new();
    let mut used_to = HashSet::new();
    for (from_index, from_stroke) in from.iter().enumerate() {
        if let Some((to_index, _)) = to.iter().enumerate().find(|(to_index, to_stroke)| {
            !used_to.contains(to_index)
                && from_stroke.correspondence_id == to_stroke.correspondence_id
        }) {
            pairs.push((from_index, to_index));
            used_from.insert(from_index);
            used_to.insert(to_index);
        }
    }

    let from_descriptors = stroke_descriptors(from, scale);
    let to_descriptors = stroke_descriptors(to, scale);
    let mut remaining_from: Vec<_> = (0..from.len())
        .filter(|index| !used_from.contains(index))
        .collect();
    let mut remaining_to: Vec<_> = (0..to.len())
        .filter(|index| !used_to.contains(index))
        .collect();
    let initial_costs = stroke_costs(
        from,
        to,
        &from_descriptors,
        &to_descriptors,
        &remaining_from,
        &remaining_to,
        &pairs,
    );
    for (left, right) in high_confidence_pairs(&initial_costs) {
        let from_index = remaining_from[left];
        let to_index = remaining_to[right];
        pairs.push((from_index, to_index));
        used_from.insert(from_index);
        used_to.insert(to_index);
    }
    remaining_from.retain(|index| !used_from.contains(index));
    remaining_to.retain(|index| !used_to.contains(index));
    if !remaining_from.is_empty() && !remaining_to.is_empty() {
        let costs = stroke_costs(
            from,
            to,
            &from_descriptors,
            &to_descriptors,
            &remaining_from,
            &remaining_to,
            &pairs,
        );
        let maximum_cost = costs
            .iter()
            .flatten()
            .copied()
            .filter(|cost| cost.is_finite())
            .fold(0.0f32, f32::max);
        for (left, right) in minimum_cost_pairs(&costs, maximum_cost + 1.0) {
            pairs.push((remaining_from[left], remaining_to[right]));
        }
    }
    pairs.sort_unstable();
    pairs
}

fn stroke_costs(
    from: &[PaintStroke],
    to: &[PaintStroke],
    from_descriptors: &[StrokeDescriptor],
    to_descriptors: &[StrokeDescriptor],
    remaining_from: &[usize],
    remaining_to: &[usize],
    anchors: &[(usize, usize)],
) -> Vec<Vec<f32>> {
    remaining_from
        .par_iter()
        .map(|from_index| {
            remaining_to
                .iter()
                .map(|to_index| {
                    stroke_cost(
                        &from_descriptors[*from_index],
                        &to_descriptors[*to_index],
                        from[*from_index].color_index != to[*to_index].color_index,
                    ) + matched_topology_cost(
                        *from_index,
                        *to_index,
                        from_descriptors,
                        to_descriptors,
                        anchors,
                    )
                })
                .collect()
        })
        .collect()
}

fn high_confidence_pairs(costs: &[Vec<f32>]) -> Vec<(usize, usize)> {
    if costs.is_empty() || costs[0].is_empty() {
        return Vec::new();
    }
    let row_best: Vec<_> = costs
        .iter()
        .map(|row| best_with_margin(row.iter().copied()))
        .collect();
    let column_best: Vec<_> = (0..costs[0].len())
        .map(|column| best_with_margin(costs.iter().map(|row| row[column])))
        .collect();
    row_best
        .iter()
        .enumerate()
        .filter_map(|(row, &(column, cost, margin))| {
            let (matched_row, column_cost, column_margin) = column_best[column];
            (matched_row == row
                && cost <= HIGH_CONFIDENCE_COST
                && column_cost <= HIGH_CONFIDENCE_COST
                && margin >= HIGH_CONFIDENCE_MARGIN
                && column_margin >= HIGH_CONFIDENCE_MARGIN)
                .then_some((row, column))
        })
        .collect()
}

fn best_with_margin(costs: impl Iterator<Item = f32>) -> (usize, f32, f32) {
    let mut ranked: Vec<_> = costs.enumerate().collect();
    ranked.sort_by(|left, right| left.1.total_cmp(&right.1));
    let (index, best) = ranked[0];
    let margin = ranked
        .get(1)
        .map_or(f32::INFINITY, |(_, second)| *second - best);
    (index, best, margin)
}

fn matched_topology_cost(
    from_index: usize,
    to_index: usize,
    from: &[StrokeDescriptor],
    to: &[StrokeDescriptor],
    anchors: &[(usize, usize)],
) -> f32 {
    if anchors.is_empty() {
        return 0.0;
    }
    let mut distance_cost = 0.0;
    let mut rotations = Vec::new();
    for (from_anchor, to_anchor) in anchors {
        let from_delta = from[from_index].centroid - from[*from_anchor].centroid;
        let to_delta = to[to_index].centroid - to[*to_anchor].centroid;
        distance_cost += ratio_distance(from_delta.length(), to_delta.length());
        if from_delta.length_squared() > f32::EPSILON && to_delta.length_squared() > f32::EPSILON {
            rotations.push(angle_delta(from_delta.to_angle(), to_delta.to_angle()));
        }
    }
    distance_cost /= anchors.len() as f32;
    let rotation_cost = if rotations.len() < 2 {
        0.0
    } else {
        let mean = Vec2::new(
            rotations.iter().map(|angle| angle.cos()).sum(),
            rotations.iter().map(|angle| angle.sin()).sum(),
        )
        .to_angle();
        rotations
            .iter()
            .map(|rotation| angle_delta(mean, *rotation).abs() / std::f32::consts::PI)
            .sum::<f32>()
            / rotations.len() as f32
    };
    MATCHED_TOPOLOGY_WEIGHT * (distance_cost + rotation_cost)
}

fn stroke_descriptors(strokes: &[PaintStroke], scale: f32) -> Vec<StrokeDescriptor> {
    let drawing_center = centroid(
        &strokes
            .iter()
            .flat_map(|stroke| stroke.points.iter().map(|point| point.position))
            .collect::<Vec<_>>(),
    );
    strokes
        .par_iter()
        .map(|stroke| {
            let positions: Vec<_> = stroke.points.iter().map(|point| point.position).collect();
            let centroid = centroid(&positions);
            let first = positions.first().copied().unwrap_or(centroid);
            let last = positions.last().copied().unwrap_or(centroid);
            let length = polyline_length(&positions);
            let (minimum, maximum) = bounds(&positions).unwrap_or((centroid, centroid));
            let mut topology = [1.0; TOPOLOGY_NEIGHBORS];
            let mut distances: Vec<_> = strokes
                .iter()
                .filter(|other| other.id != stroke.id)
                .map(|other| endpoint_polyline_distance(stroke, other) / scale)
                .filter(|distance| distance.is_finite())
                .collect();
            distances.sort_by(f32::total_cmp);
            for (target, value) in topology.iter_mut().zip(distances) {
                *target = value;
            }
            StrokeDescriptor {
                centroid: (centroid - drawing_center) / scale,
                first: (first - drawing_center) / scale,
                last: (last - drawing_center) / scale,
                length: length / scale,
                extent: (maximum - minimum) / scale,
                curvature: mean_abs_turn(&positions),
                topology,
            }
        })
        .collect()
}

fn stroke_cost(from: &StrokeDescriptor, to: &StrokeDescriptor, color_mismatch: bool) -> f32 {
    let direct = (from.first.length() - to.first.length()).abs()
        + (from.last.length() - to.last.length()).abs();
    let reverse = (from.first.length() - to.last.length()).abs()
        + (from.last.length() - to.first.length()).abs();
    let from_aspect = from.extent.min_element() / from.extent.max_element().max(f32::EPSILON);
    let to_aspect = to.extent.min_element() / to.extent.max_element().max(f32::EPSILON);
    POSITION_WEIGHT * (from.centroid.length() - to.centroid.length()).abs()
        + ENDPOINT_WEIGHT * direct.min(reverse) * 0.5
        + LENGTH_WEIGHT * ratio_distance(from.length, to.length)
        + EXTENT_WEIGHT * ratio_distance(from.extent.length(), to.extent.length())
        + EXTENT_WEIGHT * ratio_distance(from_aspect, to_aspect)
        + CURVATURE_WEIGHT * (from.curvature - to.curvature).abs()
        + TOPOLOGY_WEIGHT
            * from
                .topology
                .iter()
                .zip(to.topology)
                .map(|(left, right)| (left - right).abs())
                .sum::<f32>()
            / TOPOLOGY_NEIGHBORS as f32
        + if color_mismatch {
            COLOR_MISMATCH_COST
        } else {
            0.0
        }
}

pub(super) fn match_fills(from: &[PaintFill], to: &[PaintFill], scale: f32) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut used_to = HashSet::new();
    for (from_index, from_fill) in from.iter().enumerate() {
        if let Some((to_index, _)) = to.iter().enumerate().find(|(to_index, to_fill)| {
            !used_to.contains(to_index) && from_fill.correspondence_id == to_fill.correspondence_id
        }) {
            pairs.push((from_index, to_index));
            used_to.insert(to_index);
        }
    }
    let used_from: HashSet<_> = pairs.iter().map(|(index, _)| *index).collect();
    let remaining_from: Vec<_> = (0..from.len())
        .filter(|index| !used_from.contains(index))
        .collect();
    let remaining_to: Vec<_> = (0..to.len())
        .filter(|index| !used_to.contains(index))
        .collect();
    let costs: Vec<Vec<f32>> = remaining_from
        .par_iter()
        .map(|from_index| {
            remaining_to
                .iter()
                .map(|to_index| fill_cost(&from[*from_index], &to[*to_index], scale))
                .collect()
        })
        .collect();
    for (left, right) in minimum_cost_pairs(&costs, UNMATCHED_COST) {
        pairs.push((remaining_from[left], remaining_to[right]));
    }
    pairs.sort_unstable();
    pairs
}

fn fill_cost(from: &PaintFill, to: &PaintFill, scale: f32) -> f32 {
    let from_points: Vec<_> = from.loops.iter().flatten().copied().collect();
    let to_points: Vec<_> = to.loops.iter().flatten().copied().collect();
    let position = centroid(&from_points).distance(centroid(&to_points)) / scale;
    let area = ratio_distance(total_area(&from.loops), total_area(&to.loops));
    POSITION_WEIGHT * position
        + LENGTH_WEIGHT * area
        + from.loops.len().abs_diff(to.loops.len()) as f32
        + if from.color_index == to.color_index {
            0.0
        } else {
            COLOR_MISMATCH_COST
        }
}

pub(super) fn match_loops(from: &[Vec<Vec2>], to: &[Vec<Vec2>]) -> Vec<(usize, usize)> {
    let points: Vec<_> = from.iter().chain(to).flatten().copied().collect();
    let scale = bounds(&points)
        .map(|(minimum, maximum)| minimum.distance(maximum))
        .unwrap_or(1.0)
        .max(1.0);
    let costs: Vec<Vec<f32>> = from
        .par_iter()
        .map(|left| {
            to.iter()
                .map(|right| {
                    POSITION_WEIGHT * centroid(left).distance(centroid(right)) / scale
                        + LENGTH_WEIGHT * ratio_distance(loop_area(left), loop_area(right))
                        + LENGTH_WEIGHT * ratio_distance(closed_length(left), closed_length(right))
                })
                .collect()
        })
        .collect();
    let mut pairs = minimum_cost_pairs(&costs, UNMATCHED_COST);
    pairs.sort_unstable();
    pairs
}

fn endpoint_polyline_distance(stroke: &PaintStroke, other: &PaintStroke) -> f32 {
    let Some(first) = stroke.points.first() else {
        return f32::INFINITY;
    };
    let last = stroke.points.last().unwrap_or(first);
    point_polyline_distance(first.position, other)
        .min(point_polyline_distance(last.position, other))
}

fn point_polyline_distance(point: Vec2, stroke: &PaintStroke) -> f32 {
    if stroke.points.len() < 2 {
        return stroke
            .points
            .first()
            .map_or(f32::INFINITY, |sample| point.distance(sample.position));
    }
    stroke
        .points
        .windows(2)
        .map(|pair| {
            let start = pair[0].position;
            let delta = pair[1].position - start;
            let length_squared = delta.length_squared();
            if length_squared <= f32::EPSILON {
                point.distance(start)
            } else {
                let progress = (point - start).dot(delta) / length_squared;
                point.distance(start + delta * progress.clamp(0.0, 1.0))
            }
        })
        .fold(f32::INFINITY, f32::min)
}
