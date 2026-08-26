#![feature(proc_macro_hygiene)]

mod math {
    pub use shrimply_render_core::math::*;
}
mod bulge_pinch;
mod corner_pin;
mod displacement_map;
mod fisheye;
mod kaleidoscope;
mod lens_distortion;
mod mirror;
mod pixelate_mosaic;
mod twirl;
mod wave_ripple;

fn main() {}
