use std::f64::consts::{FRAC_PI_2, LN_2, PI};

use crate::Interpolation;

const BACK: f64 = 1.70158;
const BACK_IN_OUT: f64 = BACK * 1.525;
const BOUNCE_SCALE: f64 = 7.5625;
const BOUNCE_DIVISOR: f64 = 2.75;
const BOUNCE_FIRST: f64 = 1.0 / BOUNCE_DIVISOR;
const BOUNCE_SECOND: f64 = 2.0 / BOUNCE_DIVISOR;
const BOUNCE_THIRD: f64 = 2.5 / BOUNCE_DIVISOR;
const BOUNCE_IN_FIRST: f64 = 1.0 - BOUNCE_THIRD;
const BOUNCE_IN_SECOND: f64 = 1.0 - BOUNCE_SECOND;
const BOUNCE_IN_THIRD: f64 = 1.0 - BOUNCE_FIRST;
const BOUNCE_IN_OUT_BREAKS: [f64; 6] = [
    BOUNCE_IN_FIRST * 0.5,
    BOUNCE_IN_SECOND * 0.5,
    BOUNCE_IN_THIRD * 0.5,
    (1.0 + BOUNCE_FIRST) * 0.5,
    (1.0 + BOUNCE_SECOND) * 0.5,
    (1.0 + BOUNCE_THIRD) * 0.5,
];
const BOUNCE_IN_BREAKS: [f64; 3] = [BOUNCE_IN_FIRST, BOUNCE_IN_SECOND, BOUNCE_IN_THIRD];
const BOUNCE_OUT_BREAKS: [f64; 3] = [BOUNCE_FIRST, BOUNCE_SECOND, BOUNCE_THIRD];
const JUMP_BREAKS: [f64; 1] = [1.0];

pub(crate) fn value(interpolation: Interpolation, progress: f64) -> f64 {
    use Interpolation::*;
    let x = progress.clamp(0.0, 1.0);
    match interpolation {
        Linear => x,
        ManimSmooth => {
            let error = sigmoid(-5.0);
            ((sigmoid(10.0 * (x - 0.5)) - error) / (1.0 - 2.0 * error)).clamp(0.0, 1.0)
        }
        SineIn => 1.0 - (x * FRAC_PI_2).cos(),
        SineOut => (x * FRAC_PI_2).sin(),
        SineInOut => -((PI * x).cos() - 1.0) * 0.5,
        QuadIn => x * x,
        QuadOut => 1.0 - (1.0 - x).powi(2),
        QuadInOut => power_in_out(x, 2),
        CubicIn => x.powi(3),
        CubicOut => 1.0 - (1.0 - x).powi(3),
        CubicInOut => power_in_out(x, 3),
        QuartIn => x.powi(4),
        QuartOut => 1.0 - (1.0 - x).powi(4),
        QuartInOut => power_in_out(x, 4),
        QuintIn => x.powi(5),
        QuintOut => 1.0 - (1.0 - x).powi(5),
        QuintInOut => power_in_out(x, 5),
        ExpoIn => {
            if x != 0.0 {
                2.0_f64.powf(10.0 * x - 10.0)
            } else {
                0.0
            }
        }
        ExpoOut => {
            if x != 1.0 {
                1.0 - 2.0_f64.powf(-10.0 * x)
            } else {
                1.0
            }
        }
        ExpoInOut => {
            if x == 0.0 || x == 1.0 {
                x
            } else if x < 0.5 {
                2.0_f64.powf(20.0 * x - 10.0) * 0.5
            } else {
                (2.0 - 2.0_f64.powf(-20.0 * x + 10.0)) * 0.5
            }
        }
        CircIn => 1.0 - (1.0 - x * x).max(0.0).sqrt(),
        CircOut => (1.0 - (x - 1.0).powi(2)).max(0.0).sqrt(),
        CircInOut => {
            if x < 0.5 {
                (1.0 - (1.0 - (2.0 * x).powi(2)).max(0.0).sqrt()) * 0.5
            } else {
                ((1.0 - (-2.0 * x + 2.0).powi(2)).max(0.0).sqrt() + 1.0) * 0.5
            }
        }
        BackIn => (BACK + 1.0) * x.powi(3) - BACK * x.powi(2),
        BackOut => 1.0 + (BACK + 1.0) * (x - 1.0).powi(3) + BACK * (x - 1.0).powi(2),
        BackInOut => {
            if x < 0.5 {
                (2.0 * x).powi(2) * ((BACK_IN_OUT + 1.0) * 2.0 * x - BACK_IN_OUT) * 0.5
            } else {
                ((2.0 * x - 2.0).powi(2) * ((BACK_IN_OUT + 1.0) * (2.0 * x - 2.0) + BACK_IN_OUT)
                    + 2.0)
                    * 0.5
            }
        }
        ElasticIn => {
            if x == 0.0 || x == 1.0 {
                x
            } else {
                -2.0_f64.powf(10.0 * x - 10.0) * ((10.0 * x - 10.75) * (2.0 * PI / 3.0)).sin()
            }
        }
        ElasticOut => {
            if x == 0.0 || x == 1.0 {
                x
            } else {
                2.0_f64.powf(-10.0 * x) * ((10.0 * x - 0.75) * (2.0 * PI / 3.0)).sin() + 1.0
            }
        }
        ElasticInOut => {
            if x == 0.0 || x == 1.0 {
                x
            } else if x < 0.5 {
                -(2.0_f64.powf(20.0 * x - 10.0) * ((20.0 * x - 11.125) * (2.0 * PI / 4.5)).sin())
                    * 0.5
            } else {
                (2.0_f64.powf(-20.0 * x + 10.0) * ((20.0 * x - 11.125) * (2.0 * PI / 4.5)).sin())
                    * 0.5
                    + 1.0
            }
        }
        BounceIn => 1.0 - bounce_out(1.0 - x),
        BounceOut => bounce_out(x),
        BounceInOut => {
            if x < 0.5 {
                (1.0 - bounce_out(1.0 - 2.0 * x)) * 0.5
            } else {
                (1.0 + bounce_out(2.0 * x - 1.0)) * 0.5
            }
        }
        Jump => (x >= 1.0) as u8 as f64,
    }
}

pub(crate) fn derivative(interpolation: Interpolation, progress: f64) -> Option<f64> {
    use Interpolation::*;
    let x = progress.clamp(0.0, 1.0);
    if derivative_breakpoints(interpolation).contains(&x) {
        return None;
    }
    match interpolation {
        Linear => Some(1.0),
        ManimSmooth => {
            let error = sigmoid(-5.0);
            let value = sigmoid(10.0 * (x - 0.5));
            Some(10.0 * value * (1.0 - value) / (1.0 - 2.0 * error))
        }
        SineIn => Some(FRAC_PI_2 * (x * FRAC_PI_2).sin()),
        SineOut => Some(FRAC_PI_2 * (x * FRAC_PI_2).cos()),
        SineInOut => Some(FRAC_PI_2 * (PI * x).sin()),
        QuadIn => Some(2.0 * x),
        QuadOut => Some(2.0 * (1.0 - x)),
        QuadInOut => Some(power_in_out_derivative(x, 2)),
        CubicIn => Some(3.0 * x.powi(2)),
        CubicOut => Some(3.0 * (1.0 - x).powi(2)),
        CubicInOut => Some(power_in_out_derivative(x, 3)),
        QuartIn => Some(4.0 * x.powi(3)),
        QuartOut => Some(4.0 * (1.0 - x).powi(3)),
        QuartInOut => Some(power_in_out_derivative(x, 4)),
        QuintIn => Some(5.0 * x.powi(4)),
        QuintOut => Some(5.0 * (1.0 - x).powi(4)),
        QuintInOut => Some(power_in_out_derivative(x, 5)),
        ExpoIn if x == 0.0 => None,
        ExpoIn => Some(10.0 * LN_2 * 2.0_f64.powf(10.0 * x - 10.0)),
        ExpoOut if x == 1.0 => None,
        ExpoOut => Some(10.0 * LN_2 * 2.0_f64.powf(-10.0 * x)),
        ExpoInOut if x == 0.0 || x == 1.0 => None,
        ExpoInOut if x < 0.5 => Some(10.0 * LN_2 * 2.0_f64.powf(20.0 * x - 10.0)),
        ExpoInOut => Some(10.0 * LN_2 * 2.0_f64.powf(-20.0 * x + 10.0)),
        CircIn if x == 1.0 => None,
        CircIn => Some(x / (1.0 - x * x).sqrt()),
        CircOut if x == 0.0 => None,
        CircOut => Some((1.0 - x) / (1.0 - (x - 1.0).powi(2)).sqrt()),
        CircInOut if x == 0.5 => None,
        CircInOut if x < 0.5 => Some(2.0 * x / (1.0 - 4.0 * x * x).sqrt()),
        CircInOut => Some(2.0 * (1.0 - x) / (1.0 - (-2.0 * x + 2.0).powi(2)).sqrt()),
        BackIn => Some(3.0 * (BACK + 1.0) * x.powi(2) - 2.0 * BACK * x),
        BackOut => Some(3.0 * (BACK + 1.0) * (x - 1.0).powi(2) + 2.0 * BACK * (x - 1.0)),
        BackInOut if x < 0.5 => {
            let y = 2.0 * x;
            Some(y * (3.0 * (BACK_IN_OUT + 1.0) * y - 2.0 * BACK_IN_OUT))
        }
        BackInOut => {
            let y = 2.0 * x - 2.0;
            Some(y * (3.0 * (BACK_IN_OUT + 1.0) * y + 2.0 * BACK_IN_OUT))
        }
        ElasticIn if x == 0.0 || x == 1.0 => None,
        ElasticIn => {
            let frequency = 2.0 * PI / 3.0;
            let angle = (10.0 * x - 10.75) * frequency;
            let amplitude = 2.0_f64.powf(10.0 * x - 10.0);
            Some(-amplitude * (10.0 * LN_2 * angle.sin() + 10.0 * frequency * angle.cos()))
        }
        ElasticOut if x == 0.0 || x == 1.0 => None,
        ElasticOut => {
            let frequency = 2.0 * PI / 3.0;
            let angle = (10.0 * x - 0.75) * frequency;
            let amplitude = 2.0_f64.powf(-10.0 * x);
            Some(amplitude * (-10.0 * LN_2 * angle.sin() + 10.0 * frequency * angle.cos()))
        }
        ElasticInOut if x == 0.0 || x == 1.0 => None,
        ElasticInOut if x < 0.5 => {
            let frequency = 2.0 * PI / 4.5;
            let angle = (20.0 * x - 11.125) * frequency;
            let amplitude = 2.0_f64.powf(20.0 * x - 10.0);
            Some(-0.5 * amplitude * (20.0 * LN_2 * angle.sin() + 20.0 * frequency * angle.cos()))
        }
        ElasticInOut => {
            let frequency = 2.0 * PI / 4.5;
            let angle = (20.0 * x - 11.125) * frequency;
            let amplitude = 2.0_f64.powf(-20.0 * x + 10.0);
            Some(0.5 * amplitude * (-20.0 * LN_2 * angle.sin() + 20.0 * frequency * angle.cos()))
        }
        BounceIn => bounce_out_derivative(1.0 - x),
        BounceOut => bounce_out_derivative(x),
        BounceInOut if x < 0.5 => bounce_out_derivative(1.0 - 2.0 * x),
        BounceInOut => bounce_out_derivative(2.0 * x - 1.0),
        Jump => Some(0.0),
    }
}

pub(crate) fn derivative_breakpoints(interpolation: Interpolation) -> &'static [f64] {
    match interpolation {
        Interpolation::BounceIn => &BOUNCE_IN_BREAKS,
        Interpolation::BounceOut => &BOUNCE_OUT_BREAKS,
        Interpolation::BounceInOut => &BOUNCE_IN_OUT_BREAKS,
        Interpolation::Jump => &JUMP_BREAKS,
        _ => &[],
    }
}

fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value).exp())
}

fn power_in_out(x: f64, power: i32) -> f64 {
    if x < 0.5 {
        (2.0 * x).powi(power) * 0.5
    } else {
        1.0 - (-2.0 * x + 2.0).powi(power) * 0.5
    }
}

fn power_in_out_derivative(x: f64, power: i32) -> f64 {
    let coefficient = power as f64;
    if x < 0.5 {
        coefficient * (2.0 * x).powi(power - 1)
    } else {
        coefficient * (-2.0 * x + 2.0).powi(power - 1)
    }
}

fn bounce_out(mut x: f64) -> f64 {
    if x < BOUNCE_FIRST {
        BOUNCE_SCALE * x * x
    } else if x < BOUNCE_SECOND {
        x -= 1.5 / BOUNCE_DIVISOR;
        BOUNCE_SCALE * x * x + 0.75
    } else if x < BOUNCE_THIRD {
        x -= 2.25 / BOUNCE_DIVISOR;
        BOUNCE_SCALE * x * x + 0.9375
    } else {
        x -= 2.625 / BOUNCE_DIVISOR;
        BOUNCE_SCALE * x * x + 0.984375
    }
}

fn bounce_out_derivative(x: f64) -> Option<f64> {
    if BOUNCE_OUT_BREAKS.contains(&x) {
        return None;
    }
    let offset = if x < BOUNCE_FIRST {
        0.0
    } else if x < BOUNCE_SECOND {
        1.5 / BOUNCE_DIVISOR
    } else if x < BOUNCE_THIRD {
        2.25 / BOUNCE_DIVISOR
    } else {
        2.625 / BOUNCE_DIVISOR
    };
    Some(2.0 * BOUNCE_SCALE * (x - offset))
}
