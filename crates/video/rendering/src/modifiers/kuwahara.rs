use super::RasterModifierRuntime;
use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::RasterVisual;
use crate::visual_source::VisualModifierContext;
use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_evaluation::resolve_scalar;
use shrimply_video_modifiers::kuwahara::{KuwaharaModifier, KuwaharaVersion};

#[kernel]
fn kuwahara_horizontal_statistics(_: *const u32, _: u32, _: DisjointSlice<[f32; 8]>, _: u32) {}

#[kernel]
fn kuwahara_vertical(
    _: *const u32,
    _: *const [f32; 8],
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: u32,
    _: bool,
) {
}

struct Resolved {
    radius: u32,
    generalized: bool,
}

impl GpuModifier for Resolved {
    fn name(&self) -> &'static str {
        "Kuwahara"
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
        let mut statistics = context.take_typed_scratch::<[f32; 8]>(count)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Blur)?;

        unsafe {
            cuda_host::cuda_launch! {
                kernel: kuwahara_horizontal_statistics,
                stream: context.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), width, slice_mut(&mut statistics), self.radius]
            }
        }
        .map_err(|error| format!("launch Kuwahara horizontal CUDA kernel: {error:?}"))?;
        let statistics_ptr = statistics.cu_deviceptr() as usize as *const [f32; 8];
        unsafe {
            cuda_host::cuda_launch! {
                kernel: kuwahara_vertical,
                stream: context.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), statistics_ptr, width, height, slice_mut(pass.output_buffer()), self.radius, self.generalized]
            }
        }
        .map_err(|error| format!("launch Kuwahara vertical CUDA kernel: {error:?}"))?;
        context.recycle_typed_scratch(statistics);
        Ok(pass.finish(context))
    }
}

impl RasterModifierRuntime for KuwaharaModifier {
    fn apply_raster(
        &self,
        mut input: RasterVisual,
        context: &mut VisualModifierContext<'_>,
    ) -> Result<RasterVisual, String> {
        let radius = resolve_scalar(&self.radius, context.evaluation, context.expressions)
            .clamp(0.0, 32.0) as u32;
        if radius > 0 {
            input.push_pixel(Box::new(Resolved {
                radius,
                generalized: self.version.value_at(context.evaluation.local_time())
                    == KuwaharaVersion::Generalized,
            }));
        }
        Ok(input)
    }
}
