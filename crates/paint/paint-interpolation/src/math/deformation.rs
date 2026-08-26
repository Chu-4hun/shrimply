use glam::{Mat2, Vec2};

use super::geometry::centroid;

const INTRINSIC_SHAPE_WEIGHT: f32 = 0.35;

pub(super) fn interpolate_positions(from: &[Vec2], to: &[Vec2], progress: f32) -> Vec<Vec2> {
    if from.len() < 2 || from.len() != to.len() {
        return from
            .iter()
            .zip(to)
            .map(|(from, to)| from.lerp(*to, progress))
            .collect();
    }
    let motion = spiral_positions(from, to, progress);
    let intrinsic = intrinsic_curve(from, to, progress, false);
    let intrinsic = fit_open_endpoints(&intrinsic, motion[0], *motion.last().unwrap_or(&motion[0]));
    motion
        .into_iter()
        .zip(intrinsic)
        .map(|(motion, intrinsic)| motion.lerp(intrinsic, INTRINSIC_SHAPE_WEIGHT))
        .collect()
}

pub(super) fn interpolate_closed_positions(from: &[Vec2], to: &[Vec2], progress: f32) -> Vec<Vec2> {
    if from.len() < 3 || from.len() != to.len() {
        return from
            .iter()
            .zip(to)
            .map(|(from, to)| from.lerp(*to, progress))
            .collect();
    }
    let motion = spiral_positions(from, to, progress);
    align_similarity(&intrinsic_curve(from, to, progress, true), &motion)
}

fn spiral_positions(from: &[Vec2], to: &[Vec2], progress: f32) -> Vec<Vec2> {
    let Some(transform) = similarity(from, to) else {
        return from
            .iter()
            .zip(to)
            .map(|(from, to)| from.lerp(*to, progress))
            .collect();
    };
    from.iter()
        .zip(to)
        .map(|(from, to)| {
            let forward = transform.apply_from(*from, progress);
            let backward = transform.apply_to(*to, progress);
            forward.lerp(backward, progress)
        })
        .collect()
}

fn intrinsic_curve(from: &[Vec2], to: &[Vec2], progress: f32, closed: bool) -> Vec<Vec2> {
    let edge_count = if closed { from.len() } else { from.len() - 1 };
    let edge = |points: &[Vec2], index: usize| points[(index + 1) % points.len()] - points[index];
    let mut angle = lerp_angle(edge(from, 0).to_angle(), edge(to, 0).to_angle(), progress);
    let mut edges = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        if index > 0 {
            let from_turn = angle_delta(
                edge(from, index - 1).to_angle(),
                edge(from, index).to_angle(),
            );
            let to_turn = angle_delta(edge(to, index - 1).to_angle(), edge(to, index).to_angle());
            angle += from_turn + angle_delta(from_turn, to_turn) * progress;
        }
        let from_length = edge(from, index).length().max(f32::EPSILON);
        let to_length = edge(to, index).length().max(f32::EPSILON);
        let length = (from_length.ln() + (to_length.ln() - from_length.ln()) * progress).exp();
        edges.push(Vec2::from_angle(angle) * length);
    }
    if closed {
        let closure = edges.iter().sum::<Vec2>() / edge_count as f32;
        for edge in &mut edges {
            *edge -= closure;
        }
    }
    let mut result = Vec::with_capacity(from.len());
    result.push(Vec2::ZERO);
    for edge in edges.into_iter().take(from.len() - 1) {
        result.push(result.last().copied().unwrap_or_default() + edge);
    }
    result
}

fn fit_open_endpoints(points: &[Vec2], start: Vec2, end: Vec2) -> Vec<Vec2> {
    let origin = points.first().copied().unwrap_or_default();
    let translated_end = start + points.last().copied().unwrap_or(origin) - origin;
    let correction = end - translated_end;
    let denominator = points.len().saturating_sub(1).max(1) as f32;
    points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let progress = index as f32 / denominator;
            let smooth = progress * progress * (3.0 - 2.0 * progress);
            start + (*point - origin) + correction * smooth
        })
        .collect()
}

fn align_similarity(points: &[Vec2], targets: &[Vec2]) -> Vec<Vec2> {
    let source_center = centroid(points);
    let target_center = centroid(targets);
    let (dot, cross, denominator) = points.iter().zip(targets).fold(
        (0.0, 0.0, 0.0),
        |(dot, cross, denominator), (source, target)| {
            let source = *source - source_center;
            let target = *target - target_center;
            (
                dot + source.dot(target),
                cross + source.perp_dot(target),
                denominator + source.length_squared(),
            )
        },
    );
    if denominator <= f32::EPSILON {
        return vec![target_center; points.len()];
    }
    let linear = Mat2::from_cols_array(&[
        dot / denominator,
        cross / denominator,
        -cross / denominator,
        dot / denominator,
    ]);
    points
        .iter()
        .map(|point| target_center + linear * (*point - source_center))
        .collect()
}

fn lerp_angle(from: f32, to: f32, progress: f32) -> f32 {
    from + angle_delta(from, to) * progress
}

pub(super) fn angle_delta(from: f32, to: f32) -> f32 {
    let mut delta = to - from;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

struct Similarity {
    from_center: Vec2,
    to_center: Vec2,
    angle: f32,
    scale: f32,
}

impl Similarity {
    fn center(&self, progress: f32) -> Vec2 {
        self.from_center.lerp(self.to_center, progress)
    }

    fn apply_from(&self, point: Vec2, progress: f32) -> Vec2 {
        self.center(progress)
            + Mat2::from_angle(self.angle * progress)
                * (point - self.from_center)
                * self.scale.powf(progress)
    }

    fn apply_to(&self, point: Vec2, progress: f32) -> Vec2 {
        let remaining = 1.0 - progress;
        self.center(progress)
            + Mat2::from_angle(-self.angle * remaining) * (point - self.to_center)
                / self.scale.powf(remaining)
    }
}

fn similarity(from: &[Vec2], to: &[Vec2]) -> Option<Similarity> {
    if from.len() != to.len() || from.is_empty() {
        return None;
    }
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
        return None;
    }
    Some(Similarity {
        from_center,
        to_center,
        angle: cross.atan2(dot),
        scale: (to_energy / from_energy).sqrt(),
    })
}
