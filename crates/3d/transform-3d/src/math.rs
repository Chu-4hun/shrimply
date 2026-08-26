const SENSOR_HEIGHT_MM: f64 = 24.0;
pub const MIN_F_STOP: f32 = 0.1;
pub const MAX_F_STOP: f32 = 256.0;
pub const MIN_EXPOSURE_EV: f32 = -32.0;
pub const MAX_EXPOSURE_EV: f32 = 32.0;

pub fn focal_length_mm(vertical_fov_degrees: f64) -> f64 {
    SENSOR_HEIGHT_MM / (2.0 * (vertical_fov_degrees.to_radians() * 0.5).tan())
}

pub fn vertical_fov_degrees(focal_length_mm: f64) -> f64 {
    (2.0 * (SENSOR_HEIGHT_MM / (2.0 * focal_length_mm)).atan()).to_degrees()
}
