use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::{DitheringColorMode, DitheringParams, DitheringPattern};

pub(crate) fn load(
    context: &Arc<CudaContext>,
) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(context)
}

#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;

    #[kernel]
    pub fn dithering(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<u32>,
        params: DitheringParams,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output) = out.get_mut(index) else {
            return;
        };
        let color = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let [r, g, b, a] = color.to_array();
        let x = i as u32 % width;
        let y = i as u32 / width;
        let pattern_value = bayer(params.pattern, x, y);
        let threshold = pattern_value - 0.5;
        let steps = (params.levels.round() - 1.0).max(1.0);
        let amount = params.amount.clamp(0.0, 1.0);
        let quantize = |value: f32| {
            let adjusted = value + threshold / steps;
            (adjusted.clamp(0.0, 1.0) * steps).round() / steps
        };
        *output = match params.color_mode {
            DitheringColorMode::Color => {
                let target_r = quantize(r);
                let target_g = quantize(g);
                let target_b = quantize(b);
                math::Color::new(
                    r + (target_r - r) * amount,
                    g + (target_g - g) * amount,
                    b + (target_b - b) * amount,
                    a,
                )
                .to_rgba_u32()
            }
            DitheringColorMode::Grayscale => {
                let value = quantize(color.rec709_luma());
                math::Color::new(
                    r + (value - r) * amount,
                    g + (value - g) * amount,
                    b + (value - b) * amount,
                    a,
                )
                .to_rgba_u32()
            }
            DitheringColorMode::Palette if params.palette_len > 0 => {
                let (first, second, mix) =
                    nearest_palette_colors(r, g, b, params.palette, params.palette_len);
                let target = if pattern_value < mix { second } else { first };
                let [target_r, target_g, target_b, _] =
                    math::Color::from_rgba_u32(target).to_array();
                math::Color::new(
                    r + (target_r - r) * amount,
                    g + (target_g - g) * amount,
                    b + (target_b - b) * amount,
                    a,
                )
                .to_rgba_u32()
            }
            DitheringColorMode::Palette => unsafe { *input.add(i) },
        };
    }

    fn bayer(pattern: DitheringPattern, x: u32, y: u32) -> f32 {
        const BAYER_2: [u8; 4] = [0, 2, 3, 1];
        const BAYER_4: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
        const BAYER_8: [u8; 64] = [
            0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6,
            38, 60, 28, 52, 20, 62, 30, 54, 22, 3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49,
            17, 57, 25, 15, 47, 7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21,
        ];
        match pattern {
            DitheringPattern::Bayer2x2 => {
                (BAYER_2[((y & 1) * 2 + (x & 1)) as usize] as f32 + 0.5) / 4.0
            }
            DitheringPattern::Bayer4x4 => {
                (BAYER_4[((y & 3) * 4 + (x & 3)) as usize] as f32 + 0.5) / 16.0
            }
            DitheringPattern::Bayer8x8 => {
                (BAYER_8[((y & 7) * 8 + (x & 7)) as usize] as f32 + 0.5) / 64.0
            }
        }
    }

    fn nearest_palette_colors(
        r: f32,
        g: f32,
        b: f32,
        palette: *const u32,
        palette_len: u32,
    ) -> (u32, u32, f32) {
        let mut best = unsafe { *palette };
        let mut second = best;
        let mut best_distance = f32::MAX;
        let mut second_distance = f32::MAX;
        let mut index = 0;
        while index < palette_len {
            let color = unsafe { *palette.add(index as usize) };
            let [candidate_r, candidate_g, candidate_b, _] =
                math::Color::from_rgba_u32(color).to_array();
            let dr = r - candidate_r;
            let dg = g - candidate_g;
            let db = b - candidate_b;
            let distance = dr * dr + dg * dg + db * db;
            if distance < best_distance {
                second = best;
                second_distance = best_distance;
                best = color;
                best_distance = distance;
            } else if distance < second_distance {
                second = color;
                second_distance = distance;
            }
            index += 1;
        }
        let [best_r, best_g, best_b, _] = math::Color::from_rgba_u32(best).to_array();
        let [second_r, second_g, second_b, _] = math::Color::from_rgba_u32(second).to_array();
        let dr = second_r - best_r;
        let dg = second_g - best_g;
        let db = second_b - best_b;
        let denominator = dr * dr + dg * dg + db * db;
        let mix = if denominator > 0.000_001 {
            ((r - best_r) * dr + (g - best_g) * dg + (b - best_b) * db) / denominator
        } else {
            0.0
        };
        (best, second, mix.clamp(0.0, 1.0))
    }
}
