use std::ops::BitOr;

use glam::{Mat3, Vec2};

use crate::Rect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapAxis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisFeature {
    Minimum,
    Center,
    Maximum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AxisFeatures(u8);

impl AxisFeatures {
    pub const NONE: Self = Self(0);
    pub const MINIMUM: Self = Self(1 << AxisFeature::Minimum as u8);
    pub const CENTER: Self = Self(1 << AxisFeature::Center as u8);
    pub const MAXIMUM: Self = Self(1 << AxisFeature::Maximum as u8);
    pub const EDGES: Self = Self(Self::MINIMUM.0 | Self::MAXIMUM.0);
    pub const ALL: Self = Self(Self::EDGES.0 | Self::CENTER.0);

    pub const fn contains(self, feature: AxisFeature) -> bool {
        self.0 & (1 << feature as u8) != 0
    }
}

impl BitOr for AxisFeatures {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisGap {
    pub start: f32,
    pub end: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AxisSnapKind {
    Align,
    EqualGap {
        first: AxisGap,
        second: AxisGap,
        cross: f32,
    },
    Mirror {
        center: f32,
        peer: f32,
        cross: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisTarget {
    pub value: f32,
    pub priority: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisSnap {
    pub delta: f32,
    pub source: f32,
    pub target: f32,
    pub feature: AxisFeature,
    pub kind: AxisSnapKind,
    pub priority: u8,
}

#[derive(Clone, Copy, Default)]
pub struct Snap2d {
    pub x: Option<AxisSnap>,
    pub y: Option<AxisSnap>,
}

impl Snap2d {
    pub fn delta(self) -> Vec2 {
        Vec2::new(
            self.x.map_or(0.0, |snap| snap.delta),
            self.y.map_or(0.0, |snap| snap.delta),
        )
    }
}

pub fn transformed_rect_lines(matrix: Mat3, bounds: Rect) -> ([f32; 3], [f32; 3]) {
    let bounds = transformed_rect(matrix, bounds);
    (
        axis_values(bounds, SnapAxis::X),
        axis_values(bounds, SnapAxis::Y),
    )
}

pub fn transformed_rect(matrix: Mat3, bounds: Rect) -> Rect {
    let corners = [
        matrix.transform_point2(bounds.min),
        matrix.transform_point2(Vec2::new(bounds.max.x, bounds.min.y)),
        matrix.transform_point2(bounds.max),
        matrix.transform_point2(Vec2::new(bounds.min.x, bounds.max.y)),
    ];
    let mut minimum = corners[0];
    let mut maximum = corners[0];
    for corner in &corners[1..] {
        minimum = minimum.min(*corner);
        maximum = maximum.max(*corner);
    }
    Rect::from_min_max(minimum, maximum)
}

pub fn nearest_rect_axis_snap(
    source: Rect,
    peers: &[Rect],
    explicit: &[AxisTarget],
    canvas_size: Vec2,
    axis: SnapAxis,
    allowed_features: AxisFeatures,
    radius: f32,
) -> Option<AxisSnap> {
    const RECT_PRIORITY: u8 = 2;
    const GAP_PRIORITY: u8 = 3;
    const MIRROR_PRIORITY: u8 = 4;

    if !rect_is_valid(source) || !radius.is_finite() || radius < 0.0 {
        return None;
    }

    let mut nearest = None;
    let source_values = axis_values(source, axis);
    for target in explicit.iter().filter(|target| target.value.is_finite()) {
        for (feature, value) in axis_features(source_values)
            .into_iter()
            .filter(|(feature, _)| allowed_features.contains(*feature))
        {
            add_axis_snap(
                &mut nearest,
                AxisSnap {
                    delta: target.value - value,
                    source: value,
                    target: target.value,
                    feature,
                    kind: AxisSnapKind::Align,
                    priority: target.priority,
                },
                radius,
            );
        }
    }

    let eligible = peers
        .iter()
        .copied()
        .filter(|peer| {
            rect_is_valid(*peer) && intervals_near(source, *peer, perpendicular(axis), radius)
        })
        .collect::<Vec<_>>();
    for peer in &eligible {
        for target in axis_values(*peer, axis) {
            for (feature, value) in axis_features(source_values)
                .into_iter()
                .filter(|(feature, _)| allowed_features.contains(*feature))
            {
                add_axis_snap(
                    &mut nearest,
                    AxisSnap {
                        delta: target - value,
                        source: value,
                        target,
                        feature,
                        kind: AxisSnapKind::Align,
                        priority: RECT_PRIORITY,
                    },
                    radius,
                );
            }
        }

        let extent = axis_extent(canvas_size, axis);
        if allowed_features.contains(AxisFeature::Center) && extent.is_finite() {
            let target = extent - axis_center(*peer, axis);
            let source_center = axis_center(source, axis);
            add_axis_snap(
                &mut nearest,
                AxisSnap {
                    delta: target - source_center,
                    source: source_center,
                    target,
                    feature: AxisFeature::Center,
                    kind: AxisSnapKind::Mirror {
                        center: extent * 0.5,
                        peer: axis_center(*peer, axis),
                        cross: relationship_cross(source, &[*peer], axis),
                    },
                    priority: MIRROR_PRIORITY,
                },
                radius,
            );
        }
    }

    let mut ordered = eligible;
    ordered.sort_by(|left, right| axis_min(*left, axis).total_cmp(&axis_min(*right, axis)));
    for (before_index, before) in ordered.iter().copied().enumerate() {
        for (after_index, after) in ordered.iter().copied().enumerate().skip(before_index + 1) {
            if !intervals_near(before, after, perpendicular(axis), radius) {
                continue;
            }
            let gap = axis_min(after, axis) - axis_max(before, axis);
            if !gap.is_finite() || gap <= 0.0 {
                continue;
            }
            let pair = [before, after];
            let cross = relationship_cross(source, &pair, axis);
            if !gap_is_empty(
                &ordered,
                [before_index, after_index],
                axis_max(before, axis),
                axis_min(after, axis),
                cross,
                axis,
            ) {
                continue;
            }
            for (feature, source_value, target, first, second, empty) in [
                (
                    AxisFeature::Minimum,
                    axis_min(source, axis),
                    axis_max(after, axis) + gap,
                    AxisGap {
                        start: axis_max(before, axis),
                        end: axis_min(after, axis),
                    },
                    AxisGap {
                        start: axis_max(after, axis),
                        end: axis_max(after, axis) + gap,
                    },
                    gap_is_empty(
                        &ordered,
                        [before_index, after_index],
                        axis_max(after, axis),
                        axis_max(after, axis) + gap,
                        cross,
                        axis,
                    ),
                ),
                (
                    AxisFeature::Maximum,
                    axis_max(source, axis),
                    axis_min(before, axis) - gap,
                    AxisGap {
                        start: axis_min(before, axis) - gap,
                        end: axis_min(before, axis),
                    },
                    AxisGap {
                        start: axis_max(before, axis),
                        end: axis_min(after, axis),
                    },
                    gap_is_empty(
                        &ordered,
                        [before_index, after_index],
                        axis_min(before, axis) - gap,
                        axis_min(before, axis),
                        cross,
                        axis,
                    ),
                ),
            ] {
                if !allowed_features.contains(feature) || !empty {
                    continue;
                }
                let delta = target - source_value;
                add_axis_snap(
                    &mut nearest,
                    AxisSnap {
                        delta,
                        source: source_value,
                        target,
                        feature,
                        kind: AxisSnapKind::EqualGap {
                            first,
                            second,
                            cross,
                        },
                        priority: GAP_PRIORITY,
                    },
                    radius,
                );
            }

            let opening = axis_min(after, axis) - axis_max(before, axis);
            let source_size = axis_size(source, axis);
            if allowed_features.contains(AxisFeature::Center)
                && opening.is_finite()
                && source_size.is_finite()
                && opening >= source_size
            {
                let target = midpoint(axis_max(before, axis), axis_min(after, axis));
                let delta = target - axis_center(source, axis);
                let snapped_min = axis_min(source, axis) + delta;
                let snapped_max = axis_max(source, axis) + delta;
                add_axis_snap(
                    &mut nearest,
                    AxisSnap {
                        delta,
                        source: axis_center(source, axis),
                        target,
                        feature: AxisFeature::Center,
                        kind: AxisSnapKind::EqualGap {
                            first: AxisGap {
                                start: axis_max(before, axis),
                                end: snapped_min,
                            },
                            second: AxisGap {
                                start: snapped_max,
                                end: axis_min(after, axis),
                            },
                            cross,
                        },
                        priority: GAP_PRIORITY,
                    },
                    radius,
                );
            }
        }
    }
    nearest
}

pub fn nearest_2d_snap(
    x_sources: [f32; 3],
    y_sources: [f32; 3],
    x_targets: impl IntoIterator<Item = f32>,
    y_targets: impl IntoIterator<Item = f32>,
    radius: f32,
) -> Snap2d {
    Snap2d {
        x: nearest_axis_snap(x_sources, x_targets, radius),
        y: nearest_axis_snap(y_sources, y_targets, radius),
    }
}

pub fn nearest_axis_snap(
    sources: [f32; 3],
    targets: impl IntoIterator<Item = f32>,
    radius: f32,
) -> Option<AxisSnap> {
    if !radius.is_finite() || radius < 0.0 {
        return None;
    }

    let mut nearest = None;
    for target in targets.into_iter().filter(|target| target.is_finite()) {
        for (feature, source) in axis_features(sources)
            .into_iter()
            .filter(|(_, source)| source.is_finite())
        {
            let candidate = AxisSnap {
                delta: target - source,
                source,
                target,
                feature,
                kind: AxisSnapKind::Align,
                priority: 0,
            };
            if candidate.delta.abs() > radius
                || nearest
                    .is_some_and(|current: AxisSnap| current.delta.abs() <= candidate.delta.abs())
            {
                continue;
            }
            nearest = Some(candidate);
        }
    }
    nearest
}

fn add_axis_snap(nearest: &mut Option<AxisSnap>, candidate: AxisSnap, radius: f32) {
    if !radius.is_finite()
        || radius < 0.0
        || !candidate.delta.is_finite()
        || !candidate.source.is_finite()
        || !candidate.target.is_finite()
        || !axis_snap_kind_is_finite(candidate.kind)
        || candidate.delta.abs() > radius
    {
        return;
    }
    let replace = nearest.is_none_or(|current| {
        candidate.delta.abs() < current.delta.abs()
            || (candidate.delta.abs() == current.delta.abs()
                && candidate.priority < current.priority)
    });
    if replace {
        *nearest = Some(candidate);
    }
}

fn axis_snap_kind_is_finite(kind: AxisSnapKind) -> bool {
    match kind {
        AxisSnapKind::Align => true,
        AxisSnapKind::EqualGap {
            first,
            second,
            cross,
        } => {
            first.start.is_finite()
                && first.end.is_finite()
                && second.start.is_finite()
                && second.end.is_finite()
                && cross.is_finite()
        }
        AxisSnapKind::Mirror {
            center,
            peer,
            cross,
        } => center.is_finite() && peer.is_finite() && cross.is_finite(),
    }
}

fn axis_features(values: [f32; 3]) -> [(AxisFeature, f32); 3] {
    [
        (AxisFeature::Minimum, values[0]),
        (AxisFeature::Center, values[1]),
        (AxisFeature::Maximum, values[2]),
    ]
}

fn axis_values(rect: Rect, axis: SnapAxis) -> [f32; 3] {
    let minimum = axis_min(rect, axis);
    let maximum = axis_max(rect, axis);
    [minimum, midpoint(minimum, maximum), maximum]
}

fn rect_is_valid(rect: Rect) -> bool {
    rect.min.is_finite() && rect.max.is_finite() && rect.min.cmple(rect.max).all()
}

fn axis_min(rect: Rect, axis: SnapAxis) -> f32 {
    match axis {
        SnapAxis::X => rect.min.x,
        SnapAxis::Y => rect.min.y,
    }
}

fn axis_max(rect: Rect, axis: SnapAxis) -> f32 {
    match axis {
        SnapAxis::X => rect.max.x,
        SnapAxis::Y => rect.max.y,
    }
}

fn axis_center(rect: Rect, axis: SnapAxis) -> f32 {
    midpoint(axis_min(rect, axis), axis_max(rect, axis))
}

fn midpoint(left: f32, right: f32) -> f32 {
    left * 0.5 + right * 0.5
}

fn axis_size(rect: Rect, axis: SnapAxis) -> f32 {
    axis_max(rect, axis) - axis_min(rect, axis)
}

fn axis_extent(size: Vec2, axis: SnapAxis) -> f32 {
    match axis {
        SnapAxis::X => size.x,
        SnapAxis::Y => size.y,
    }
}

fn perpendicular(axis: SnapAxis) -> SnapAxis {
    match axis {
        SnapAxis::X => SnapAxis::Y,
        SnapAxis::Y => SnapAxis::X,
    }
}

fn intervals_near(left: Rect, right: Rect, axis: SnapAxis, tolerance: f32) -> bool {
    axis_min(left, axis) <= axis_max(right, axis) + tolerance
        && axis_min(right, axis) <= axis_max(left, axis) + tolerance
}

fn gap_is_empty(
    peers: &[Rect],
    excluded: [usize; 2],
    start: f32,
    end: f32,
    cross: f32,
    axis: SnapAxis,
) -> bool {
    if !start.is_finite() || !end.is_finite() || !cross.is_finite() || start >= end {
        return false;
    }
    let cross_axis = perpendicular(axis);
    peers.iter().enumerate().all(|(index, peer)| {
        excluded.contains(&index)
            || axis_max(*peer, axis) <= start
            || axis_min(*peer, axis) >= end
            || cross < axis_min(*peer, cross_axis)
            || cross > axis_max(*peer, cross_axis)
    })
}

fn relationship_cross(source: Rect, peers: &[Rect], axis: SnapAxis) -> f32 {
    let axis = perpendicular(axis);
    let mut minimum = axis_min(source, axis);
    let mut maximum = axis_max(source, axis);
    for peer in peers {
        minimum = minimum.max(axis_min(*peer, axis));
        maximum = maximum.min(axis_max(*peer, axis));
    }
    if minimum <= maximum {
        midpoint(minimum, maximum)
    } else {
        let weight = 1.0 / peers.len().max(1) as f32;
        let peer_center = peers
            .iter()
            .map(|peer| axis_center(*peer, axis))
            .map(|center| center * weight)
            .sum::<f32>();
        midpoint(axis_center(source, axis), peer_center)
    }
}

pub fn nearest_angle_degrees(angle: f32, step: f32, radius: f32) -> Option<f32> {
    if !angle.is_finite() || !step.is_finite() || step <= 0.0 || !radius.is_finite() || radius < 0.0
    {
        return None;
    }
    let snapped = (angle / step).round() * step;
    ((angle - snapped).abs() <= radius).then_some(snapped)
}

pub fn rect_axis_feature(rect: Rect, axis: SnapAxis, feature: AxisFeature) -> f32 {
    let (minimum, maximum) = match axis {
        SnapAxis::X => (rect.min.x, rect.max.x),
        SnapAxis::Y => (rect.min.y, rect.max.y),
    };
    match feature {
        AxisFeature::Minimum => minimum,
        AxisFeature::Center => midpoint(minimum, maximum),
        AxisFeature::Maximum => maximum,
    }
}

pub fn resolve_axis_snap(mut snap: AxisSnap, rect: Rect, axis: SnapAxis) -> AxisSnap {
    snap.source = rect_axis_feature(rect, axis, snap.feature);
    snap.delta = snap.target - snap.source;
    if let AxisSnapKind::EqualGap {
        mut first,
        mut second,
        cross,
    } = snap.kind
    {
        match snap.feature {
            AxisFeature::Minimum => second.end = snap.source,
            AxisFeature::Center => {
                first.end = axis_min(rect, axis);
                second.start = axis_max(rect, axis);
            }
            AxisFeature::Maximum => first.start = snap.source,
        }
        snap.kind = AxisSnapKind::EqualGap {
            first,
            second,
            cross,
        };
    }
    snap
}

pub fn solve_parameter_correction(
    derivatives: [Vec2; 2],
    residual: Vec2,
    active: [bool; 2],
) -> Vec2 {
    if !active[0] && !active[1] {
        return Vec2::ZERO;
    }
    if !active[0] || !active[1] {
        let index = usize::from(active[1]);
        let denominator = derivatives[index].length_squared();
        if denominator <= f32::EPSILON {
            return Vec2::ZERO;
        }
        let mut correction = Vec2::ZERO;
        correction[index] = residual.dot(derivatives[index]) / denominator;
        return correction;
    }

    let a = derivatives[0].length_squared();
    let b = derivatives[0].dot(derivatives[1]);
    let c = derivatives[1].length_squared();
    let right = Vec2::new(derivatives[0].dot(residual), derivatives[1].dot(residual));
    let determinant = a * c - b * b;
    if determinant.abs() > f32::EPSILON {
        return Vec2::new(c * right.x - b * right.y, a * right.y - b * right.x) / determinant;
    }

    let projected = derivatives[0] * right.x + derivatives[1] * right.y;
    let denominator = projected.length_squared();
    if denominator <= f32::EPSILON {
        Vec2::ZERO
    } else {
        right * (right.length_squared() / denominator)
    }
}
