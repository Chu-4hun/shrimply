use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};

use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::erode_dilate::{ErodeDilateModifier, ErodeDilateOperation};

#[kernel]
fn erode_horizontal(_: *const u32, _: u32, _: DisjointSlice<f32>, _: u32) {}

#[kernel]
fn dilate_horizontal(_: *const u32, _: u32, _: DisjointSlice<f32>, _: u32) {}

#[kernel]
fn erode_vertical(_: *const u32, _: *const f32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32) {}

#[kernel]
fn dilate_vertical(_: *const u32, _: *const f32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32) {}

struct Resolved {
    operation: ErodeDilateOperation,
    radius: u32,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Erode / dilate"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let launch =
            LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?);
        let mut pass = input.into_pass(context)?;
        let mut scratch = context.take_typed_scratch::<f32>(count)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Blur)?;

        match self.operation {
            ErodeDilateOperation::Erode => unsafe {
                cuda_host::cuda_launch! {
                    kernel: erode_horizontal,
                    stream: context.stream(),
                    module: &module,
                    config: launch,
                    args: [
                        pass.input_ptr(),
                        width,
                        slice_mut(&mut scratch),
                        self.radius
                    ]
                }
            },
            ErodeDilateOperation::Dilate => unsafe {
                cuda_host::cuda_launch! {
                    kernel: dilate_horizontal,
                    stream: context.stream(),
                    module: &module,
                    config: launch,
                    args: [
                        pass.input_ptr(),
                        width,
                        slice_mut(&mut scratch),
                        self.radius
                    ]
                }
            },
        }
        .map_err(|error| format!("launch horizontal morphology CUDA kernel: {error:?}"))?;

        let horizontal = scratch.cu_deviceptr() as usize as *const f32;
        match self.operation {
            ErodeDilateOperation::Erode => unsafe {
                cuda_host::cuda_launch! {
                    kernel: erode_vertical,
                    stream: context.stream(),
                    module: &module,
                    config: launch,
                    args: [
                        pass.input_ptr(),
                        horizontal,
                        width,
                        height,
                        slice_mut(pass.output_buffer()),
                        self.radius
                    ]
                }
            },
            ErodeDilateOperation::Dilate => unsafe {
                cuda_host::cuda_launch! {
                    kernel: dilate_vertical,
                    stream: context.stream(),
                    module: &module,
                    config: launch,
                    args: [
                        pass.input_ptr(),
                        horizontal,
                        width,
                        height,
                        slice_mut(pass.output_buffer()),
                        self.radius
                    ]
                }
            },
        }
        .map_err(|error| format!("launch vertical morphology CUDA kernel: {error:?}"))?;

        context.recycle_typed_scratch(scratch);
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for ErodeDilateModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let radius = resolve_scalar(&self.radius, context.evaluation, context.expressions)
            .clamp(0.0, 100.0) as u32;
        if radius > 0 {
            input.push_pixel(Box::new(Resolved {
                operation: self.operation.value_at(context.evaluation.local_time()),
                radius,
            }));
        }
        Ok(input)
    }
}
