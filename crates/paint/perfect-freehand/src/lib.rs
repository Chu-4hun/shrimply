// Ported from perfect-freehand, Copyright (c) 2021 Stephen Ruiz Ltd.
// SPDX-License-Identifier: MIT

//! Pressure-sensitive freehand stroke geometry.
//!
//! This is a Rust port of perfect-freehand's computational API. It deliberately
//! produces points rather than SVG or other rendered output.

use std::convert::identity;
use std::f32::consts::PI;

use glam::Vec2;

pub type Easing = fn(f32) -> f32;

const RATE_OF_PRESSURE_CHANGE: f32 = 0.275;
const FIXED_PI: f32 = PI + 0.0001;
const START_CAP_SEGMENTS: u32 = 13;
const END_CAP_SEGMENTS: u32 = 29;
const CORNER_CAP_SEGMENTS: u32 = 13;
const END_NOISE_THRESHOLD: f32 = 3.0;
const MIN_STREAMLINE_T: f32 = 0.15;
const STREAMLINE_T_RANGE: f32 = 0.85;
const MIN_RADIUS: f32 = 0.01;
const DEFAULT_FIRST_PRESSURE: f32 = 0.25;
const DEFAULT_PRESSURE: f32 = 0.5;
const DEFAULT_SIZE: f32 = 16.0;
const DEFAULT_THINNING: f32 = 0.5;
const DEFAULT_SMOOTHING: f32 = 0.5;
const DEFAULT_STREAMLINE: f32 = 0.5;
const UNIT_OFFSET: Vec2 = Vec2::ONE;
const FLAT_CAP_INNER_SCALE: f32 = 0.5;
const FLAT_CAP_OUTER_SCALE: f32 = 0.51;
const FLAT_END_INNER_SCALE: f32 = 0.99;

/// One raw input sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputPoint {
    pub point: Vec2,
    /// `None`, negative, and NaN pressures use perfect-freehand's defaults.
    pub pressure: Option<f32>,
}

impl From<Vec2> for InputPoint {
    fn from(point: Vec2) -> Self {
        Self {
            point,
            pressure: None,
        }
    }
}

/// A streamline-adjusted point returned by [`get_stroke_points`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokePoint {
    pub point: Vec2,
    pub pressure: f32,
    pub distance: f32,
    pub vector: Vec2,
    pub running_length: f32,
}

/// A start or end taper distance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Taper {
    /// Do not taper this end.
    #[default]
    None,
    /// Taper over the larger of the stroke size and total stroke length.
    Full,
    /// Taper over an exact distance.
    Distance(f32),
}

/// Cap and taper behavior for one end of a stroke.
#[derive(Clone, Copy, Debug)]
pub struct StrokeEndOptions {
    pub cap: bool,
    pub taper: Taper,
    pub easing: Easing,
}

impl StrokeEndOptions {
    pub fn start() -> Self {
        Self {
            cap: true,
            taper: Taper::None,
            easing: ease_out_quad,
        }
    }

    pub fn end() -> Self {
        Self {
            cap: true,
            taper: Taper::None,
            easing: ease_out_cubic,
        }
    }
}

/// Options shared by the three stroke generation operations.
#[derive(Clone, Copy, Debug)]
pub struct StrokeOptions {
    /// Base stroke diameter.
    pub size: f32,
    pub thinning: f32,
    pub smoothing: f32,
    pub streamline: f32,
    pub easing: Easing,
    pub simulate_pressure: bool,
    pub start: StrokeEndOptions,
    pub end: StrokeEndOptions,
    /// Whether the input represents a completed stroke.
    pub last: bool,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            thinning: DEFAULT_THINNING,
            smoothing: DEFAULT_SMOOTHING,
            streamline: DEFAULT_STREAMLINE,
            easing: identity,
            simulate_pressure: true,
            start: StrokeEndOptions::start(),
            end: StrokeEndOptions::end(),
            last: false,
        }
    }
}

/// Return the polygon surrounding raw input samples.
pub fn get_stroke(points: &[InputPoint], options: &StrokeOptions) -> Vec<Vec2> {
    get_stroke_outline_points(&get_stroke_points(points, options), options)
}

/// Streamline raw samples and calculate their stroke metadata.
pub fn get_stroke_points(points: &[InputPoint], options: &StrokeOptions) -> Vec<StrokePoint> {
    if points.is_empty() {
        return Vec::new();
    }

    let interpolation = MIN_STREAMLINE_T + (1.0 - options.streamline) * STREAMLINE_T_RANGE;
    let mut samples = points.to_vec();

    // Extra samples only prevent tapered two-point strokes from becoming
    // dashes. They distort ordinary short strokes, so leave those untouched.
    if samples.len() == 2
        && (!matches!(options.start.taper, Taper::None)
            || !matches!(options.end.taper, Taper::None))
    {
        let first = samples[0].point;
        let last = samples[1].point;
        let first_pressure = valid_pressure(samples[0].pressure);
        let last_pressure = valid_pressure(samples[1].pressure);
        samples.truncate(1);
        for index in 1..5 {
            let amount = index as f32 / 4.0;
            samples.push(InputPoint {
                point: first.lerp(last, amount),
                pressure: first_pressure
                    .zip(last_pressure)
                    .map(|(first, last)| first + (last - first) * amount),
            });
        }
    }

    if samples.len() == 1 {
        samples.push(InputPoint {
            point: samples[0].point + UNIT_OFFSET,
            pressure: samples[0].pressure,
        });
    }

    let mut stroke_points = vec![StrokePoint {
        point: samples[0].point,
        pressure: valid_pressure(samples[0].pressure).unwrap_or(DEFAULT_FIRST_PRESSURE),
        vector: UNIT_OFFSET,
        distance: 0.0,
        running_length: 0.0,
    }];
    let mut has_reached_minimum_length = false;
    let mut running_length = 0.0;
    let max = samples.len() - 1;

    for (index, sample) in samples.iter().enumerate().skip(1) {
        let previous = *stroke_points.last().expect("stroke starts with one point");
        let point = if options.last && index == max {
            sample.point
        } else {
            previous.point.lerp(sample.point, interpolation)
        };

        if previous.point == point {
            continue;
        }

        let distance = point.distance(previous.point);
        running_length += distance;
        if index < max && !has_reached_minimum_length {
            if running_length < options.size {
                continue;
            }
            has_reached_minimum_length = true;
        }

        stroke_points.push(StrokePoint {
            point,
            pressure: valid_pressure(sample.pressure).unwrap_or(DEFAULT_PRESSURE),
            vector: (previous.point - point).normalize(),
            distance,
            running_length,
        });
    }

    stroke_points[0].vector = stroke_points
        .get(1)
        .map_or(Vec2::ZERO, |point| point.vector);
    stroke_points
}

/// Expand streamline-adjusted points into a stroke outline polygon.
pub fn get_stroke_outline_points(points: &[StrokePoint], options: &StrokeOptions) -> Vec<Vec2> {
    if points.is_empty() || options.size <= 0.0 {
        return Vec::new();
    }

    let total_length = points.last().expect("stroke is nonempty").running_length;
    let taper_start = taper_distance(options.start.taper, options.size, total_length);
    let taper_end = taper_distance(options.end.taper, options.size, total_length);
    let min_distance = (options.size * options.smoothing).powi(2);
    let mut left_points = Vec::new();
    let mut right_points = Vec::new();
    let mut previous_pressure = initial_pressure(points, options.simulate_pressure, options.size);
    let mut radius = stroke_radius(
        options.size,
        options.thinning,
        points.last().expect("stroke is nonempty").pressure,
        options.easing,
    );
    let mut first_radius = None;
    let mut previous_vector = points[0].vector;
    let mut previous_left_point = points[0].point;
    let mut previous_right_point = previous_left_point;
    let mut previous_point_was_sharp = false;

    for (index, stroke_point) in points.iter().enumerate() {
        let mut pressure = stroke_point.pressure;
        let is_last_point = index == points.len() - 1;

        if !is_last_point && total_length - stroke_point.running_length < END_NOISE_THRESHOLD {
            continue;
        }

        if options.thinning != 0.0 {
            if options.simulate_pressure {
                pressure =
                    simulate_pressure(previous_pressure, stroke_point.distance, options.size);
            }
            radius = stroke_radius(options.size, options.thinning, pressure, options.easing);
        } else {
            radius = options.size / 2.0;
        }
        first_radius.get_or_insert(radius);

        let taper_start_strength = if stroke_point.running_length < taper_start {
            (options.start.easing)(stroke_point.running_length / taper_start)
        } else {
            1.0
        };
        let taper_end_strength = if total_length - stroke_point.running_length < taper_end {
            (options.end.easing)((total_length - stroke_point.running_length) / taper_end)
        } else {
            1.0
        };
        radius = js_max(
            MIN_RADIUS,
            radius * js_min(taper_start_strength, taper_end_strength),
        );

        let next_vector = if is_last_point {
            stroke_point.vector
        } else {
            points[index + 1].vector
        };
        let next_dot = if is_last_point {
            1.0
        } else {
            stroke_point.vector.dot(next_vector)
        };
        let previous_dot = stroke_point.vector.dot(previous_vector);
        let point_is_sharp = previous_dot < 0.0 && !previous_point_was_sharp;
        let next_point_is_sharp = next_dot < 0.0;

        if point_is_sharp || next_point_is_sharp {
            let offset = Vec2::new(previous_vector.y, -previous_vector.x) * radius;
            for segment in 0..=CORNER_CAP_SEGMENTS {
                let t = segment as f32 / CORNER_CAP_SEGMENTS as f32;
                let left = (-offset).rotate(Vec2::from_angle(FIXED_PI * t)) + stroke_point.point;
                let right = offset.rotate(Vec2::from_angle(-FIXED_PI * t)) + stroke_point.point;
                left_points.push(left);
                right_points.push(right);
                previous_left_point = left;
                previous_right_point = right;
            }
            if next_point_is_sharp {
                previous_point_was_sharp = true;
            }
            continue;
        }

        previous_point_was_sharp = false;
        if is_last_point {
            let offset = Vec2::new(stroke_point.vector.y, -stroke_point.vector.x) * radius;
            left_points.push(stroke_point.point - offset);
            right_points.push(stroke_point.point + offset);
            continue;
        }

        let direction = next_vector.lerp(stroke_point.vector, next_dot);
        let offset = Vec2::new(direction.y, -direction.x) * radius;
        let left = stroke_point.point - offset;
        if index <= 1 || previous_left_point.distance_squared(left) > min_distance {
            left_points.push(left);
            previous_left_point = left;
        }
        let right = stroke_point.point + offset;
        if index <= 1 || previous_right_point.distance_squared(right) > min_distance {
            right_points.push(right);
            previous_right_point = right;
        }

        previous_pressure = pressure;
        previous_vector = stroke_point.vector;
    }

    let first_point = points[0].point;
    let last_point = points.get(1).map_or(points[0].point + UNIT_OFFSET, |_| {
        points.last().expect("stroke is nonempty").point
    });

    if points.len() == 1 && ((!js_truthy(taper_start) && !js_truthy(taper_end)) || options.last) {
        return draw_dot(
            first_point,
            first_radius
                .filter(|radius| js_truthy(*radius))
                .unwrap_or(radius),
        );
    }

    let mut end_cap = Vec::new();
    let mut start_cap = Vec::new();
    if points.len() > 1 {
        if !js_truthy(taper_start) {
            if options.start.cap {
                start_cap = round_start_cap(first_point, right_points[0]);
            } else {
                start_cap = flat_start_cap(first_point, left_points[0], right_points[0]);
            }
        }

        let vector = -points.last().expect("stroke is nonempty").vector;
        let direction = Vec2::new(vector.y, -vector.x);
        if js_truthy(taper_end) {
            end_cap.push(last_point);
        } else if options.end.cap {
            end_cap = round_end_cap(last_point, direction, radius);
        } else {
            end_cap = flat_end_cap(last_point, direction, radius);
        }
    }

    left_points.extend(end_cap);
    right_points.reverse();
    left_points.extend(right_points);
    left_points.extend(start_cap);
    left_points
}

fn ease_out_quad(value: f32) -> f32 {
    value * (2.0 - value)
}

fn ease_out_cubic(value: f32) -> f32 {
    let value = value - 1.0;
    value * value * value + 1.0
}

fn valid_pressure(pressure: Option<f32>) -> Option<f32> {
    pressure.filter(|pressure| !pressure.is_nan() && *pressure >= 0.0)
}

fn js_truthy(value: f32) -> bool {
    value != 0.0 && !value.is_nan()
}

fn js_min(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else {
        left.min(right)
    }
}

fn js_max(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else {
        left.max(right)
    }
}

fn taper_distance(taper: Taper, size: f32, total_length: f32) -> f32 {
    match taper {
        Taper::None => 0.0,
        Taper::Full => js_max(size, total_length),
        Taper::Distance(distance) => distance,
    }
}

fn initial_pressure(points: &[StrokePoint], simulate: bool, size: f32) -> f32 {
    points
        .iter()
        .take(10)
        .fold(points[0].pressure, |acc, point| {
            let pressure = if simulate {
                simulate_pressure(acc, point.distance, size)
            } else {
                point.pressure
            };
            (acc + pressure) / 2.0
        })
}

fn simulate_pressure(previous_pressure: f32, distance: f32, size: f32) -> f32 {
    let speed = js_min(1.0, distance / size);
    let rate = js_min(1.0, 1.0 - speed);
    js_min(
        1.0,
        previous_pressure + (rate - previous_pressure) * (speed * RATE_OF_PRESSURE_CHANGE),
    )
}

fn stroke_radius(size: f32, thinning: f32, pressure: f32, easing: Easing) -> f32 {
    size * easing(0.5 - thinning * (0.5 - pressure))
}

fn draw_dot(center: Vec2, radius: f32) -> Vec<Vec2> {
    let direction = center - (center + UNIT_OFFSET);
    let start = center + Vec2::new(direction.y, -direction.x).normalize() * -radius;
    let mut points = Vec::new();
    for segment in 1..=START_CAP_SEGMENTS {
        let t = segment as f32 / START_CAP_SEGMENTS as f32;
        points.push((start - center).rotate(Vec2::from_angle(FIXED_PI * 2.0 * t)) + center);
    }
    points
}

fn round_start_cap(center: Vec2, right_point: Vec2) -> Vec<Vec2> {
    let mut cap = Vec::new();
    for segment in 1..=START_CAP_SEGMENTS {
        let t = segment as f32 / START_CAP_SEGMENTS as f32;
        cap.push((right_point - center).rotate(Vec2::from_angle(FIXED_PI * t)) + center);
    }
    cap
}

fn flat_start_cap(center: Vec2, left_point: Vec2, right_point: Vec2) -> Vec<Vec2> {
    let corners = left_point - right_point;
    let offset_a = corners * FLAT_CAP_INNER_SCALE;
    let offset_b = corners * FLAT_CAP_OUTER_SCALE;
    vec![
        center - offset_a,
        center - offset_b,
        center + offset_b,
        center + offset_a,
    ]
}

fn round_end_cap(center: Vec2, direction: Vec2, radius: f32) -> Vec<Vec2> {
    let start = center + direction * radius;
    let mut cap = Vec::new();
    for segment in 1..=END_CAP_SEGMENTS {
        let t = segment as f32 / END_CAP_SEGMENTS as f32;
        cap.push((start - center).rotate(Vec2::from_angle(FIXED_PI * 3.0 * t)) + center);
    }
    cap
}

fn flat_end_cap(center: Vec2, direction: Vec2, radius: f32) -> Vec<Vec2> {
    vec![
        center + direction * radius,
        center + direction * (radius * FLAT_END_INNER_SCALE),
        center - direction * (radius * FLAT_END_INNER_SCALE),
        center - direction * radius,
    ]
}
