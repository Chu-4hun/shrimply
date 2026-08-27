use glam::{Mat3, Vec2};
use num_traits::ToPrimitive;
use shrimply_interpolation::Interpolation;
use shrimply_math_core::{Fraction, fraction_denominator, fraction_numerator};
use shrimply_math_geometry::Rect;
use shrimply_project::Time;

const FULL_TURN_DEGREES: f32 = 360.0;
const PERCENT: f32 = 100.0;
const AXIS_ALIGNMENT_EPSILON: f32 = 0.001;

#[derive(Clone, Copy, Debug)]
pub struct Keyframe<T> {
    pub frame: i64,
    pub value: T,
    pub interpolation: Interpolation,
}

#[derive(Clone, Copy, Debug)]
pub struct RectValue {
    pub rect: Rect,
    pub opacity: f32,
}

pub fn time(frame: i64, fps: Fraction) -> Time {
    let magnitude = Fraction::from(frame.unsigned_abs()) / fps;
    Time {
        seconds: if frame < 0 { -magnitude } else { magnitude },
    }
}

pub fn ceil_positive_fraction(value: Fraction) -> Option<i64> {
    let numerator = fraction_numerator(value);
    let denominator = fraction_denominator(value);
    if numerator <= 0 || denominator <= 0 {
        return None;
    }
    numerator
        .checked_add(denominator - 1)?
        .checked_div(denominator)
}

pub fn fit_durations(first: &mut Time, second: &mut Time, available: Time) {
    let total = first.seconds + second.seconds;
    if total <= available.seconds || total == Fraction::from(0_u64) {
        return;
    }
    first.seconds = available.seconds * first.seconds / total;
    second.seconds = available.seconds - first.seconds;
}

pub fn fit_scale(container: glam::Vec2, content: glam::Vec2) -> f32 {
    (container / content.max(glam::Vec2::ONE)).min_element()
}

pub fn crop_percentages(crop: Rect, bounds: Rect) -> [f32; 4] {
    let width = bounds.width().max(f32::EPSILON);
    let height = bounds.height().max(f32::EPSILON);
    [
        ((crop.top() - bounds.top()) / height * PERCENT).clamp(0.0, PERCENT),
        ((bounds.right() - crop.right()) / width * PERCENT).clamp(0.0, PERCENT),
        ((bounds.bottom() - crop.bottom()) / height * PERCENT).clamp(0.0, PERCENT),
        ((crop.left() - bounds.left()) / width * PERCENT).clamp(0.0, PERCENT),
    ]
}

pub fn transformed_crop_percentages(
    crop: Rect,
    source_to_canvas: Mat3,
    source_size: Vec2,
) -> Option<([f32; 4], bool)> {
    let determinant = source_to_canvas.determinant();
    if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
        return None;
    }
    let canvas_to_source = source_to_canvas.inverse();
    let corners = [
        canvas_to_source.transform_point2(crop.min),
        canvas_to_source.transform_point2(Vec2::new(crop.max.x, crop.min.y)),
        canvas_to_source.transform_point2(crop.max),
        canvas_to_source.transform_point2(Vec2::new(crop.min.x, crop.max.y)),
    ];
    let min = corners
        .iter()
        .copied()
        .fold(Vec2::splat(f32::INFINITY), Vec2::min);
    let max = corners
        .iter()
        .copied()
        .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
    let axis_aligned = corners.iter().all(|corner| {
        ((corner.x - min.x).abs() <= AXIS_ALIGNMENT_EPSILON
            || (corner.x - max.x).abs() <= AXIS_ALIGNMENT_EPSILON)
            && ((corner.y - min.y).abs() <= AXIS_ALIGNMENT_EPSILON
                || (corner.y - max.y).abs() <= AXIS_ALIGNMENT_EPSILON)
    });
    Some((
        crop_percentages(
            Rect::from_min_max(min, max),
            Rect::from_xywh(0.0, 0.0, source_size.x, source_size.y),
        ),
        axis_aligned,
    ))
}

pub fn relative_crop(existing: [f32; 4], requested: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let combined = std::array::from_fn(|index| existing[index].max(requested[index]));
    let vertical = (PERCENT - existing[0] - existing[2]).max(f32::EPSILON);
    let horizontal = (PERCENT - existing[1] - existing[3]).max(f32::EPSILON);
    (
        [
            ((combined[0] - existing[0]) / vertical * PERCENT).clamp(0.0, PERCENT),
            ((combined[1] - existing[1]) / horizontal * PERCENT).clamp(0.0, PERCENT),
            ((combined[2] - existing[2]) / vertical * PERCENT).clamp(0.0, PERCENT),
            ((combined[3] - existing[3]) / horizontal * PERCENT).clamp(0.0, PERCENT),
        ],
        combined,
    )
}

pub fn qtblend_transform(
    input_size: Vec2,
    rect: Rect,
    rotation_degrees: f32,
    distort: bool,
    rotation_anchor: Vec2,
) -> Mat3 {
    let requested_scale = rect.size() / input_size.max(Vec2::ONE);
    let scale = if distort {
        requested_scale
    } else {
        Vec2::splat(requested_scale.min_element())
    };
    let rotation_anchor = rect.min + rect.size() * rotation_anchor;
    Mat3::from_translation(rotation_anchor)
        * Mat3::from_angle(rotation_degrees.to_radians())
        * Mat3::from_translation(-rotation_anchor)
        * Mat3::from_translation(rect.center())
        * Mat3::from_scale(scale)
        * Mat3::from_translation(input_size * -0.5)
}

pub fn equivalent_angle_near(angle: f32, reference: f32) -> f32 {
    angle + ((reference - angle) / FULL_TURN_DEGREES).round() * FULL_TURN_DEGREES
}

pub fn frei0r_parameter_index(value: f32, option_count: u32) -> u32 {
    assert!(option_count > 0, "frei0r parameters require an option");
    ((value.clamp(0.0, 1.0) * option_count as f32).floor() as u32).min(option_count - 1)
}

pub fn frei0r_tilt_degrees(value: f32) -> f32 {
    (value - 0.5) * FULL_TURN_DEGREES
}

pub fn scalar_animation(value: &str, fps: Fraction) -> Result<Vec<Keyframe<f32>>, String> {
    if !value.contains('=') {
        return Ok(vec![Keyframe {
            frame: 0,
            value: value
                .trim()
                .parse()
                .map_err(|error| format!("invalid animated number: {error}"))?,
            interpolation: Interpolation::Linear,
        }]);
    }
    animation(value, fps, |value| {
        value
            .trim()
            .parse()
            .map_err(|error| format!("invalid animated number: {error}"))
    })
}

pub fn scalar_pair_animation(
    first: &str,
    second: &str,
    fps: Fraction,
) -> Result<Vec<Keyframe<Vec2>>, String> {
    let first = scalar_animation(first, fps)?;
    let second = scalar_animation(second, fps)?;
    let mut frames = first
        .iter()
        .chain(&second)
        .map(|keyframe| keyframe.frame)
        .collect::<Vec<_>>();
    frames.sort_unstable();
    frames.dedup();
    Ok(frames
        .into_iter()
        .map(|frame| {
            let x = value_at(&first, frame, |from, to, progress| {
                from + (to - from) * progress
            });
            let y = value_at(&second, frame, |from, to, progress| {
                from + (to - from) * progress
            });
            let interpolation = first
                .iter()
                .find(|keyframe| keyframe.frame == frame)
                .or_else(|| second.iter().find(|keyframe| keyframe.frame == frame))
                .expect("merged animation frame must belong to an input")
                .interpolation;
            Keyframe {
                frame,
                value: Vec2::new(x, y),
                interpolation,
            }
        })
        .collect())
}

pub fn point_animation(value: &str, fps: Fraction) -> Result<Vec<Keyframe<Vec2>>, String> {
    let parse = |value: &str| {
        let values = value.split_whitespace().collect::<Vec<_>>();
        if values.len() < 2 {
            return Err("animated point has fewer than two values".to_owned());
        }
        Ok(Vec2::new(
            values[0]
                .parse()
                .map_err(|error| format!("invalid animated point: {error}"))?,
            values[1]
                .parse()
                .map_err(|error| format!("invalid animated point: {error}"))?,
        ))
    };
    if !value.contains('=') {
        return Ok(vec![Keyframe {
            frame: 0,
            value: parse(value)?,
            interpolation: Interpolation::Linear,
        }]);
    }
    animation(value, fps, parse)
}

pub fn rect_animation(
    value: &str,
    fps: Fraction,
    frame_size: Vec2,
) -> Result<Vec<Keyframe<RectValue>>, String> {
    if !value.contains('=') {
        return Ok(vec![Keyframe {
            frame: 0,
            value: parse_rect(value, frame_size)?,
            interpolation: Interpolation::Linear,
        }]);
    }
    animation(value, fps, |value| parse_rect(value, frame_size))
}

fn parse_rect(value: &str, frame_size: Vec2) -> Result<RectValue, String> {
    let geometry = if value.split_whitespace().count() >= 4 {
        value.to_owned()
    } else {
        value.replace(['/', ':', 'x'], " ")
    };
    let values = geometry.split_whitespace().collect::<Vec<_>>();
    if values.len() < 4 {
        return Err("animated rectangle has fewer than four values".to_owned());
    }
    let dimension = |value: &str, scale: f32| {
        value
            .strip_suffix('%')
            .map_or_else(
                || value.parse::<f32>(),
                |value| value.parse::<f32>().map(|value| value / PERCENT * scale),
            )
            .map_err(|error| format!("invalid animated rectangle: {error}"))
    };
    Ok(RectValue {
        rect: Rect::from_xywh(
            dimension(values[0], frame_size.x)?,
            dimension(values[1], frame_size.y)?,
            dimension(values[2], frame_size.x)?,
            dimension(values[3], frame_size.y)?,
        ),
        opacity: values
            .get(4)
            .map_or(Ok(1.0), |value| value.parse::<f32>())
            .map_err(|error| format!("invalid animated rectangle opacity: {error}"))?,
    })
}

pub fn value_at<T: Copy>(
    keyframes: &[Keyframe<T>],
    frame: i64,
    lerp: impl Fn(T, T, f32) -> T,
) -> T {
    let next = keyframes.partition_point(|keyframe| keyframe.frame <= frame);
    if next == 0 {
        return keyframes[0].value;
    }
    if next == keyframes.len() {
        return keyframes[next - 1].value;
    }
    let from = keyframes[next - 1];
    let to = keyframes[next];
    let span = (to.frame - from.frame).max(1) as f64;
    let progress = (frame - from.frame) as f64 / span;
    lerp(
        from.value,
        to.value,
        from.interpolation.value(progress) as f32,
    )
}

fn animation<T>(
    value: &str,
    fps: Fraction,
    parse_value: impl Fn(&str) -> Result<T, String>,
) -> Result<Vec<Keyframe<T>>, String> {
    let mut keyframes = Vec::new();
    for part in value.split(';').filter(|part| !part.trim().is_empty()) {
        let (marker, value) = part
            .split_once('=')
            .ok_or_else(|| "animation keyframe has no value".to_owned())?;
        let (time, interpolation) = marker_interpolation(marker.trim());
        keyframes.push(Keyframe {
            frame: parse_frame(time, fps)?,
            value: parse_value(value)?,
            interpolation,
        });
    }
    keyframes.sort_by_key(|keyframe| keyframe.frame);
    if keyframes.is_empty() {
        return Err("animation has no keyframes".to_owned());
    }
    Ok(keyframes)
}

pub fn parse_frame(value: &str, fps: Fraction) -> Result<i64, String> {
    if !value.contains(':') {
        return value
            .parse()
            .map_err(|error| format!("invalid animation frame: {error}"));
    }
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 3 {
        return Err("invalid animation timestamp".to_owned());
    }
    let hours = fields[0]
        .parse::<u64>()
        .map_err(|error| format!("invalid animation hour: {error}"))?;
    let minutes = fields[1]
        .parse::<u64>()
        .map_err(|error| format!("invalid animation minute: {error}"))?;
    let seconds = decimal_fraction(fields[2])?;
    let total = Fraction::from(hours * 3600 + minutes * 60) + seconds;
    (total * fps)
        .to_f64()
        .map(|frames| frames.round() as i64)
        .ok_or_else(|| "animation timestamp is not finite".to_owned())
}

fn decimal_fraction(value: &str) -> Result<Fraction, String> {
    if let Some((whole, fractional)) = value.split_once('.') {
        let denominator = 10_u64.pow(fractional.len() as u32);
        let numerator = whole
            .parse::<u64>()
            .map_err(|error| format!("invalid timestamp seconds: {error}"))?
            * denominator
            + fractional
                .parse::<u64>()
                .map_err(|error| format!("invalid timestamp fraction: {error}"))?;
        Ok(Fraction::new(numerator, denominator))
    } else {
        value
            .parse::<u64>()
            .map(Fraction::from)
            .map_err(|error| format!("invalid timestamp seconds: {error}"))
    }
}

fn marker_interpolation(marker: &str) -> (&str, Interpolation) {
    let Some(last) = marker.chars().last() else {
        return (marker, Interpolation::Linear);
    };
    let interpolation = match last {
        '|' | '!' => Interpolation::Jump,
        '~' | '$' | '-' => Interpolation::ManimSmooth,
        'a' => Interpolation::SineIn,
        'b' => Interpolation::SineOut,
        'c' => Interpolation::SineInOut,
        'd' => Interpolation::QuadIn,
        'e' => Interpolation::QuadOut,
        'f' => Interpolation::QuadInOut,
        'g' => Interpolation::CubicIn,
        'h' => Interpolation::CubicOut,
        'i' => Interpolation::CubicInOut,
        'j' => Interpolation::QuartIn,
        'k' => Interpolation::QuartOut,
        'l' => Interpolation::QuartInOut,
        'm' => Interpolation::QuintIn,
        'n' => Interpolation::QuintOut,
        'o' => Interpolation::QuintInOut,
        'p' => Interpolation::ExpoIn,
        'q' => Interpolation::ExpoOut,
        'r' => Interpolation::ExpoInOut,
        's' => Interpolation::CircIn,
        't' => Interpolation::CircOut,
        'u' => Interpolation::CircInOut,
        'v' => Interpolation::BackIn,
        'w' => Interpolation::BackOut,
        'x' => Interpolation::BackInOut,
        'y' => Interpolation::ElasticIn,
        'z' => Interpolation::ElasticOut,
        'A' => Interpolation::ElasticInOut,
        'B' => Interpolation::BounceIn,
        'C' => Interpolation::BounceOut,
        'D' => Interpolation::BounceInOut,
        _ => return (marker, Interpolation::Linear),
    };
    (&marker[..marker.len() - last.len_utf8()], interpolation)
}
