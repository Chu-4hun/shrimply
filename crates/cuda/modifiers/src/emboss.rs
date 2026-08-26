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
    pub fn emboss(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        direction: f32,
        depth: f32,
        amount: f32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let radians = direction.to_radians();
        let dx = radians.cos().round() as i32;
        let dy = radians.sin().round() as i32;
        let at = |step: i32| {
            let xx = (x as i32 + dx * step).max(0).min(width as i32 - 1) as u32;
            let yy = (y as i32 + dy * step).max(0).min(height as i32 - 1) as u32;
            math::Color::from_rgba_u32(unsafe { *input.add((yy * width + xx) as usize) })
        };
        let [r, g, b, a] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        let previous = at(-1);
        let next = at(1);
        let value = 0.5
            + ((previous.r + previous.g + previous.b) - (next.r + next.g + next.b)) / 3.0 * depth;
        *output = math::Color::new(
            r + (value - r) * amount,
            g + (value - g) * amount,
            b + (value - b) * amount,
            a,
        )
        .to_rgba_u32();
    }
}
