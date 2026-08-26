use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::{resolve_color, resolve_scalar, resolve_vec2};
use shrimply_render_core::DropShadowParams;
use shrimply_video_modifiers::drop_shadow::DropShadowModifier;

#[kernel]
fn drop_shadow_horizontal(
    _: *const u32,
    _: u32,
    _: u32,
    _: DisjointSlice<f32>,
    _: DropShadowParams,
) {
}

#[kernel]
fn drop_shadow_vertical(
    _: *const u32,
    _: *const f32,
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: DropShadowParams,
) {
}

struct Resolved {
    offset: glam::Vec2,
    radius: u32,
    color: u32,
}
impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Drop shadow"
    }
    fn apply(
        &self,
        c: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let launch =
            LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?);
        let mut pass = input.into_pass(c)?;
        let mut alpha = c.take_typed_scratch::<f32>(count)?;
        let module = c.modifier_module(crate::gpu::modifiers::ModifierModule::General)?;
        let params = DropShadowParams {
            offset: self.offset,
            radius: self.radius,
            color: self.color,
        };
        unsafe {
            cuda_host::cuda_launch! {
                kernel: drop_shadow_horizontal,
                stream: c.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), width, height, slice_mut(&mut alpha), params]
            }
        }
        .map_err(|error| format!("launch horizontal CUDA kernel: {error:?}"))?;
        let alpha_ptr = alpha.cu_deviceptr() as usize as *const f32;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: drop_shadow_vertical,
                stream: c.stream(), module: &module, config: launch,
                args: [
                    pass.input_ptr(), alpha_ptr, width, height,
                    slice_mut(pass.output_buffer()), params
                ]
            }
        }
        .map_err(|error| format!("launch vertical CUDA kernel: {error:?}"))?;
        c.recycle_typed_scratch(alpha);
        Ok(pass.finish(c))
    }
}
impl RasterModifierRuntime for DropShadowModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        c: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let offset = resolve_vec2(&self.offset, c.evaluation, c.expressions);
        let color = resolve_color(&self.color, c.evaluation, c.expressions);
        input.push_pixel(Box::new(Resolved {
            offset,
            radius: resolve_scalar(&self.blur_radius, c.evaluation, c.expressions).clamp(0., 32.)
                as u32,
            color: color.to_rgba_u32(),
        }));
        Ok(input)
    }
}
