use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::ChannelMixerParams;
use std::sync::Arc;
pub(crate) fn load(c: &Arc<CudaContext>) -> Result<device::LoadedModule, EmbeddedModuleError> {
    device::load(c)
}
#[cuda_module]
pub(crate) mod device {
    use super::*;
    use crate::math;
    #[kernel]
    pub fn channel_mixer(
        input: *const u32,
        mut out: DisjointSlice<u32>,
        params: ChannelMixerParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(d) = out.get_mut(idx) else { return };
        let [r, g, b, a] = math::Color::from_rgba_u32(unsafe { *input.add(i) }).to_array();
        let color = params.matrix * glam::Vec3::new(r, g, b);
        *d = math::Color::new(color.x, color.y, color.z, a).to_rgba_u32();
    }
}
