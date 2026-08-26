use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use num_traits::ToPrimitive;
use shrimply_math_core::Fraction;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    TextInterpolation, Time, TimelineCurveKeyframe, TimelineKeyframe, TimelineScalar,
    TimelineTextKeyframe, TimelineValueType, TimelineVector, fraction_denominator,
    fraction_numerator,
};

pub(crate) fn scalar_value_at<T: TimelineScalar>(
    keyframes: &[TimelineCurveKeyframe<T>],
    time: Time,
) -> T {
    let Some((left, right, progress)) = keyframe_segment::<T>(keyframes, time) else {
        return endpoint_value::<T>(keyframes, time);
    };
    let left_value = left.value.to_f64();
    let right_value = right.value.to_f64();
    let progress = left.interpolation_to_next.value(progress);
    T::from_f64(left_value + (right_value - left_value) * progress)
}

pub fn vector_value_at<T: TimelineVector>(keyframes: &[TimelineCurveKeyframe<T>], time: Time) -> T {
    let Some((left, right, progress)) = keyframe_segment::<T>(keyframes, time) else {
        return endpoint_value::<T>(keyframes, time);
    };
    T::mix(
        &left.value,
        &right.value,
        left.interpolation_to_next.value(progress),
    )
}

pub(crate) fn text_value_at(keyframes: &[TimelineTextKeyframe], time: Time) -> String {
    let Some((left, right, progress)) = keyframe_segment::<String>(keyframes, time) else {
        return endpoint_value::<String>(keyframes, time);
    };
    let progress = left.interpolation_to_next.value(progress).clamp(0.0, 1.0);
    interpolate_text(
        &left.value,
        &right.value,
        left.text_interpolation_to_next,
        progress,
        (
            fraction_numerator(time.seconds),
            fraction_denominator(time.seconds),
        ),
    )
}

pub fn text_edit_count(from: &str, to: &str, mode: TextInterpolation) -> usize {
    if mode == TextInterpolation::Jump || (from == to && mode != TextInterpolation::Decode) {
        return 0;
    }
    let from = graphemes(from);
    let to = graphemes(to);
    if mode == TextInterpolation::Decode {
        return from.len() + from.len().abs_diff(to.len()) + to.len();
    }
    if mode == TextInterpolation::Diff {
        return text_diff(&from, &to)
            .iter()
            .filter(|operation| operation.is_edit())
            .count();
    }
    let prefix = common_prefix_len(&from, &to);
    let suffix = match mode {
        TextInterpolation::Insert => common_suffix_len(&from[prefix..], &to[prefix..]),
        TextInterpolation::Jump
        | TextInterpolation::Type
        | TextInterpolation::Append
        | TextInterpolation::Diff
        | TextInterpolation::Decode => 0,
    };
    let prefix = if mode == TextInterpolation::Type {
        0
    } else {
        prefix
    };
    from.len() + to.len() - prefix.saturating_mul(2) - suffix.saturating_mul(2)
}

fn interpolate_text(
    from: &str,
    to: &str,
    mode: TextInterpolation,
    progress: f64,
    decode_frame: (i64, i64),
) -> String {
    if from == to && mode != TextInterpolation::Decode {
        return to.to_string();
    }
    if mode == TextInterpolation::Jump {
        return if progress < 1.0 { from } else { to }.to_string();
    }

    let from = graphemes(from);
    let to = graphemes(to);
    if mode == TextInterpolation::Decode {
        return decode_text(&from, &to, progress, decode_frame);
    }
    if mode == TextInterpolation::Diff {
        return diff_text(&from, &to, progress);
    }
    let prefix = if mode == TextInterpolation::Type {
        0
    } else {
        common_prefix_len(&from, &to)
    };
    let suffix = if mode == TextInterpolation::Insert {
        common_suffix_len(&from[prefix..], &to[prefix..])
    } else {
        0
    };
    edit_text(&from, &to, prefix, suffix, progress)
}

const DECODE_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const SCRAMBLE_PHASE_END: f64 = 1.0;
const RESIZE_PHASE_END: f64 = 2.0;
const DECODE_PHASE_END: f64 = 3.0;

fn decode_text(from: &[&str], to: &[&str], progress: f64, frame: (i64, i64)) -> String {
    let phase = progress * DECODE_PHASE_END;
    let mut output = String::new();
    if phase < SCRAMBLE_PHASE_END {
        let scrambled = (phase * from.len() as f64).floor() as usize;
        for (index, value) in from.iter().enumerate() {
            if index < scrambled {
                output.push(decode_letter(from, to, index, frame));
            } else {
                output.push_str(value);
            }
        }
        return output;
    }
    if phase < RESIZE_PHASE_END {
        let resize_progress = phase - SCRAMBLE_PHASE_END;
        let length = if to.len() >= from.len() {
            from.len() + (resize_progress * to.len().abs_diff(from.len()) as f64).floor() as usize
        } else {
            from.len() - (resize_progress * from.len().abs_diff(to.len()) as f64).floor() as usize
        };
        for index in 0..length {
            output.push(decode_letter(from, to, index, frame));
        }
        return output;
    }

    let revealed = if phase >= DECODE_PHASE_END {
        to.len()
    } else {
        ((phase - RESIZE_PHASE_END) * to.len() as f64).floor() as usize
    };
    for (index, value) in to.iter().enumerate() {
        if index < revealed {
            output.push_str(value);
        } else {
            output.push(decode_letter(from, to, index, frame));
        }
    }
    output
}

fn decode_letter(from: &[&str], to: &[&str], index: usize, frame: (i64, i64)) -> char {
    let mut hasher = DefaultHasher::new();
    from.hash(&mut hasher);
    to.hash(&mut hasher);
    index.hash(&mut hasher);
    frame.hash(&mut hasher);
    DECODE_ALPHABET[hasher.finish() as usize % DECODE_ALPHABET.len()] as char
}

#[derive(Clone, Copy)]
enum TextDiff<'a> {
    Equal(&'a str),
    Delete(&'a str),
    Insert(&'a str),
}

impl TextDiff<'_> {
    fn is_edit(self) -> bool {
        !matches!(self, Self::Equal(_))
    }
}

fn text_diff<'a>(from: &[&'a str], to: &[&'a str]) -> Vec<TextDiff<'a>> {
    let width = to.len() + 1;
    let cells = (from.len() + 1)
        .checked_mul(width)
        .expect("text diff is too large");
    let mut lengths = vec![0usize; cells];
    for from_index in (0..from.len()).rev() {
        for to_index in (0..to.len()).rev() {
            lengths[from_index * width + to_index] = if from[from_index] == to[to_index] {
                lengths[(from_index + 1) * width + to_index + 1] + 1
            } else {
                lengths[(from_index + 1) * width + to_index]
                    .max(lengths[from_index * width + to_index + 1])
            };
        }
    }

    let mut operations = Vec::with_capacity(from.len() + to.len());
    let (mut from_index, mut to_index) = (0, 0);
    while from_index < from.len() && to_index < to.len() {
        if from[from_index] == to[to_index] {
            operations.push(TextDiff::Equal(from[from_index]));
            from_index += 1;
            to_index += 1;
        } else if lengths[(from_index + 1) * width + to_index]
            >= lengths[from_index * width + to_index + 1]
        {
            operations.push(TextDiff::Delete(from[from_index]));
            from_index += 1;
        } else {
            operations.push(TextDiff::Insert(to[to_index]));
            to_index += 1;
        }
    }
    operations.extend(
        from[from_index..]
            .iter()
            .map(|value| TextDiff::Delete(value)),
    );
    operations.extend(to[to_index..].iter().map(|value| TextDiff::Insert(value)));
    operations
}

fn diff_text(from: &[&str], to: &[&str], progress: f64) -> String {
    let operations = text_diff(from, to);
    let total = operations
        .iter()
        .filter(|operation| operation.is_edit())
        .count();
    let completed = if progress >= 1.0 {
        total
    } else {
        (progress * total as f64).floor() as usize
    };
    let mut edit_index = 0;
    operations
        .into_iter()
        .filter_map(|operation| match operation {
            TextDiff::Equal(value) => Some(value),
            TextDiff::Delete(value) => {
                let visible = edit_index >= completed;
                edit_index += 1;
                visible.then_some(value)
            }
            TextDiff::Insert(value) => {
                let visible = edit_index < completed;
                edit_index += 1;
                visible.then_some(value)
            }
        })
        .collect()
}

fn graphemes(value: &str) -> Vec<&str> {
    UnicodeSegmentation::graphemes(value, true).collect()
}

fn common_prefix_len(left: &[&str], right: &[&str]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_suffix_len(left: &[&str], right: &[&str]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

fn edit_text(from: &[&str], to: &[&str], prefix: usize, suffix: usize, progress: f64) -> String {
    let from_middle_end = from.len().saturating_sub(suffix);
    let to_middle_end = to.len().saturating_sub(suffix);
    let removed = from_middle_end.saturating_sub(prefix);
    let inserted = to_middle_end.saturating_sub(prefix);
    let total = removed + inserted;
    if total == 0 {
        return to.concat();
    }
    let completed = if progress >= 1.0 {
        total
    } else {
        (progress * total as f64).floor() as usize
    };
    let remaining = removed.saturating_sub(completed.min(removed));
    let inserted = completed.saturating_sub(removed).min(inserted);
    from[..prefix]
        .iter()
        .chain(&from[prefix..prefix + remaining])
        .chain(&to[prefix..prefix + inserted])
        .chain(&to[to_middle_end..])
        .copied()
        .collect()
}

fn keyframe_segment<T: TimelineValueType>(
    keyframes: &[T::Keyframe],
    time: Time,
) -> Option<(&T::Keyframe, &T::Keyframe, f64)> {
    let first = keyframes.first()?;
    let last = keyframes.last()?;
    if time <= first.time() || time >= last.time() {
        return None;
    }
    let pair = keyframes
        .windows(2)
        .find(|pair| time >= pair[0].time() && time <= pair[1].time())?;
    let span = pair[1].time().seconds - pair[0].time().seconds;
    if span <= Fraction::from(0u8) {
        return Some((&pair[0], &pair[1], 1.0));
    }
    let progress = ((time.seconds - pair[0].time().seconds) / span)
        .max(Fraction::from(0u8))
        .min(Fraction::from(1u8))
        .to_f64()
        .unwrap_or_default();
    Some((&pair[0], &pair[1], progress))
}

fn endpoint_value<T: TimelineValueType>(keyframes: &[T::Keyframe], time: Time) -> T {
    let Some(first) = keyframes.first() else {
        return T::default_value();
    };
    if time <= first.time() {
        return first.value().clone();
    }
    keyframes
        .last()
        .map(|keyframe| keyframe.value().clone())
        .unwrap_or_else(T::default_value)
}
