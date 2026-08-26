use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_video_modifiers::mirror::MirrorModifier;
#[kernel]
fn mirror(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32, _: u32) {}
struct Resolved {
    horizontal: u32,
    vertical: u32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Mirror"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let w = input.width();
        let h = input.height();
        let n = w as usize * h as usize;
        let mut p = input.into_pass(c)?;
        let m = c.modifier_module(crate::gpu::modifiers::ModifierModule::Geometry)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: mirror,
                stream: c.stream(),
                module: &m,
                config: LaunchConfig::for_num_elems(u32::try_from(n).map_err(|_| "canvas is too large")?),
                args: [
                    p.input_ptr(),
                    w,
                    h,
                    slice_mut(p.output_buffer()),
                    self.horizontal,
                    self.vertical
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(p.finish(c))
    }
}
impl RasterModifierRuntime for MirrorModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        _: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        input.push_pixel(Box::new(Resolved {
            horizontal: self.horizontal as u32,
            vertical: self.vertical as u32,
        }));
        Ok(input)
    }
}
