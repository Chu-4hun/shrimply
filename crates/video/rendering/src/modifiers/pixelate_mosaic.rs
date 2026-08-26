use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::pixelate_mosaic::PixelateMosaicModifier;
#[kernel]
fn pixelate_mosaic(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32, _: u32) {}
struct Resolved {
    block_width: u32,
    block_height: u32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Pixelate / mosaic"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let w = input.width();
        let h = input.height();
        let count = w as usize * h as usize;
        let mut pass = input.into_pass(c)?;
        let module = c.modifier_module(crate::gpu::modifiers::ModifierModule::Geometry)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: pixelate_mosaic,
                stream: c.stream(),
                module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [
                    pass.input_ptr(),
                    w,
                    h,
                    slice_mut(pass.output_buffer()),
                    self.block_width,
                    self.block_height
                ]
            }
        }
        .map_err(|error| format!("launch CUDA kernel: {error:?}"))?;
        Ok(pass.finish(c))
    }
}
impl RasterModifierRuntime for PixelateMosaicModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        input.push_pixel(Box::new(Resolved {
            block_width: resolve_scalar(&self.block_width, c.evaluation, c.expressions)
                .clamp(1., 512.) as u32,
            block_height: resolve_scalar(&self.block_height, c.evaluation, c.expressions)
                .clamp(1., 512.) as u32,
        }));
        Ok(input)
    }
}
