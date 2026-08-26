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
    pub fn directional_blur(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
        angle: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = (i as u32 % width) as i32;
        let y = (i as u32 / width) as i32;
        let dx = math::sin_f32(angle + core::f32::consts::FRAC_PI_2);
        let dy = math::sin_f32(angle);
        let mut r = 0.0;
        let mut g = 0.0;
        let mut b = 0.0;
        let mut a = 0.0;
        let count = radius * 2 + 1;
        for d in -(radius as i32)..=radius as i32 {
            let sx = (x as f32 + dx * d as f32)
                .round()
                .clamp(0.0, (width - 1) as f32) as usize;
            let sy = (y as f32 + dy * d as f32)
                .round()
                .clamp(0.0, (height - 1) as f32) as usize;
            let p = math::Color::from_rgba_u32(unsafe { *input.add(sy * width as usize + sx) });
            r += p.r * p.a;
            g += p.g * p.a;
            b += p.b * p.a;
            a += p.a;
        }
        let n = count as f32;
        let inv_alpha = if a > 0.0 { 1.0 / a } else { 0.0 };
        *output =
            math::Color::new(r * inv_alpha, g * inv_alpha, b * inv_alpha, a / n).to_rgba_u32();
    }
}
