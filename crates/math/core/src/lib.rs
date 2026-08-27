use fraction::ToPrimitive;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

pub use fraction::{Fraction, GenericFraction, Ratio, Sign};

pub const FRACTION_ZERO: Fraction =
    GenericFraction::Rational(Sign::Plus, fraction::Ratio::new_raw(0, 1));
const FRAME_RATE_DECIMAL_SCALE: f64 = 1_000.0;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Time {
    #[serde(
        deserialize_with = "deserialize_fraction",
        serialize_with = "serialize_fraction"
    )]
    pub seconds: Fraction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmpteTimecode {
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub frames: i64,
    pub frame_separator: char,
    pub frame_width: usize,
}

pub fn smpte_timecode(frame: i64, frame_rate: Fraction, drop_frame: bool) -> Option<SmpteTimecode> {
    if frame < 0 {
        return None;
    }
    let rate_numerator = i128::from(fraction_numerator(frame_rate));
    let rate_denominator = i128::from(fraction_denominator(frame_rate));
    if rate_numerator <= 0 || rate_denominator <= 0 {
        return None;
    }

    let mut frame = i128::from(frame);
    let (rate_numerator, rate_denominator, frame_separator) =
        if rate_numerator * 1_001 == rate_denominator * 30_000 {
            if drop_frame {
                frame = drop_frame_number(frame, 30, 2)?;
            }
            (30, 1, if drop_frame { ';' } else { ':' })
        } else if rate_numerator * 1_001 == rate_denominator * 60_000 {
            if drop_frame {
                frame = drop_frame_number(frame, 60, 4)?;
            }
            (60, 1, if drop_frame { ';' } else { ':' })
        } else if drop_frame {
            (
                rate_numerator,
                rate_denominator,
                if rate_numerator % rate_denominator == 0 {
                    ':'
                } else {
                    ';'
                },
            )
        } else {
            (
                round_ratio_ties_even(rate_numerator, rate_denominator)?,
                1,
                ':',
            )
        };
    if rate_numerator <= 0 {
        return None;
    }

    let hours = frame
        .checked_mul(rate_denominator)?
        .checked_div(rate_numerator.checked_mul(3_600)?)?;
    frame = frame.checked_sub(
        hours
            .checked_mul(3_600)?
            .checked_mul(rate_numerator)?
            .checked_div(rate_denominator)?,
    )?;
    let minutes = frame
        .checked_mul(rate_denominator)?
        .checked_div(rate_numerator.checked_mul(60)?)?;
    frame = frame.checked_sub(
        minutes
            .checked_mul(60)?
            .checked_mul(rate_numerator)?
            .checked_div(rate_denominator)?,
    )?;
    let seconds = frame
        .checked_mul(rate_denominator)?
        .checked_div(rate_numerator)?;
    let second_frames = seconds.checked_mul(rate_numerator)?;
    let second_frames = second_frames
        .checked_add(rate_denominator - 1)?
        .checked_div(rate_denominator)?;
    frame = frame.checked_sub(second_frames)?;

    Some(SmpteTimecode {
        hours: i64::try_from(hours).ok()?,
        minutes: i64::try_from(minutes).ok()?,
        seconds: i64::try_from(seconds).ok()?,
        frames: i64::try_from(frame).ok()?,
        frame_separator,
        frame_width: if rate_numerator > rate_denominator * 999 {
            4
        } else if rate_numerator > rate_denominator * 99 {
            3
        } else {
            2
        },
    })
}

pub fn format_smpte_timecode(timecode: SmpteTimecode) -> String {
    format!(
        "{:02}:{:02}:{:02}{}{:0width$}",
        timecode.hours,
        timecode.minutes,
        timecode.seconds,
        timecode.frame_separator,
        timecode.frames,
        width = timecode.frame_width,
    )
}

fn drop_frame_number(frame: i128, nominal_rate: i128, dropped_frames: i128) -> Option<i128> {
    let frames_per_minute = nominal_rate.checked_mul(60)?.checked_sub(dropped_frames)?;
    let frames_per_ten_minutes = nominal_rate
        .checked_mul(600)?
        .checked_sub(dropped_frames.checked_mul(9)?)?;
    let ten_minute_blocks = frame.checked_div(frames_per_ten_minutes)?;
    let remaining = frame.checked_rem(frames_per_ten_minutes)?;
    let extra_minutes = remaining
        .checked_sub(dropped_frames)
        .map_or(0, |remaining| remaining / frames_per_minute);
    frame
        .checked_add(ten_minute_blocks.checked_mul(dropped_frames.checked_mul(9)?)?)?
        .checked_add(extra_minutes.checked_mul(dropped_frames)?)
}

fn round_ratio_ties_even(numerator: i128, denominator: i128) -> Option<i128> {
    let quotient = numerator.checked_div(denominator)?;
    let remainder = numerator.checked_rem(denominator)?;
    match remainder.checked_mul(2)?.cmp(&denominator) {
        std::cmp::Ordering::Less => Some(quotient),
        std::cmp::Ordering::Greater => quotient.checked_add(1),
        std::cmp::Ordering::Equal if quotient % 2 == 0 => Some(quotient),
        std::cmp::Ordering::Equal => quotient.checked_add(1),
    }
}

#[derive(Serialize, Deserialize)]
struct RawFraction {
    numerator: i64,
    denominator: i64,
}

pub fn deserialize_fraction<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Fraction, D::Error> {
    let raw = RawFraction::deserialize(deserializer)?;
    if raw.denominator <= 0 {
        return Err(D::Error::custom("fraction denominator must be positive"));
    }
    Ok(fraction_new(raw.numerator, raw.denominator))
}

pub fn serialize_fraction<S: Serializer>(
    value: &Fraction,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    RawFraction {
        numerator: fraction_numerator(*value),
        denominator: fraction_denominator(*value),
    }
    .serialize(serializer)
}

pub fn fraction_new(numerator: i64, denominator: i64) -> Fraction {
    if denominator == 0 {
        return FRACTION_ZERO;
    }
    if numerator < 0 {
        GenericFraction::Rational(
            Sign::Minus,
            fraction::Ratio::new(numerator.unsigned_abs(), denominator.unsigned_abs()),
        )
    } else {
        GenericFraction::Rational(
            Sign::Plus,
            fraction::Ratio::new(numerator as u64, denominator.unsigned_abs()),
        )
    }
}

pub fn fraction_from_integer(value: i64) -> Fraction {
    fraction_new(value, 1)
}

pub fn fraction_from_f64(value: f64) -> Fraction {
    if !value.is_finite() {
        return FRACTION_ZERO;
    }
    let mut value = format!("{value:.12}");
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    Fraction::from_str(&value).unwrap_or(FRACTION_ZERO)
}

pub fn frame_rate_from_f64(value: f64) -> Option<Fraction> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let numerator = (value * FRAME_RATE_DECIMAL_SCALE).round();
    if !(1.0..=f64::from(u32::MAX)).contains(&numerator) {
        return None;
    }
    Some(fraction_new(
        numerator as i64,
        FRAME_RATE_DECIMAL_SCALE as i64,
    ))
}

pub fn fraction_as_u32_ratio(value: Fraction) -> Option<(u32, u32)> {
    let GenericFraction::Rational(Sign::Plus, ratio) = value else {
        return None;
    };
    Some((
        u32::try_from(*ratio.numer()).ok()?,
        u32::try_from(*ratio.denom()).ok()?,
    ))
}

pub fn fraction_snapped(value: f64, minimum: Fraction, step: Fraction) -> Fraction {
    let step_value = fraction_as_f64(step);
    assert!(step_value > 0.0, "fraction snap step must be positive");
    let ticks = ((value - fraction_as_f64(minimum)) / step_value)
        .round()
        .to_i64()
        .expect("fraction snap exceeds i64");
    minimum + step * fraction_from_integer(ticks)
}

pub fn fraction_is_finite(value: Fraction) -> bool {
    value.to_f64().is_some_and(f64::is_finite)
}

pub fn fraction_as_f64(value: Fraction) -> f64 {
    value.to_f64().unwrap_or(0.0)
}

pub fn fraction_as_label(value: Fraction) -> String {
    value.to_string()
}

pub fn fraction_numerator(value: Fraction) -> i64 {
    match value {
        GenericFraction::Rational(sign, ratio) => {
            let numerator = i64::try_from(*ratio.numer()).unwrap_or(i64::MAX);
            if sign == Sign::Minus {
                -numerator
            } else {
                numerator
            }
        }
        _ => 0,
    }
}

pub fn fraction_denominator(value: Fraction) -> i64 {
    value
        .denom()
        .and_then(|value| i64::try_from(*value).ok())
        .unwrap_or(1)
}

pub fn fraction_round_nonnegative_u64(value: Fraction) -> u64 {
    let GenericFraction::Rational(Sign::Plus, ratio) = value else {
        return 0;
    };
    let numerator = u128::from(*ratio.numer());
    let denominator = u128::from(*ratio.denom()).max(1);
    numerator
        .saturating_add(denominator / 2)
        .checked_div(denominator)
        .unwrap_or(0)
        .min(u128::from(u64::MAX)) as u64
}

pub fn frame_rate_from_duration(duration: Duration) -> Option<Fraction> {
    let nanoseconds = duration.as_nanos();
    if nanoseconds == 0 {
        return None;
    }
    Some(fraction_new(
        1_000_000_000,
        nanoseconds.min(i64::MAX as u128) as i64,
    ))
}

/// Converts a zero-based frame to an exact time using checked rational math.
pub fn time_from_frame(frame: u64, frame_rate: Fraction) -> Option<Time> {
    let GenericFraction::Rational(Sign::Plus, rate) = frame_rate else {
        return None;
    };
    if *rate.numer() == 0 {
        return None;
    }
    let numerator = u128::from(frame).checked_mul(u128::from(*rate.denom()))?;
    let denominator = u128::from(*rate.numer());
    let divisor = gcd_u128(numerator, denominator);
    let numerator = numerator / divisor;
    let denominator = denominator / divisor;
    Some(Time::from_fraction(
        i64::try_from(numerator).ok()?,
        i64::try_from(denominator).ok()?,
    ))
}

pub fn fit_nonnegative_fraction_pair(
    available: Fraction,
    first: Fraction,
    second: Fraction,
) -> (Fraction, Fraction) {
    let sum = first + second;
    if sum <= available || sum == FRACTION_ZERO {
        return (first, second);
    }
    let fitted_first = available * first / sum;
    (fitted_first, available - fitted_first)
}

fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left.max(1)
}

impl Time {
    pub const ZERO: Self = Self {
        seconds: FRACTION_ZERO,
    };

    pub fn from_seconds_f64(seconds: f64) -> Self {
        if !seconds.is_finite() {
            return Self::ZERO;
        }
        let nanos = (seconds * 1_000_000_000.0).round();
        Self::from_nanos_i128(nanos.clamp(i128::MIN as f64, i128::MAX as f64) as i128)
    }

    pub fn from_seconds(value: i64) -> Self {
        Self {
            seconds: fraction_from_integer(value),
        }
    }

    pub fn from_seconds_u64(value: u64) -> Self {
        Self {
            seconds: Fraction::from(value),
        }
    }

    pub fn from_fraction(numerator: i64, denominator: i64) -> Self {
        Self {
            seconds: fraction_new(numerator, denominator),
        }
    }

    pub fn from_nanos(nanos: u64) -> Self {
        Self {
            seconds: fraction_new(nanos.min(i64::MAX as u64) as i64, 1_000_000_000),
        }
    }

    pub fn from_nanos_i128(nanos: i128) -> Self {
        Self {
            seconds: fraction_new(
                nanos.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                1_000_000_000,
            ),
        }
    }

    pub fn as_secs_f64(self) -> f64 {
        fraction_as_f64(self.seconds)
    }

    pub fn as_nonnegative_nanos(self) -> u64 {
        match self.seconds {
            GenericFraction::Rational(Sign::Plus, ratio) => {
                ((*ratio.numer() as u128 * 1_000_000_000 / *ratio.denom() as u128)
                    .min(u64::MAX as u128)) as u64
            }
            _ => 0,
        }
    }

    pub fn as_nanos_i128(self) -> i128 {
        match self.seconds {
            GenericFraction::Rational(sign, ratio) => {
                let nanos = *ratio.numer() as i128 * 1_000_000_000 / *ratio.denom() as i128;
                if sign == Sign::Minus { -nanos } else { nanos }
            }
            _ => 0,
        }
    }

    pub fn as_sample_frame(self, sample_rate: u32) -> u64 {
        ((self.as_nonnegative_nanos() as u128 * sample_rate as u128 / 1_000_000_000)
            .min(u64::MAX as u128)) as u64
    }

    pub fn as_frame(self, frame_rate: Fraction) -> u64 {
        self.frame_at_rate(frame_rate, false)
    }

    pub fn as_frame_ceil(self, frame_rate: Fraction) -> u64 {
        self.frame_at_rate(frame_rate, true)
    }

    fn frame_at_rate(self, frame_rate: Fraction, round_up: bool) -> u64 {
        let GenericFraction::Rational(Sign::Plus, time) = self.seconds else {
            return 0;
        };
        let GenericFraction::Rational(Sign::Plus, rate) = frame_rate else {
            return 0;
        };
        let numerator = u128::from(*time.numer()).saturating_mul(u128::from(*rate.numer()));
        let denominator =
            u128::from(*time.denom()).saturating_mul(u128::from(*rate.denom()).max(1));
        let frames = if round_up {
            numerator.div_ceil(denominator)
        } else {
            numerator / denominator
        };
        frames.min(u128::from(u64::MAX)) as u64
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            seconds: self.seconds + other.seconds,
        }
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        if self <= other {
            Self::ZERO
        } else {
            Self {
                seconds: self.seconds - other.seconds,
            }
        }
    }

    pub fn signed_sub(self, other: Self) -> Self {
        Self {
            seconds: self.seconds - other.seconds,
        }
    }

    pub fn abs_diff(self, other: Self) -> Self {
        if self >= other {
            self.saturating_sub(other)
        } else {
            other.saturating_sub(self)
        }
    }

    pub fn snapped(self, step: Self) -> Self {
        let step = step.as_nanos_i128();
        if step <= 0 {
            return self;
        }
        Self::from_nanos_i128((self.as_nanos_i128() + step / 2).div_euclid(step) * step)
    }

    pub fn from_duration(duration: Duration) -> Self {
        Self::from_nanos_i128(duration.as_nanos().min(i128::MAX as u128) as i128)
    }

    pub fn approx_eq(self, other: Self) -> bool {
        let diff = if self >= other {
            self.seconds - other.seconds
        } else {
            other.seconds - self.seconds
        };
        diff <= fraction_new(1, 1_000_000_000)
    }

    pub fn clamp(self, min: Self, max: Self) -> Self {
        self.max(min).min(max)
    }

    pub fn as_label(self) -> String {
        format!("{}s", fraction_as_label(self.seconds))
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rate_is_the_reciprocal_of_the_latest_render_cost() {
        assert_eq!(
            frame_rate_from_duration(Duration::from_millis(8)),
            Some(Fraction::from(125)),
        );
        assert_eq!(
            fraction_round_nonnegative_u64(
                frame_rate_from_duration(Duration::from_nanos(16_666_667))
                    .expect("nonzero render cost must have a frame rate"),
            ),
            60,
        );
        assert_eq!(frame_rate_from_duration(Duration::ZERO), None);
    }
}
