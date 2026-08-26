use cuda_core::LaunchConfig;
use cuda_device::{DisjointSlice, kernel};
use shrimply_project::project::{
    VisualClipTransition, VisualClipTransitionKind, VisualTransition, VisualTransitionKind,
};
use shrimply_render_core::VisualTransitionMaskKind;

use crate::gpu::modifiers::{CanvasRgbaFrame, GpuModifier, ModifierContext};
use crate::layer::Visual;

#[kernel]
fn visual_transition_mask(
    _: *const u32,
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: shrimply_render_core::VisualTransitionMaskParams,
) {
}

#[kernel]
fn transition_gaussian_blur_horizontal(_: *const u32, _: u32, _: DisjointSlice<u32>, _: u32) {}

#[kernel]
fn transition_gaussian_blur_vertical(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32) {
}

#[kernel]
fn transition_pixelate(_: *const u32, _: u32, _: u32, _: DisjointSlice<u32>, _: u32, _: u32) {}

#[kernel]
fn origami_transition(
    _: *const u32,
    _: u32,
    _: u32,
    _: DisjointSlice<u32>,
    _: *const f32,
    _: u32,
    _: f32,
) {
}

struct Mask {
    kind: VisualTransitionMaskKind,
    visibility: f32,
    angle_degrees: f32,
    softness: f32,
    center: glam::Vec2,
    normalized_center: bool,
    grain_size: u32,
    line_variation: f32,
}

impl GpuModifier for Mask {
    fn name(&self) -> &'static str {
        "Visual transition mask"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let center = if self.normalized_center {
            self.center * glam::Vec2::new(width as f32, height as f32)
        } else {
            self.center
        };
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Matte)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: visual_transition_mask,
                stream: context.stream(), module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [
                    pass.input_ptr(),
                    width,
                    height,
                    slice_mut(pass.output_buffer()),
                    shrimply_render_core::VisualTransitionMaskParams {
                        kind: self.kind,
                        visibility: self.visibility,
                        angle_degrees: self.angle_degrees,
                        softness: self.softness,
                        center,
                        grain_size: self.grain_size,
                        line_variation: self.line_variation,
                    }
                ]
            }
        }
        .map_err(|error| format!("launch visual transition mask CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

struct Blur {
    radius: u32,
}

impl GpuModifier for Blur {
    fn name(&self) -> &'static str {
        "Transition blur"
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
        let mut scratch = context.take_scratch(count)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Matte)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: transition_gaussian_blur_horizontal,
                stream: context.stream(), module: &module, config: launch,
                args: [pass.input_ptr(), width, slice_mut(&mut scratch), self.radius]
            }
        }
        .map_err(|error| format!("launch transition horizontal blur CUDA kernel: {error:?}"))?;
        let scratch_ptr = scratch.cu_deviceptr() as usize as *const u32;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: transition_gaussian_blur_vertical,
                stream: context.stream(), module: &module, config: launch,
                args: [scratch_ptr, width, height, slice_mut(pass.output_buffer()), self.radius]
            }
        }
        .map_err(|error| format!("launch transition vertical blur CUDA kernel: {error:?}"))?;
        context.recycle_scratch(scratch);
        Ok(pass.finish(context))
    }
}

struct Pixelate {
    block_size: u32,
}

impl GpuModifier for Pixelate {
    fn name(&self) -> &'static str {
        "Transition pixelate"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Matte)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: transition_pixelate,
                stream: context.stream(), module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [
                    pass.input_ptr(),
                    width,
                    height,
                    slice_mut(pass.output_buffer()),
                    self.block_size,
                    self.block_size
                ]
            }
        }
        .map_err(|error| format!("launch transition pixelate CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

struct Origami {
    visibility: f32,
    depth: f32,
    direction_degrees: f32,
    grid: u32,
}

impl GpuModifier for Origami {
    fn name(&self) -> &'static str {
        "Origami transition"
    }

    fn apply(
        &self,
        context: &mut ModifierContext<'_>,
        input: CanvasRgbaFrame,
    ) -> Result<CanvasRgbaFrame, String> {
        let width = input.width();
        let height = input.height();
        let count = width as usize * height as usize;
        let vertices = crate::math::origami_mesh_vertices(
            width,
            height,
            self.grid,
            self.visibility,
            self.depth,
            self.direction_degrees,
        );
        let vertices = context.upload(&vertices)?;
        let vertices_ptr = vertices.cu_deviceptr() as usize as *const f32;
        let mut pass = input.into_pass(context)?;
        let module = context.modifier_module(crate::gpu::modifiers::ModifierModule::Matte)?;
        unsafe {
            cuda_host::cuda_launch! {
                kernel: origami_transition,
                stream: context.stream(), module: &module,
                config: LaunchConfig::for_num_elems(u32::try_from(count).map_err(|_| "canvas is too large")?),
                args: [
                    pass.input_ptr(),
                    width,
                    height,
                    slice_mut(pass.output_buffer()),
                    vertices_ptr,
                    self.grid,
                    self.visibility
                ]
            }
        }
        .map_err(|error| format!("launch origami transition CUDA kernel: {error:?}"))?;
        Ok(pass.finish(context))
    }
}

pub(crate) fn apply(
    visual: &mut Visual,
    transition: &VisualTransition,
    visibility: f32,
    center: glam::Vec2,
) {
    let visibility = visibility.clamp(0.0, 1.0);
    match transition.kind {
        VisualTransitionKind::Wipe => visual.push_pixel(Box::new(Mask {
            kind: VisualTransitionMaskKind::Wipe,
            visibility,
            angle_degrees: transition.effect_angle_degrees,
            softness: transition.effect_detail,
            center,
            normalized_center: false,
            grain_size: 1,
            line_variation: 0.0,
        })),
        VisualTransitionKind::Iris => visual.push_pixel(Box::new(Mask {
            kind: VisualTransitionMaskKind::Iris,
            visibility,
            angle_degrees: 0.0,
            softness: transition.effect_detail,
            center: transition.iris_center,
            normalized_center: true,
            grain_size: u32::from(transition.effect_amount >= 0.5),
            line_variation: 0.0,
        })),
        VisualTransitionKind::ClockWipe => visual.push_pixel(Box::new(Mask {
            kind: VisualTransitionMaskKind::ClockWipe,
            visibility,
            angle_degrees: transition.effect_angle_degrees,
            softness: transition.effect_detail,
            center,
            normalized_center: false,
            grain_size: u32::from(transition.effect_amount >= 0.5),
            line_variation: 0.0,
        })),
        VisualTransitionKind::Blur => {
            let radius = (transition.effect_amount * (1.0 - visibility))
                .round()
                .clamp(0.0, 100.0) as u32;
            if radius > 0 {
                visual.push_pixel(Box::new(Blur { radius }));
            }
        }
        VisualTransitionKind::Pixelate => {
            let block_size =
                crate::math::lerp(transition.effect_amount.clamp(1.0, 512.0), 1.0, visibility)
                    .round() as u32;
            if block_size > 1 {
                visual.push_pixel(Box::new(Pixelate { block_size }));
            }
        }
        VisualTransitionKind::Dissolve => visual.push_pixel(Box::new(Mask {
            kind: VisualTransitionMaskKind::Dissolve,
            visibility,
            angle_degrees: 0.0,
            softness: 0.0,
            center,
            normalized_center: false,
            grain_size: transition.effect_detail.round().clamp(1.0, 64.0) as u32,
            line_variation: 0.0,
        })),
        VisualTransitionKind::TriangularFold => visual.push_pixel(Box::new(Mask {
            kind: VisualTransitionMaskKind::TriangularFold,
            visibility,
            angle_degrees: transition.effect_angle_degrees,
            softness: transition.effect_amount,
            center,
            normalized_center: false,
            grain_size: transition.effect_detail.round().clamp(32.0, 512.0) as u32,
            line_variation: 0.0,
        })),
        VisualTransitionKind::Origami if visibility < 0.98 => {
            visual.push_pixel(Box::new(Origami {
                visibility,
                depth: transition.effect_amount,
                direction_degrees: transition.effect_angle_degrees,
                grid: transition.effect_detail.round().clamp(2.0, 6.0) as u32,
            }))
        }
        VisualTransitionKind::StreakWipe => visual.push_pixel(Box::new(Mask {
            kind: VisualTransitionMaskKind::StreakWipe,
            visibility,
            angle_degrees: transition.effect_angle_degrees,
            softness: transition.effect_softness,
            center,
            normalized_center: false,
            grain_size: transition.effect_detail.round().clamp(1.0, 256.0) as u32,
            line_variation: transition.effect_amount.clamp(0.0, 1.0),
        })),
        _ => {}
    }
}

pub(crate) fn apply_clip_mask(
    visual: &mut Visual,
    transition: &VisualClipTransition,
    progress: f32,
) {
    let (kind, center, normalized_center, grain_size) = match transition.kind {
        VisualClipTransitionKind::Wipe => {
            (VisualTransitionMaskKind::Wipe, glam::Vec2::ZERO, false, 1)
        }
        VisualClipTransitionKind::Iris => (
            VisualTransitionMaskKind::Iris,
            transition.center,
            true,
            u32::from(transition.iris_from_inside),
        ),
        VisualClipTransitionKind::Dissolve => (
            VisualTransitionMaskKind::Dissolve,
            glam::Vec2::ZERO,
            false,
            transition.dissolve_grain_size,
        ),
        VisualClipTransitionKind::ClockWipe => (
            VisualTransitionMaskKind::ClockWipe,
            transition.center,
            true,
            u32::from(transition.clockwise),
        ),
        _ => return,
    };
    visual.push_pixel(Box::new(Mask {
        kind,
        visibility: progress.clamp(0.0, 1.0),
        angle_degrees: transition.direction_degrees,
        softness: transition.softness,
        center,
        normalized_center,
        grain_size,
        line_variation: 0.0,
    }));
}
