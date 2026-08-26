use hashbrown::HashSet;

use glam::Vec2;
use rayon::prelude::*;

use super::{PaintDrawing, PaintFill, PaintPoint, PaintStroke};
use correspondence::correspond_stroke_points;
use deformation::{interpolate_closed_positions, interpolate_positions};
use geometry::{bounds, centroid};
use matching::{match_fills, match_loops, match_strokes};
use topology::normalize_stroke_counts;

mod assignment;
mod correspondence;
mod deformation;
mod geometry;
mod matching;
mod topology;

const NEUTRAL_PRESSURE: f32 = 0.5;

pub(super) fn interpolate_drawing(
    from: &PaintDrawing,
    to: &PaintDrawing,
    progress: f32,
) -> PaintDrawing {
    if progress <= 0.0 {
        return from.clone();
    }
    if progress >= 1.0 {
        return to.clone();
    }

    let scale = drawing_scale(from, to);
    let (from_strokes, to_strokes) = normalize_stroke_counts(&from.strokes, &to.strokes, scale);
    let stroke_pairs = match_strokes(&from_strokes, &to_strokes, scale);
    let strokes = stroke_pairs
        .par_iter()
        .map(|&(from_index, to_index)| {
            interpolate_stroke(&from_strokes[from_index], &to_strokes[to_index], progress)
        })
        .collect();

    let fill_pairs = match_fills(&from.fills, &to.fills, scale);
    let mut used_from = HashSet::new();
    let mut used_to = HashSet::new();
    for &(from_index, to_index) in &fill_pairs {
        used_from.insert(from_index);
        used_to.insert(to_index);
    }
    let mut fills: Vec<_> = fill_pairs
        .par_iter()
        .map(|&(from_index, to_index)| {
            interpolate_fill(&from.fills[from_index], &to.fills[to_index], progress)
        })
        .collect();
    fills.extend(
        from.fills
            .iter()
            .enumerate()
            .filter(|(index, _)| !used_from.contains(index))
            .map(|(_, fill)| collapse_fill(fill, 1.0 - progress, false)),
    );
    fills.extend(
        to.fills
            .iter()
            .enumerate()
            .filter(|(index, _)| !used_to.contains(index))
            .map(|(_, fill)| collapse_fill(fill, progress, true)),
    );

    PaintDrawing { strokes, fills }
}

fn interpolate_stroke(from: &PaintStroke, to: &PaintStroke, progress: f32) -> PaintStroke {
    let (from_points, to_points) = correspond_stroke_points(from, to);
    let from_positions: Vec<_> = from_points.iter().map(|point| point.position).collect();
    let to_positions: Vec<_> = to_points.iter().map(|point| point.position).collect();
    let positions = interpolate_positions(&from_positions, &to_positions, progress);
    let points = from_points
        .iter()
        .zip(&to_points)
        .zip(positions)
        .map(|((from, to), position)| PaintPoint {
            position,
            pressure: interpolate_pressure(from.pressure, to.pressure, progress),
        })
        .collect();
    PaintStroke {
        id: from.id,
        correspondence_id: from.correspondence_id,
        width_scale: from.width_scale + (to.width_scale - from.width_scale) * progress,
        color_index: if progress < 0.5 {
            from.color_index
        } else {
            to.color_index
        },
        points,
    }
}

fn interpolate_fill(from: &PaintFill, to: &PaintFill, progress: f32) -> PaintFill {
    let from_center = centroid(&from.loops.iter().flatten().copied().collect::<Vec<_>>());
    let to_center = centroid(&to.loops.iter().flatten().copied().collect::<Vec<_>>());
    let pairs = match_loops(&from.loops, &to.loops);
    let matched_from: HashSet<_> = pairs.iter().map(|(left, _)| *left).collect();
    let matched_to: HashSet<_> = pairs.iter().map(|(_, right)| *right).collect();
    let mut loops = Vec::with_capacity(from.loops.len() + to.loops.len() - pairs.len());
    for (left, right) in pairs {
        loops.push(interpolate_loop(
            &from.loops[left],
            &to.loops[right],
            progress,
        ));
    }
    loops.extend(
        from.loops
            .iter()
            .enumerate()
            .filter(|(index, _)| !matched_from.contains(index))
            .map(|(_, boundary)| {
                boundary
                    .iter()
                    .map(|point| from_center.lerp(*point, 1.0 - progress))
                    .collect()
            }),
    );
    loops.extend(
        to.loops
            .iter()
            .enumerate()
            .filter(|(index, _)| !matched_to.contains(index))
            .map(|(_, boundary)| {
                boundary
                    .iter()
                    .map(|point| to_center.lerp(*point, progress))
                    .collect()
            }),
    );
    PaintFill {
        id: from.id,
        correspondence_id: from.correspondence_id,
        seed: from.seed.lerp(to.seed, progress),
        color_index: if progress < 0.5 {
            from.color_index
        } else {
            to.color_index
        },
        loops,
    }
}

fn interpolate_loop(from: &[Vec2], to: &[Vec2], progress: f32) -> Vec<Vec2> {
    let count = from.len().max(to.len()).max(3);
    let from = resample_closed(from, count);
    let target = resample_closed(to, count);
    let candidate = (0..2 * count)
        .into_par_iter()
        .map(|candidate| {
            let reverse = candidate >= count;
            let offset = candidate % count;
            let cost: f32 = from
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    let index = (index + offset) % count;
                    let index = if reverse { count - 1 - index } else { index };
                    point.distance_squared(target[index])
                })
                .sum();
            (candidate, cost)
        })
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(candidate, _)| candidate)
        .unwrap();
    let reverse = candidate >= count;
    let offset = candidate % count;
    let best: Vec<_> = (0..count)
        .map(|index| {
            let index = (index + offset) % count;
            target[if reverse { count - 1 - index } else { index }]
        })
        .collect();
    interpolate_closed_positions(&from, &best, progress)
}

fn collapse_fill(fill: &PaintFill, amount: f32, _entering: bool) -> PaintFill {
    let center = centroid(&fill.loops.iter().flatten().copied().collect::<Vec<_>>());
    let mut fill = fill.clone();
    fill.seed = center.lerp(fill.seed, amount);
    for point in fill.loops.iter_mut().flatten() {
        *point = center.lerp(*point, amount);
    }
    fill
}

fn resample_stroke(points: &[PaintPoint], count: usize) -> Vec<PaintPoint> {
    if points.is_empty() {
        return vec![
            PaintPoint {
                position: Vec2::ZERO,
                pressure: None,
            };
            count
        ];
    }
    if points.len() == 1 {
        return vec![points[0]; count];
    }
    let positions: Vec<_> = points.iter().map(|point| point.position).collect();
    let distances = cumulative_lengths(&positions, false);
    let total = *distances.last().unwrap_or(&0.0);
    if total <= f32::EPSILON {
        return vec![points[0]; count];
    }
    (0..count)
        .map(|index| {
            let distance = total * index as f32 / count.saturating_sub(1).max(1) as f32;
            sample_stroke(points, &distances, distance)
        })
        .collect()
}

fn sample_stroke(points: &[PaintPoint], distances: &[f32], distance: f32) -> PaintPoint {
    let right = distances
        .iter()
        .position(|value| *value >= distance)
        .unwrap_or(points.len() - 1);
    if right == 0 {
        return points[0];
    }
    let left = right - 1;
    let span = distances[right] - distances[left];
    let progress = if span <= f32::EPSILON {
        0.0
    } else {
        (distance - distances[left]) / span
    };
    PaintPoint {
        position: points[left].position.lerp(points[right].position, progress),
        pressure: interpolate_pressure(points[left].pressure, points[right].pressure, progress),
    }
}

fn interpolate_pressure(from: Option<f32>, to: Option<f32>, progress: f32) -> Option<f32> {
    match (from, to) {
        (None, None) => None,
        (from, to) => {
            let from = from.unwrap_or(NEUTRAL_PRESSURE);
            let to = to.unwrap_or(NEUTRAL_PRESSURE);
            Some(from + (to - from) * progress)
        }
    }
}

fn resample_closed(points: &[Vec2], count: usize) -> Vec<Vec2> {
    if points.is_empty() {
        return vec![Vec2::ZERO; count];
    }
    if points.len() == 1 {
        return vec![points[0]; count];
    }
    let distances = cumulative_lengths(points, true);
    let total = *distances.last().unwrap_or(&0.0);
    if total <= f32::EPSILON {
        return vec![points[0]; count];
    }
    (0..count)
        .map(|index| {
            let distance = total * index as f32 / count as f32;
            let right = distances
                .iter()
                .position(|value| *value >= distance)
                .unwrap_or(points.len());
            let left = if right == 0 { 0 } else { right - 1 };
            let right_index = right % points.len();
            let span = distances[right] - distances[left];
            let progress = if span <= f32::EPSILON {
                0.0
            } else {
                (distance - distances[left]) / span
            };
            points[left % points.len()].lerp(points[right_index], progress)
        })
        .collect()
}

fn cumulative_lengths(points: &[Vec2], closed: bool) -> Vec<f32> {
    let mut distances = Vec::with_capacity(points.len() + usize::from(closed));
    distances.push(0.0);
    for pair in points.windows(2) {
        distances.push(distances.last().copied().unwrap_or(0.0) + pair[0].distance(pair[1]));
    }
    if closed {
        distances.push(
            distances.last().copied().unwrap_or(0.0)
                + points
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .distance(points[0]),
        );
    }
    distances
}

fn drawing_scale(from: &PaintDrawing, to: &PaintDrawing) -> f32 {
    let points: Vec<_> = from
        .strokes
        .iter()
        .chain(&to.strokes)
        .flat_map(|stroke| stroke.points.iter().map(|point| point.position))
        .chain(
            from.fills
                .iter()
                .chain(&to.fills)
                .flat_map(|fill| fill.loops.iter().flatten().copied()),
        )
        .collect();
    bounds(&points)
        .map(|(minimum, maximum)| (maximum - minimum).length().max(1.0))
        .unwrap_or(1.0)
}
