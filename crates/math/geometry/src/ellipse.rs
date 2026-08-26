use glam::Vec2;

#[derive(Clone, Copy, Debug)]
pub struct EllipseSegment {
    pub center: Vec2,
    pub radius: Vec2,
    pub start_radians: f32,
    pub sweep_radians: f32,
}

pub fn ellipse_segment(size: Vec2, completion_degrees: f32) -> Option<EllipseSegment> {
    const FULL_ELLIPSE_DEGREES: f32 = 360.0;
    let sweep_radians = completion_degrees
        .clamp(0.0, FULL_ELLIPSE_DEGREES)
        .to_radians();
    if sweep_radians <= f32::EPSILON {
        return None;
    }
    let start_radians = core::f32::consts::TAU - sweep_radians;
    let mut minimum = Vec2::ZERO;
    let mut maximum = Vec2::ZERO;
    for angle in [
        start_radians,
        core::f32::consts::FRAC_PI_2,
        core::f32::consts::PI,
        core::f32::consts::PI + core::f32::consts::FRAC_PI_2,
        core::f32::consts::TAU,
    ] {
        if angle + f32::EPSILON < start_radians {
            continue;
        }
        let point = Vec2::new(angle.cos(), angle.sin());
        minimum = minimum.min(point);
        maximum = maximum.max(point);
    }
    let span = (maximum - minimum).max(Vec2::splat(f32::EPSILON));
    let radius = size / span;
    Some(EllipseSegment {
        center: -minimum * radius,
        radius,
        start_radians,
        sweep_radians,
    })
}
