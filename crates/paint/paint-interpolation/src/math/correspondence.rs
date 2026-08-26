use glam::{Mat2, Vec2};

use super::{
    cumulative_lengths, geometry::centroid, geometry::polyline_length, resample_stroke,
    sample_stroke,
};
use crate::{PaintPoint, PaintStroke};

const MAX_CURVE_MATCH_SAMPLES: usize = 192;
const POSITION_WEIGHT: f32 = 3.0;
const TANGENT_WEIGHT: f32 = 0.75;
const PARAMETER_WEIGHT: f32 = 0.2;
const WARP_PENALTY: f32 = 0.025;
const DTW_WARP_WEIGHT: f32 = 0.5;
const CLOSED_STROKE_ENDPOINT_RATIO: f32 = 0.05;

pub(super) fn correspond_stroke_points(
    from: &PaintStroke,
    to: &PaintStroke,
) -> (Vec<PaintPoint>, Vec<PaintPoint>) {
    let count = from.points.len().max(to.points.len()).max(1);
    let from_points = resample_stroke(&from.points, count);
    let mut to_points = resample_stroke(&to.points, count);
    if count >= 4 && is_closed(&from_points) && is_closed(&to_points) {
        to_points = align_closed_curve(&from_points, &to_points);
    } else if curve_alignment_cost(&from_points, to_points.iter())
        > curve_alignment_cost(&from_points, to_points.iter().rev())
    {
        to_points.reverse();
    }
    if count < 3 {
        return (from_points, to_points);
    }

    let match_count = count.min(MAX_CURVE_MATCH_SAMPLES);
    let from_match = resample_stroke(&from_points, match_count);
    let to_match = resample_stroke(&to_points, match_count);
    let warp = curve_warp(&from_match, &to_match);
    let distances = cumulative_lengths(
        &to_points
            .iter()
            .map(|point| point.position)
            .collect::<Vec<_>>(),
        false,
    );
    let total = *distances.last().unwrap_or(&0.0);
    let warped = (0..count)
        .map(|index| {
            let progress = index as f32 / count.saturating_sub(1).max(1) as f32;
            let position = progress * match_count.saturating_sub(1) as f32;
            let left = position.floor() as usize;
            let right = (left + 1).min(match_count - 1);
            let amount = position - left as f32;
            let target_progress = warp[left] + (warp[right] - warp[left]) * amount;
            sample_stroke(&to_points, &distances, total * target_progress)
        })
        .collect();
    (from_points, warped)
}

fn is_closed(points: &[PaintPoint]) -> bool {
    let positions: Vec<_> = points.iter().map(|point| point.position).collect();
    let Some((first, last)) = positions.first().zip(positions.last()) else {
        return false;
    };
    first.distance(*last) <= polyline_length(&positions) * CLOSED_STROKE_ENDPOINT_RATIO
}

fn align_closed_curve(from: &[PaintPoint], to: &[PaintPoint]) -> Vec<PaintPoint> {
    let unique = from.len() - 1;
    let samples = unique.min(MAX_CURVE_MATCH_SAMPLES);
    let from_sample: Vec<_> = (0..samples)
        .map(|index| from[index * unique / samples].position)
        .collect();
    let mut best = (f32::INFINITY, false, 0usize);
    for reverse in [false, true] {
        let candidate: Vec<_> = if reverse {
            to[..unique].iter().copied().rev().collect()
        } else {
            to[..unique].to_vec()
        };
        for offset in 0..samples {
            let target: Vec<_> = (0..samples)
                .map(|index| candidate[((index + offset) % samples) * unique / samples].position)
                .collect();
            let cost = similarity_residual(&from_sample, &target);
            if cost < best.0 {
                best = (cost, reverse, offset);
            }
        }
    }
    let candidate: Vec<_> = if best.1 {
        to[..unique].iter().copied().rev().collect()
    } else {
        to[..unique].to_vec()
    };
    let offset = (best.2 * unique + samples / 2) / samples;
    let mut aligned: Vec<_> = (0..unique)
        .map(|index| candidate[(index + offset) % unique])
        .collect();
    aligned.push(aligned[0]);
    aligned
}

fn similarity_residual(from: &[Vec2], to: &[Vec2]) -> f32 {
    let from_center = centroid(from);
    let to_center = centroid(to);
    let (dot, cross, from_energy, to_energy) = from.iter().zip(to).fold(
        (0.0, 0.0, 0.0, 0.0),
        |(dot, cross, from_energy, to_energy), (from, to)| {
            let from = *from - from_center;
            let to = *to - to_center;
            (
                dot + from.dot(to),
                cross + from.perp_dot(to),
                from_energy + from.length_squared(),
                to_energy + to.length_squared(),
            )
        },
    );
    if from_energy <= f32::EPSILON || to_energy <= f32::EPSILON {
        return from_energy.max(to_energy);
    }
    1.0 - dot.hypot(cross) / (from_energy * to_energy).sqrt()
}

fn curve_alignment_cost<'a>(from: &[PaintPoint], to: impl Iterator<Item = &'a PaintPoint>) -> f32 {
    let from_positions: Vec<_> = from.iter().map(|point| point.position).collect();
    let to_positions: Vec<_> = to.map(|point| point.position).collect();
    similarity_residual(&from_positions, &to_positions)
}

fn curve_warp(from: &[PaintPoint], to: &[PaintPoint]) -> Vec<f32> {
    let count = from.len();
    let from_positions: Vec<_> = from.iter().map(|point| point.position).collect();
    let to_positions: Vec<_> = to.iter().map(|point| point.position).collect();
    let from_normalized = normalize_curve(&from_positions);
    let to_normalized = normalize_curve(&to_positions);
    let from_tangents = curve_tangents(&from_normalized);
    let to_tangents = curve_tangents(&to_normalized);
    let denominator = count.saturating_sub(1).max(1) as f32;
    let mut cost = vec![f32::INFINITY; count * count];
    for left in 0..count {
        for right in 0..count {
            let parameter_distance = (left as f32 - right as f32) / denominator;
            let local = POSITION_WEIGHT
                * from_normalized[left].distance_squared(to_normalized[right])
                + TANGENT_WEIGHT * (1.0 - from_tangents[left].dot(to_tangents[right])).max(0.0)
                + PARAMETER_WEIGHT * parameter_distance.powi(2);
            let previous = match (left, right) {
                (0, 0) => 0.0,
                (0, _) => cost[right - 1] + WARP_PENALTY,
                (_, 0) => cost[(left - 1) * count] + WARP_PENALTY,
                _ => cost[(left - 1) * count + right - 1]
                    .min(cost[(left - 1) * count + right] + WARP_PENALTY)
                    .min(cost[left * count + right - 1] + WARP_PENALTY),
            };
            cost[left * count + right] = local + previous;
        }
    }

    let mut totals = vec![0.0; count];
    let mut visits = vec![0usize; count];
    let (mut left, mut right) = (count - 1, count - 1);
    loop {
        totals[left] += right as f32 / denominator;
        visits[left] += 1;
        if left == 0 && right == 0 {
            break;
        }
        match (left, right) {
            (0, _) => right -= 1,
            (_, 0) => left -= 1,
            _ => {
                let diagonal = cost[(left - 1) * count + right - 1];
                let vertical = cost[(left - 1) * count + right] + WARP_PENALTY;
                let horizontal = cost[left * count + right - 1] + WARP_PENALTY;
                if diagonal <= vertical && diagonal <= horizontal {
                    left -= 1;
                    right -= 1;
                } else if vertical <= horizontal {
                    left -= 1;
                } else {
                    right -= 1;
                }
            }
        }
    }
    let mut previous = 0.0f32;
    let mut warp: Vec<_> = totals
        .into_iter()
        .zip(visits)
        .enumerate()
        .map(|(index, (total, visits))| {
            let matched = if visits == 0 {
                previous
            } else {
                total / visits as f32
            };
            let identity = index as f32 / denominator;
            let progress = (identity + (matched - identity) * DTW_WARP_WEIGHT).max(previous);
            previous = progress;
            progress
        })
        .collect();
    warp[0] = 0.0;
    warp[count - 1] = 1.0;
    warp
}

fn normalize_curve(points: &[Vec2]) -> Vec<Vec2> {
    let center = centroid(points);
    let length = polyline_length(points).max(f32::EPSILON);
    let endpoints =
        points.last().copied().unwrap_or(center) - points.first().copied().unwrap_or(center);
    let radial = points.first().copied().unwrap_or(center) - center;
    let direction = if endpoints.length_squared() > f32::EPSILON {
        endpoints
    } else {
        radial
    };
    let rotation = if direction.length_squared() <= f32::EPSILON {
        Mat2::IDENTITY
    } else {
        Mat2::from_angle(-direction.to_angle())
    };
    points
        .iter()
        .map(|point| rotation * (*point - center) / length)
        .collect()
}

fn curve_tangents(points: &[Vec2]) -> Vec<Vec2> {
    (0..points.len())
        .map(|index| {
            let previous = points[index.saturating_sub(1)];
            let next = points[(index + 1).min(points.len() - 1)];
            (next - previous).normalize_or_zero()
        })
        .collect()
}
