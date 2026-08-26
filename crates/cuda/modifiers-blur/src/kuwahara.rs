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
    pub fn kuwahara_horizontal_statistics(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<[f32; 8]>,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = (i as u32 % width) as i32;
        let row = i / width as usize;
        let mut sums = [0.0; 8];
        for distance in 0..=radius as i32 {
            accumulate(&mut sums, 0, unsafe {
                *input
                    .add(row * width as usize + (x - distance).clamp(0, width as i32 - 1) as usize)
            });
            accumulate(&mut sums, 4, unsafe {
                *input
                    .add(row * width as usize + (x + distance).clamp(0, width as i32 - 1) as usize)
            });
        }
        *output = sums;
    }

    #[kernel]
    pub fn kuwahara_vertical(
        input: *const u32,
        statistics: *const [f32; 8],
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
        generalized: bool,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i % width as usize;
        let y = (i / width as usize) as i32;
        let mut regions = [[0.0; 4]; 4];
        for distance in 0..=radius as i32 {
            let top = (y - distance).clamp(0, height as i32 - 1) as usize;
            let bottom = (y + distance).clamp(0, height as i32 - 1) as usize;
            let top_stats = unsafe { *statistics.add(top * width as usize + x) };
            let bottom_stats = unsafe { *statistics.add(bottom * width as usize + x) };
            add_half(&mut regions[0], &top_stats, 0);
            add_half(&mut regions[1], &top_stats, 4);
            add_half(&mut regions[2], &bottom_stats, 0);
            add_half(&mut regions[3], &bottom_stats, 4);
        }

        let side = (radius + 1) as f32;
        let area = side * side;
        let [_, _, _, alpha] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        *output = if generalized {
            generalized_color(regions, area, alpha)
        } else {
            classic_color(regions, area, alpha)
        };
    }

    fn accumulate(sums: &mut [f32; 8], offset: usize, pixel: u32) {
        let color = math::Color::from_rgba_u32(pixel);
        let [r, g, b, _] = color.to_array();
        let luminance = color.rec709_luma();
        sums[offset] += r;
        sums[offset + 1] += g;
        sums[offset + 2] += b;
        sums[offset + 3] += luminance * luminance;
    }

    fn add_half(region: &mut [f32; 4], statistics: &[f32; 8], offset: usize) {
        region[0] += statistics[offset];
        region[1] += statistics[offset + 1];
        region[2] += statistics[offset + 2];
        region[3] += statistics[offset + 3];
    }

    fn variance(region: &[f32; 4], area: f32) -> f32 {
        let r = region[0] / area;
        let g = region[1] / area;
        let b = region[2] / area;
        let mean = math::Color::new(r, g, b, 1.0).rec709_luma();
        (region[3] / area - mean * mean).max(0.0)
    }

    fn classic_color(regions: [[f32; 4]; 4], area: f32, alpha: f32) -> u32 {
        let mut best = 0;
        let mut best_variance = variance(&regions[0], area);
        for (index, region) in regions.iter().enumerate().skip(1) {
            let candidate = variance(region, area);
            if candidate < best_variance {
                best = index;
                best_variance = candidate;
            }
        }
        math::Color::new(
            regions[best][0] / area,
            regions[best][1] / area,
            regions[best][2] / area,
            alpha,
        )
        .to_rgba_u32()
    }

    fn generalized_color(regions: [[f32; 4]; 4], area: f32, alpha: f32) -> u32 {
        let mut color = [0.0; 3];
        let mut total = 0.0;
        for region in &regions {
            let inverse = 1.0 / (variance(region, area) + 0.0001);
            let weight = inverse * inverse;
            color[0] += region[0] / area * weight;
            color[1] += region[1] / area * weight;
            color[2] += region[2] / area * weight;
            total += weight;
        }
        math::Color::new(color[0] / total, color[1] / total, color[2] / total, alpha).to_rgba_u32()
    }
}
