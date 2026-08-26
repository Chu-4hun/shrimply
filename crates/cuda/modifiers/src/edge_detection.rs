use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::EdgeDetectionParams;
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
    fn luminance(pixel: u32) -> f32 {
        math::Color::from_rgba_u32(pixel).rec709_luma()
    }
    #[kernel]
    pub fn edge_detection(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        params: EdgeDetectionParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let at = |dx: i32, dy: i32| {
            let xx = (x as i32 + dx).max(0).min(width as i32 - 1) as u32;
            let yy = (y as i32 + dy).max(0).min(height as i32 - 1) as u32;
            luminance(unsafe { *input.add((yy * width + xx) as usize) })
        };
        let gx = -at(-1, -1) - 2.0 * at(-1, 0) - at(-1, 1) + at(1, -1) + 2.0 * at(1, 0) + at(1, 1);
        let gy = -at(-1, -1) - 2.0 * at(0, -1) - at(1, -1) + at(-1, 1) + 2.0 * at(0, 1) + at(1, 1);
        let edge = (gx * gx + gy * gy).sqrt().clamp(0.0, 1.0) * params.amount;
        let [_, _, _, a] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        *output = params
            .background
            .lerp(params.edge, edge)
            .with_alpha(a)
            .to_rgba_u32();
    }
}
