use std::sync::Arc;

use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;

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
    pub fn chroma_key(
        input: *const u32,
        mut out: DisjointSlice<u32>,
        params: shrimply_render_core::ChromaKeyParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        *output = apply(
            unsafe { *input.add(i) },
            params.key.r,
            params.key.g,
            params.key.b,
            params.similarity,
            params.softness,
            params.spill,
        );
    }

    fn apply(
        pixel: u32,
        key_r: f32,
        key_g: f32,
        key_b: f32,
        similarity: f32,
        softness: f32,
        spill: f32,
    ) -> u32 {
        let [mut r, mut g, mut b, a] = math::Color::from_rgba_u32(pixel).to_array();
        let dr = r - key_r;
        let dg = g - key_g;
        let db = b - key_b;
        let distance = (dr * dr + dg * dg + db * db).sqrt() / 1.732_050_8;
        let keep = math::smoothstep(similarity, similarity + softness.max(0.000_01), distance);
        let key_max = key_r.max(key_g).max(key_b);
        let key_min = key_r.min(key_g).min(key_b);
        let chroma = (key_max - key_min).max(0.000_01);
        let kr = (key_r - key_min) / chroma;
        let kg = (key_g - key_min) / chroma;
        let kb = (key_b - key_min) / chroma;
        let projection = (r * kr + g * kg + b * kb) / (kr * kr + kg * kg + kb * kb).max(0.000_01);
        let removal = projection.max(0.0) * spill.clamp(0.0, 1.0) * (1.0 - keep);
        r = (r - kr * removal).max(0.0);
        g = (g - kg * removal).max(0.0);
        b = (b - kb * removal).max(0.0);
        math::Color::new(r, g, b, a * keep).to_rgba_u32()
    }
}
