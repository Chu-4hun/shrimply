use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::ScanlinesCrtParams;
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
    pub fn scanlines_crt(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        params: ScanlinesCrtParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let nx = (x as f32 / (width.max(2) - 1) as f32) * 2.0 - 1.0;
        let ny = (y as f32 / (height.max(2) - 1) as f32) * 2.0 - 1.0;
        let bend = 1.0 + params.curvature * (nx * nx + ny * ny);
        let sx = ((nx * bend + 1.0) * 0.5 * (width - 1) as f32).round() as i32;
        let sy = ((ny * bend + 1.0) * 0.5 * (height - 1) as f32).round() as i32;
        if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
            *output = 0;
            return;
        }
        let [r, g, b, a] = math::Color::from_rgba_u32(unsafe {
            *input.add((sy as u32 * width + sx as u32) as usize)
        })
        .to_array();
        let line = if (y as f32 / params.spacing.max(1.0)).fract() > 0.5 {
            1.0 - params.intensity
        } else {
            1.0
        };
        let phase = x % 3;
        let mr = if phase == 0 { 1.0 } else { 1.0 - params.mask };
        let mg = if phase == 1 { 1.0 } else { 1.0 - params.mask };
        let mb = if phase == 2 { 1.0 } else { 1.0 - params.mask };
        *output = math::Color::new(r * line * mr, g * line * mg, b * line * mb, a).to_rgba_u32();
    }
}
