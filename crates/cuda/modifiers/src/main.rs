#![feature(proc_macro_hygiene)]

mod math {
    pub use shrimply_render_core::math::*;
}
mod alpha_outline;
mod channel_mixer;
mod chromatic_aberration;
mod color_correction;
mod colorize_duotone;
mod dithering;
mod drop_shadow;
mod edge_detection;
mod emboss;
mod film_grain;
mod halftone;
mod invert;
mod posterize;
mod scanlines_crt;
mod threshold;
mod vignette;

fn main() {}
