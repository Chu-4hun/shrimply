use super::LayerBlendMode;
use crate::math::Color;

pub(super) fn blend(
    source: u32,
    destination: u32,
    mode: LayerBlendMode,
    opacity: f32,
    noise_seed: u32,
    pixel_index: u32,
) -> u32 {
    let source = Color::from_rgba_u32(source);
    let destination = Color::from_rgba_u32(destination);
    let mut source_alpha = source.a * opacity.clamp(0.0, 1.0);
    if matches!(mode, LayerBlendMode::Dissolve) {
        source_alpha =
            (noise(noise_seed, pixel_index) as f32 <= source_alpha * u32::MAX as f32) as u8 as f32;
    }
    source
        .blend_over::<false>(destination, mode, source_alpha)
        .to_rgba_u32()
}

fn noise(seed: u32, pixel_index: u32) -> u32 {
    let mut value = seed ^ pixel_index.wrapping_mul(0x9e37_79b9);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}
