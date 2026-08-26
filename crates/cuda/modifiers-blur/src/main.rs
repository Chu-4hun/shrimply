#![feature(proc_macro_hygiene)]

mod math {
    pub use shrimply_render_core::math::*;
}
mod directional_blur;
mod erode_dilate;
mod gaussian_blur;
mod glow_bloom;
mod kuwahara;
mod radial_blur;
mod sharpen;
mod zoom_blur;

fn main() {}
