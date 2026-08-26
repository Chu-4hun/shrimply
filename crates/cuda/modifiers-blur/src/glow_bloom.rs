use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use std::sync::Arc;

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
    pub fn glow_bloom_horizontal(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<math::Color>,
        threshold: f32,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i % width as usize;
        let row = i - x;
        let radius = radius as i32;
        let mut glow: math::Color = math::Color::TRANSPARENT;
        for dx in -radius..=radius {
            let sample_x = (x as i32 + dx).clamp(0, width as i32 - 1) as usize;
            let pixel = math::Color::from_rgba_u32(unsafe { *input.add(row + sample_x) });
            let luminance = (pixel.r + pixel.g + pixel.b) / 3.0 * pixel.a;
            if luminance >= threshold {
                glow.r += pixel.r * pixel.a;
                glow.g += pixel.g * pixel.a;
                glow.b += pixel.b * pixel.a;
                glow.a += pixel.a;
            }
        }
        let scale = 1.0 / (2 * radius + 1) as f32;
        *output = glow.map(|channel| channel * scale);
    }

    #[kernel]
    pub fn glow_bloom_vertical(
        input: *const u32,
        horizontal: *const math::Color,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
        intensity: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i % width as usize;
        let y = i / width as usize;
        let radius = radius as i32;
        let mut glow: math::Color = math::Color::TRANSPARENT;
        for dy in -radius..=radius {
            let sample_y = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
            let sample = unsafe { *horizontal.add(sample_y * width as usize + x) };
            glow.r += sample.r;
            glow.g += sample.g;
            glow.b += sample.b;
            glow.a += sample.a;
        }
        let scale = intensity / (2 * radius + 1) as f32;
        let base = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let glow_alpha = (glow.a * scale).clamp(0.0, 1.0);
        let output_alpha = base.a + glow_alpha * (1.0 - base.a);
        let inverse_alpha = if output_alpha > 0.0 {
            1.0 / output_alpha
        } else {
            0.0
        };
        *output = math::Color::new(
            (base.r * base.a + glow.r * scale) * inverse_alpha,
            (base.g * base.a + glow.g * scale) * inverse_alpha,
            (base.b * base.a + glow.b * scale) * inverse_alpha,
            output_alpha,
        )
        .to_rgba_u32();
    }
}
