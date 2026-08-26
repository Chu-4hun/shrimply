use glam::Vec2;
use uuid::Uuid;

use super::geometry::{centroid, polyline_length};
use crate::{PaintPoint, PaintStroke};

const MERGE_GAP_SCALE: f32 = 0.025;
const MERGE_MAX_TURN_RADIANS: f32 = std::f32::consts::PI * 2.0 / 3.0;
const MERGE_TURN_WEIGHT: f32 = 0.25;
const UUID_MIX: u128 = 0x9e37_79b9_7f4a_7c15_6a09_e667_f3bc_c909;

pub(super) fn normalize_stroke_counts(
    from: &[PaintStroke],
    to: &[PaintStroke],
    scale: f32,
) -> (Vec<PaintStroke>, Vec<PaintStroke>) {
    if from.is_empty() {
        return (collapsed_proxies(to), to.to_vec());
    }
    if to.is_empty() {
        return (from.to_vec(), collapsed_proxies(from));
    }

    let mut from = from.to_vec();
    let mut to = to.to_vec();
    while from.len() > to.len() {
        if !merge_best_pair(&mut from, scale) {
            split_longest(&mut to);
        }
    }
    while to.len() > from.len() {
        if !merge_best_pair(&mut to, scale) {
            split_longest(&mut from);
        }
    }
    (from, to)
}

fn collapsed_proxies(strokes: &[PaintStroke]) -> Vec<PaintStroke> {
    strokes
        .iter()
        .map(|stroke| {
            let mut stroke = stroke.clone();
            let center = centroid(
                &stroke
                    .points
                    .iter()
                    .map(|point| point.position)
                    .collect::<Vec<_>>(),
            );
            stroke.width_scale = 0.0;
            for point in &mut stroke.points {
                point.position = center;
            }
            stroke
        })
        .collect()
}

#[derive(Clone, Copy)]
struct MergeCandidate {
    left: usize,
    right: usize,
    reverse_left: bool,
    reverse_right: bool,
    score: f32,
}

fn merge_best_pair(strokes: &mut Vec<PaintStroke>, scale: f32) -> bool {
    let maximum_gap = scale.max(1.0) * MERGE_GAP_SCALE;
    let mut best = None::<MergeCandidate>;
    for left in 0..strokes.len() {
        for right in left + 1..strokes.len() {
            if strokes[left].color_index != strokes[right].color_index {
                continue;
            }
            for reverse_left in [false, true] {
                for reverse_right in [false, true] {
                    let Some((gap, turn)) =
                        join_cost(&strokes[left], &strokes[right], reverse_left, reverse_right)
                    else {
                        continue;
                    };
                    if gap > maximum_gap || turn > MERGE_MAX_TURN_RADIANS {
                        continue;
                    }
                    let score = gap / maximum_gap.max(f32::EPSILON)
                        + MERGE_TURN_WEIGHT * turn / std::f32::consts::PI;
                    if best.is_none_or(|candidate| score < candidate.score) {
                        best = Some(MergeCandidate {
                            left,
                            right,
                            reverse_left,
                            reverse_right,
                            score,
                        });
                    }
                }
            }
        }
    }
    let Some(best) = best else {
        return false;
    };
    let right = strokes.remove(best.right);
    let left = strokes.remove(best.left);
    strokes.insert(
        best.left,
        merge_strokes(left, right, best.reverse_left, best.reverse_right),
    );
    true
}

fn join_cost(
    left: &PaintStroke,
    right: &PaintStroke,
    reverse_left: bool,
    reverse_right: bool,
) -> Option<(f32, f32)> {
    let left_points = oriented_points(left, reverse_left);
    let right_points = oriented_points(right, reverse_right);
    let gap = left_points
        .last()?
        .position
        .distance(right_points.first()?.position);
    if left_points.len() < 2 || right_points.len() < 2 {
        return Some((gap, 0.0));
    }
    let incoming = (left_points[left_points.len() - 1].position
        - left_points[left_points.len() - 2].position)
        .normalize_or_zero();
    let outgoing = (right_points[1].position - right_points[0].position).normalize_or_zero();
    let turn = incoming.dot(outgoing).clamp(-1.0, 1.0).acos();
    Some((gap, turn))
}

fn merge_strokes(
    left: PaintStroke,
    right: PaintStroke,
    reverse_left: bool,
    reverse_right: bool,
) -> PaintStroke {
    let left_length = stroke_length(&left);
    let right_length = stroke_length(&right);
    let mut points = oriented_points(&left, reverse_left);
    let mut right_points = oriented_points(&right, reverse_right);
    if points
        .last()
        .zip(right_points.first())
        .is_some_and(|(left, right)| left.position.distance_squared(right.position) <= f32::EPSILON)
    {
        right_points.remove(0);
    }
    points.extend(right_points);
    let total = left_length + right_length;
    PaintStroke {
        id: merged_uuid(left.id, right.id),
        correspondence_id: merged_uuid(left.correspondence_id, right.correspondence_id),
        width_scale: if total <= f32::EPSILON {
            (left.width_scale + right.width_scale) * 0.5
        } else {
            (left.width_scale * left_length + right.width_scale * right_length) / total
        },
        color_index: left.color_index,
        points,
    }
}

fn oriented_points(stroke: &PaintStroke, reverse: bool) -> Vec<PaintPoint> {
    if reverse {
        stroke.points.iter().copied().rev().collect()
    } else {
        stroke.points.clone()
    }
}

fn split_longest(strokes: &mut Vec<PaintStroke>) {
    let Some((index, _)) = strokes
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| stroke_length(left).total_cmp(&stroke_length(right)))
    else {
        return;
    };
    let stroke = strokes.remove(index);
    let (left, right) = split_stroke(stroke);
    strokes.insert(index, right);
    strokes.insert(index, left);
}

fn split_stroke(stroke: PaintStroke) -> (PaintStroke, PaintStroke) {
    let positions: Vec<_> = stroke.points.iter().map(|point| point.position).collect();
    let total = polyline_length(&positions);
    let (left_points, right_points) = if stroke.points.len() < 2 || total <= f32::EPSILON {
        (stroke.points.clone(), stroke.points.clone())
    } else {
        split_points(&stroke.points, total * 0.5)
    };
    let left = PaintStroke {
        id: child_uuid(stroke.id, false),
        correspondence_id: child_uuid(stroke.correspondence_id, false),
        width_scale: stroke.width_scale,
        color_index: stroke.color_index,
        points: left_points,
    };
    let right = PaintStroke {
        id: child_uuid(stroke.id, true),
        correspondence_id: child_uuid(stroke.correspondence_id, true),
        width_scale: stroke.width_scale,
        color_index: stroke.color_index,
        points: right_points,
    };
    (left, right)
}

fn split_points(points: &[PaintPoint], distance: f32) -> (Vec<PaintPoint>, Vec<PaintPoint>) {
    let mut traversed = 0.0;
    for (index, pair) in points.windows(2).enumerate() {
        let length = pair[0].position.distance(pair[1].position);
        if traversed + length < distance {
            traversed += length;
            continue;
        }
        let progress = if length <= f32::EPSILON {
            0.0
        } else {
            (distance - traversed) / length
        };
        let split = PaintPoint {
            position: pair[0].position.lerp(pair[1].position, progress),
            pressure: match (pair[0].pressure, pair[1].pressure) {
                (None, None) => None,
                (left, right) => Some(
                    left.unwrap_or(0.5) + (right.unwrap_or(0.5) - left.unwrap_or(0.5)) * progress,
                ),
            },
        };
        let mut left = points[..=index].to_vec();
        let mut right = points[index + 1..].to_vec();
        left.push(split);
        right.insert(0, split);
        return (left, right);
    }
    (points.to_vec(), points.to_vec())
}

fn stroke_length(stroke: &PaintStroke) -> f32 {
    polyline_length(
        &stroke
            .points
            .iter()
            .map(|point| point.position)
            .collect::<Vec<Vec2>>(),
    )
}

fn merged_uuid(left: Uuid, right: Uuid) -> Uuid {
    let (left, right) = if left.as_u128() <= right.as_u128() {
        (left.as_u128(), right.as_u128())
    } else {
        (right.as_u128(), left.as_u128())
    };
    Uuid::from_u128(left.rotate_left(31) ^ right.rotate_right(29) ^ UUID_MIX)
}

fn child_uuid(parent: Uuid, right: bool) -> Uuid {
    let branch = if right { UUID_MIX } else { !UUID_MIX };
    Uuid::from_u128(parent.as_u128().rotate_left(47) ^ branch)
}
