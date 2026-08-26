use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::ChromaticAberrationParams;
use std::sync::Arc;
pub(crate) fn load(c: &Arc<CudaContext>) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(c)
}
#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;
    fn clamp(v: i32, max: u32) -> u32 {
        v.max(0).min(max as i32 - 1) as u32
    }
    #[kernel]
    pub fn chromatic_aberration(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        params: ChromaticAberrationParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(d) = out.get_mut(idx) else { return };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let at = |ox: f32, oy: f32| unsafe {
            *input.add(
                (clamp(y as i32 + oy.round() as i32, height) * width
                    + clamp(x as i32 + ox.round() as i32, width)) as usize,
            )
        };
        let [r, _, _, _] =
            math::Color::from_rgba_u32(at(params.offsets[0], params.offsets[1])).to_array();
        let [_, g, _, a] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        let [_, _, b, _] =
            math::Color::from_rgba_u32(at(params.offsets[2], params.offsets[3])).to_array();
        *d = math::Color::new(r, g, b, a).to_rgba_u32();
    }
}
