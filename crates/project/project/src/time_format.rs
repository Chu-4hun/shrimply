use std::time::Duration;

use shrimply_math_core::Fraction;

use crate::project::{
    Time, fraction_as_f64, fraction_denominator, fraction_numerator, playback_speed_or_default,
};

pub fn human_duration(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs().max(1);
    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

pub fn project_duration(time: Time) -> String {
    format!("{:.2}s", time.as_secs_f64())
}

pub fn playback_time(time: Time) -> String {
    let centiseconds =
        shrimply_math_core::fraction_round_nonnegative_u64(time.seconds * Fraction::from(100_u64));
    let hours = centiseconds / 360_000;
    let minutes = (centiseconds / 6_000) % 60;
    let seconds = (centiseconds / 100) % 60;
    let subsecond = centiseconds % 100;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{subsecond:02}")
}

pub fn playback_speed_suffix(playback_speed: Fraction) -> String {
    let playback_speed = playback_speed_or_default(playback_speed);
    if fraction_numerator(playback_speed) == 1 && fraction_denominator(playback_speed) == 1 {
        String::new()
    } else {
        format!(" @ {:.0}x", fraction_as_f64(playback_speed))
    }
}

pub fn timeline_tick(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else {
        let total_seconds = seconds.round().max(0.0) as u64;
        let hours = total_seconds / 3600;
        let minutes = total_seconds / 60 % 60;
        let seconds = total_seconds % 60;
        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }
}
