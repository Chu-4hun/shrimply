fn sin(value) {
  shrimply_sin(Fraction(value))
}

fn cos(value) {
  shrimply_cos(Fraction(value))
}

fn tan(value) {
  shrimply_tan(Fraction(value))
}

fn sqrt(value) {
  shrimply_sqrt(Fraction(value))
}

fn pow(value, power) {
  shrimply_pow(Fraction(value), Fraction(power))
}

fn clamp(value, low, high) {
  let value = Fraction(value);
  let low = Fraction(low);
  let high = Fraction(high);
  if value < low {
    low
  } else if value > high {
    high
  } else {
    value
  }
}

fn lerp(a, b, progress) {
  let a = Fraction(a);
  let b = Fraction(b);
  a + (b - a) * Fraction(progress)
}
