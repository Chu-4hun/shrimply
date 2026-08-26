use glam::Vec2;

pub(super) fn centroid(points: &[Vec2]) -> Vec2 {
    if points.is_empty() {
        return Vec2::ZERO;
    }
    points.iter().sum::<Vec2>() / points.len() as f32
}

pub(super) fn bounds(points: &[Vec2]) -> Option<(Vec2, Vec2)> {
    let first = *points.first()?;
    Some(
        points
            .iter()
            .skip(1)
            .fold((first, first), |(min, max), point| {
                (min.min(*point), max.max(*point))
            }),
    )
}

pub(super) fn polyline_length(points: &[Vec2]) -> f32 {
    points
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum()
}

pub(super) fn mean_abs_turn(points: &[Vec2]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    points
        .windows(3)
        .map(|window| {
            let first = window[1] - window[0];
            let second = window[2] - window[1];
            first.perp_dot(second).atan2(first.dot(second)).abs()
        })
        .sum::<f32>()
        / (points.len() - 2) as f32
}

pub(super) fn ratio_distance(left: f32, right: f32) -> f32 {
    ((left.max(f32::EPSILON) / right.max(f32::EPSILON)).ln()).abs()
}

pub(super) fn total_area(loops: &[Vec<Vec2>]) -> f32 {
    loops.iter().map(|points| loop_area(points)).sum()
}

pub(super) fn loop_area(points: &[Vec2]) -> f32 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(left, right)| left.perp_dot(*right))
        .sum::<f32>()
        .abs()
        * 0.5
}

pub(super) fn closed_length(points: &[Vec2]) -> f32 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(left, right)| left.distance(*right))
        .sum()
}
