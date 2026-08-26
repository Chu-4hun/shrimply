use std::mem::{size_of, size_of_val};

use crate::video_shader::background as shader;
use ash::vk;
use shrimply_background::{
    Background, BackgroundGenerator, CenteredLines, Checkerboard, ColorGradient, Curve,
    GradientMode, Grid, GridLineStyle, NoiseColorMode, NoiseDistribution, PerlinMode, PerlinNoise,
    Rainbow, RainbowBands, RainbowFill, SolidColor, Voronoi, VoronoiFill, VoronoiMetric,
    WhiteNoise,
};
use shrimply_project::project::Time;

const THREADS: u32 = 16;

pub(super) struct RenderContext {
    pub device: ash::Device,
    pub memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue: vk::Queue,
    pub command_pool: vk::CommandPool,
    pub pipeline_cache: vk::PipelineCache,
}

#[derive(Clone, PartialEq)]
pub(super) struct RenderKey(pub(super) shader::BackgroundUniforms);

impl RenderKey {
    pub(super) fn new(width: u32, height: u32, time: Time, background: &Background) -> Self {
        Self(uniforms(width.max(1), height.max(1), time, background))
    }
}

pub(super) struct Renderer {
    context: RenderContext,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    uniforms: Buffer,
    pending: Option<(vk::Fence, vk::CommandBuffer)>,
}

impl Renderer {
    pub(super) fn new(context: RenderContext) -> Result<Self, String> {
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];
        let descriptor_set_layout = unsafe {
            context.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }
        .map_err(|error| format!("create background descriptor layout: {error:?}"))?;
        let layouts = [descriptor_set_layout];
        let pipeline_layout = unsafe {
            context.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts),
                None,
            )
        }
        .map_err(|error| format!("create background pipeline layout: {error:?}"))?;
        let spirv = ash::util::read_spv(&mut std::io::Cursor::new(shader::SPIRV_BYTES))
            .map_err(|error| format!("decode background SPIR-V: {error}"))?;
        let module = unsafe {
            context
                .device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&spirv), None)
        }
        .map_err(|error| format!("create background shader module: {error:?}"))?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(module)
            .name(shader::MAIN_ENTRY_POINT);
        let pipeline = unsafe {
            context.device.create_compute_pipelines(
                context.pipeline_cache,
                &[vk::ComputePipelineCreateInfo::default()
                    .stage(stage)
                    .layout(pipeline_layout)],
                None,
            )
        }
        .map_err(|(_, error)| format!("create background compute pipeline: {error:?}"))?[0];
        unsafe { context.device.destroy_shader_module(module, None) };

        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
            },
        ];
        let descriptor_pool = unsafe {
            context.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&pool_sizes),
                None,
            )
        }
        .map_err(|error| format!("create background descriptor pool: {error:?}"))?;
        let descriptor_set = unsafe {
            context.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&layouts),
            )
        }
        .map_err(|error| format!("allocate background descriptor set: {error:?}"))?[0];
        let uniforms = Buffer::new(&context, size_of::<shader::BackgroundUniforms>() as u64)?;
        Ok(Self {
            context,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_set,
            pipeline_layout,
            pipeline,
            uniforms,
            pending: None,
        })
    }

    pub(super) fn render(
        &mut self,
        output: vk::Buffer,
        key: &RenderKey,
        signal: vk::Semaphore,
    ) -> Result<(), String> {
        let width = key.0.common.width;
        let height = key.0.common.height;
        self.finish_pending()?;
        self.uniforms.write(std::slice::from_ref(&key.0))?;
        let output_info = [vk::DescriptorBufferInfo {
            buffer: output,
            offset: 0,
            range: u64::from(width) * u64::from(height) * 4,
        }];
        let uniform_info = [self.uniforms.descriptor()];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(self.descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&output_info),
            vk::WriteDescriptorSet::default()
                .dst_set(self.descriptor_set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&uniform_info),
        ];
        unsafe { self.context.device.update_descriptor_sets(&writes, &[]) };

        let command = unsafe {
            self.context.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.context.command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|error| format!("allocate background command buffer: {error:?}"))?[0];
        unsafe {
            self.context.device.begin_command_buffer(
                command,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
        }
        .map_err(|error| format!("begin background command buffer: {error:?}"))?;
        unsafe {
            self.context.device.cmd_bind_pipeline(
                command,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline,
            );
            self.context.device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[self.descriptor_set],
                &[],
            );
            self.context.device.cmd_dispatch(
                command,
                width.div_ceil(THREADS),
                height.div_ceil(THREADS),
                1,
            );
            self.context.device.end_command_buffer(command)
        }
        .map_err(|error| format!("end background command buffer: {error:?}"))?;
        let fence = unsafe {
            self.context
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
        }
        .map_err(|error| format!("create background fence: {error:?}"))?;
        let submitted = unsafe {
            let commands = [command];
            let signals = [signal];
            self.context.device.queue_submit(
                self.context.queue,
                &[vk::SubmitInfo::default()
                    .command_buffers(&commands)
                    .signal_semaphores(&signals)],
                fence,
            )
        };
        if let Err(error) = submitted {
            unsafe {
                self.context.device.destroy_fence(fence, None);
                self.context
                    .device
                    .free_command_buffers(self.context.command_pool, &[command]);
            }
            return Err(format!("submit background render: {error:?}"));
        }
        self.pending = Some((fence, command));
        Ok(())
    }

    fn finish_pending(&mut self) -> Result<(), String> {
        let Some((fence, command)) = self.pending.take() else {
            return Ok(());
        };
        let result = unsafe {
            self.context
                .device
                .wait_for_fences(&[fence], true, u64::MAX)
        }
        .map_err(|error| format!("wait for background render: {error:?}"));
        unsafe {
            self.context.device.destroy_fence(fence, None);
            self.context
                .device
                .free_command_buffers(self.context.command_pool, &[command]);
        }
        result
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if let Err(error) = self.finish_pending() {
            tracing::error!(%error, "Could not finish background render during cleanup");
        }
        unsafe {
            self.context.device.destroy_pipeline(self.pipeline, None);
            self.context
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.context
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.context
                .device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}

fn uniforms(
    width: u32,
    height: u32,
    time: Time,
    background: &Background,
) -> shader::BackgroundUniforms {
    let seconds = time.as_secs_f64().max(0.0);
    let mut value: shader::BackgroundUniforms = unsafe { std::mem::zeroed() };
    value.common.width = width;
    value.common.height = height;
    match &background.generator {
        BackgroundGenerator::SolidColor(config) => solid_color(&mut value, config, time),
        BackgroundGenerator::ColorGradient(config) => gradient(&mut value, config, time),
        BackgroundGenerator::Grid(config) => grid(&mut value, config, time),
        BackgroundGenerator::WhiteNoise(config) => noise(&mut value, config, time, seconds),
        BackgroundGenerator::PerlinNoise(config) => perlin(&mut value, config, time),
        BackgroundGenerator::CenteredLines(config) => centered_lines(&mut value, config, time),
        BackgroundGenerator::Rainbow(config) => rainbow(&mut value, config, time),
        BackgroundGenerator::Checkerboard(config) => checker(&mut value, config, time),
        BackgroundGenerator::Voronoi(config) => voronoi(&mut value, config, time),
        BackgroundGenerator::TestPattern => value.common.kind = shader::BackgroundKind::TestPattern,
    }
    value
}

fn solid_color(value: &mut shader::BackgroundUniforms, config: &SolidColor, time: Time) {
    value.common.kind = shader::BackgroundKind::ColorGradient;
    value.gradient.mode = 0;
    value.gradient.color_a = config.color.value_at(time).to_srgba();
}

fn curve(value: Curve) -> u32 {
    match value {
        Curve::Step => 0,
        Curve::Linear => 1,
        Curve::Smooth => 2,
        Curve::Smoother => 3,
    }
}

fn interval(seconds: f64, duration: f32) -> (u32, f32) {
    let position = seconds / f64::from(duration.max(0.001));
    (
        position.floor().clamp(0.0, f64::from(u32::MAX)) as u32,
        position.fract() as f32,
    )
}

fn gradient(value: &mut shader::BackgroundUniforms, config: &ColorGradient, time: Time) {
    value.common.kind = shader::BackgroundKind::ColorGradient;
    value.gradient.mode = match config.mode.value_at(time) {
        GradientMode::Solid => 0,
        GradientMode::Linear => 1,
        GradientMode::Radial => 2,
        GradientMode::Conic => 3,
    };
    value.gradient.curve = curve(config.curve.value_at(time));
    value.gradient.angle = config.angle_degrees.value_at(time);
    value.gradient.scale = config.scale.value_at(time);
    value.gradient.color_a = config.color_a.value_at(time).to_srgba();
    value.gradient.color_b = config.color_b.value_at(time).to_srgba();
    value.gradient.center = config.center.value_at(time).to_array();
    value.gradient.position = config.position.value_at(time).to_array();
    value.gradient.cycle_position = config.cycle_position.value_at(time);
}

fn grid(value: &mut shader::BackgroundUniforms, config: &Grid, time: Time) {
    value.common.kind = shader::BackgroundKind::Grid;
    value.grid.background = config.background_color.value_at(time).to_srgba();
    value.grid.horizontal = config.horizontal_color.value_at(time).to_srgba();
    value.grid.vertical = config.vertical_color.value_at(time).to_srgba();
    value.grid.spacing = config.spacing.value_at(time).to_array();
    value.grid.line_width = config.line_width.value_at(time).to_array();
    value.grid.position = config.position.value_at(time).to_array();
    value.grid.rotation = config.rotation_degrees.value_at(time);
    value.grid.line_style = match config.line_style.value_at(time) {
        GridLineStyle::Solid => 0,
        GridLineStyle::Dashed => 1,
        GridLineStyle::Dotted => 2,
    };
    value.grid.dash_length = config.dash_length.value_at(time);
    value.grid.dash_gap = config.dash_gap.value_at(time);
    value.grid.dash_position = config.dash_position.value_at(time);
    value.grid.wobble_amount = config.wobble_amount.value_at(time);
    value.grid.wobble_scale = config.wobble_scale.value_at(time);
    value.grid.wobble_position = config.wobble_position.value_at(time);
    value.grid.middle_padding = config.middle_padding.value_at(time).to_array();
    value.grid.padding_randomness = config.padding_randomness.value_at(time).to_array();
    value.grid.seed = config.seed.value_at(time);
}

fn noise(value: &mut shader::BackgroundUniforms, config: &WhiteNoise, time: Time, seconds: f64) {
    value.common.kind = shader::BackgroundKind::WhiteNoise;
    value.noise.distribution = match config.distribution.value_at(time) {
        NoiseDistribution::Uniform => 0,
        NoiseDistribution::Gaussian => 1,
        NoiseDistribution::Binary => 2,
    };
    value.noise.color_mode = match config.color_mode.value_at(time) {
        NoiseColorMode::Monochrome => 0,
        NoiseColorMode::Rgb => 1,
        NoiseColorMode::Duotone => 2,
    };
    value.noise.pixel_size = config.pixel_size.value_at(time).max(1);
    value.noise.epoch = if config.animated.value_at(time).get() {
        interval(seconds, config.refresh_interval.value_at(time)).0
    } else {
        0
    };
    value.noise.color_a = config.color_a.value_at(time).to_srgba();
    value.noise.color_b = config.color_b.value_at(time).to_srgba();
    value.noise.brightness = config.brightness.value_at(time);
    value.noise.contrast = config.contrast.value_at(time);
    value.noise.seed = config.seed.value_at(time);
}

fn perlin(value: &mut shader::BackgroundUniforms, config: &PerlinNoise, time: Time) {
    value.common.kind = shader::BackgroundKind::PerlinNoise;
    value.perlin.mode = match config.mode.value_at(time) {
        PerlinMode::Fbm => 0,
        PerlinMode::Turbulence => 1,
        PerlinMode::Ridged => 2,
    };
    value.perlin.octaves = config.octaves.value_at(time).clamp(1, 8);
    value.perlin.seed = config.seed.value_at(time);
    value.perlin.scale = config.scale.value_at(time);
    value.perlin.color_a = config.color_a.value_at(time).to_srgba();
    value.perlin.color_b = config.color_b.value_at(time).to_srgba();
    value.perlin.lacunarity = config.lacunarity.value_at(time);
    value.perlin.persistence = config.persistence.value_at(time);
    value.perlin.contrast = config.contrast.value_at(time);
    value.perlin.evolution = config.evolution.value_at(time);
    value.perlin.position = config.position.value_at(time).to_array();
    value.perlin.warp_amount = config.warp_amount.value_at(time);
    value.perlin.warp_scale = config.warp_scale.value_at(time);
}

fn centered_lines(value: &mut shader::BackgroundUniforms, config: &CenteredLines, time: Time) {
    value.common.kind = shader::BackgroundKind::CenteredLines;
    value.centered_lines.background = config.background_color.value_at(time).to_srgba();
    value.centered_lines.line = config.line_color.value_at(time).to_srgba();
    value.centered_lines.center = config.center.value_at(time).to_array();
    value.centered_lines.rotation_degrees = config.rotation_degrees.value_at(time);
    value.centered_lines.line_count = config.line_count.value_at(time);
    value.centered_lines.line_width = config.line_width.value_at(time);
    value.centered_lines.line_width_randomness = config.line_width_randomness.value_at(time);
    value.centered_lines.line_length = config.line_length.value_at(time);
    value.centered_lines.line_length_randomness = config.line_length_randomness.value_at(time);
    value.centered_lines.line_offset = config.line_offset.value_at(time);
    value.centered_lines.line_offset_randomness = config.line_offset_randomness.value_at(time);
    value.centered_lines.angular_randomness = config.angular_randomness.value_at(time);
    value.centered_lines.fade_length = config.fade_length.value_at(time);
    value.centered_lines.seed = config.seed.value_at(time);
}

fn rainbow(value: &mut shader::BackgroundUniforms, config: &Rainbow, time: Time) {
    value.common.kind = shader::BackgroundKind::Rainbow;
    value.rainbow.fill = match config.fill.value_at(time) {
        RainbowFill::Linear => 0,
        RainbowFill::Radial => 1,
        RainbowFill::Conic => 2,
    };
    value.rainbow.bands = match config.bands.value_at(time) {
        RainbowBands::Smooth => 0,
        RainbowBands::Stepped => 1,
    };
    value.rainbow.band_count = config.band_count.value_at(time);
    value.rainbow.angle = config.angle_degrees.value_at(time);
    value.rainbow.center = config.center.value_at(time).to_array();
    value.rainbow.scale = config.scale.value_at(time);
    value.rainbow.saturation = config.saturation.value_at(time);
    value.rainbow.brightness = config.brightness.value_at(time);
    value.rainbow.alpha = config.alpha.value_at(time);
    value.rainbow.position = config.position.value_at(time).to_array();
    value.rainbow.hue_position = config.hue_position.value_at(time);
}

fn checker(value: &mut shader::BackgroundUniforms, config: &Checkerboard, time: Time) {
    value.common.kind = shader::BackgroundKind::Checkerboard;
    value.checker.color_a = config.color_a.value_at(time).to_srgba();
    value.checker.color_b = config.color_b.value_at(time).to_srgba();
    value.checker.cell_size = config.cell_size.value_at(time).to_array();
    value.checker.edge_softness = config.edge_softness.value_at(time);
    value.checker.position = config.position.value_at(time).to_array();
    value.checker.rotation = config.rotation_degrees.value_at(time);
}

fn voronoi(value: &mut shader::BackgroundUniforms, config: &Voronoi, time: Time) {
    value.common.kind = shader::BackgroundKind::Voronoi;
    value.voronoi.fill = match config.fill.value_at(time) {
        VoronoiFill::Distance => 0,
        VoronoiFill::Cells => 1,
        VoronoiFill::Edges => 2,
    };
    value.voronoi.metric = match config.metric.value_at(time) {
        VoronoiMetric::Euclidean => 0,
        VoronoiMetric::Manhattan => 1,
        VoronoiMetric::Chebyshev => 2,
    };
    value.voronoi.seed = config.seed.value_at(time);
    value.voronoi.cell_size = config.cell_size.value_at(time);
    value.voronoi.color_a = config.color_a.value_at(time).to_srgba();
    value.voronoi.color_b = config.color_b.value_at(time).to_srgba();
    value.voronoi.edge_color = config.edge_color.value_at(time).to_srgba();
    value.voronoi.jitter = config.jitter.value_at(time);
    value.voronoi.edge_width = config.edge_width.value_at(time);
    value.voronoi.position = config.position.value_at(time).to_array();
    value.voronoi.motion_amount = config.motion_amount.value_at(time);
    value.voronoi.motion_position = config.motion_position.value_at(time);
}

struct Buffer {
    device: ash::Device,
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: u64,
}

impl Buffer {
    fn new(context: &RenderContext, size: u64) -> Result<Self, String> {
        let buffer = unsafe {
            context.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }
        .map_err(|error| format!("create background uniform buffer: {error:?}"))?;
        let requirements = unsafe { context.device.get_buffer_memory_requirements(buffer) };
        let flags = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        let memory_type = (0..context.memory_properties.memory_type_count)
            .find(|index| {
                requirements.memory_type_bits & (1 << index) != 0
                    && context.memory_properties.memory_types[*index as usize]
                        .property_flags
                        .contains(flags)
            })
            .ok_or_else(|| "no host-visible Vulkan memory for background uniforms".to_string())?;
        let memory = unsafe {
            context.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(memory_type),
                None,
            )
        }
        .map_err(|error| format!("allocate background uniform memory: {error:?}"))?;
        unsafe { context.device.bind_buffer_memory(buffer, memory, 0) }
            .map_err(|error| format!("bind background uniform memory: {error:?}"))?;
        Ok(Self {
            device: context.device.clone(),
            buffer,
            memory,
            size,
        })
    }

    fn write<T>(&self, values: &[T]) -> Result<(), String> {
        let bytes = size_of_val(values) as u64;
        if bytes > self.size {
            return Err("background uniform upload exceeds allocation".to_string());
        }
        let mapped = unsafe {
            self.device
                .map_memory(self.memory, 0, bytes, vk::MemoryMapFlags::empty())
        }
        .map_err(|error| format!("map background uniform memory: {error:?}"))?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr().cast::<u8>(),
                mapped.cast(),
                bytes as usize,
            );
            self.device.unmap_memory(self.memory);
        }
        Ok(())
    }

    fn descriptor(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buffer,
            offset: 0,
            range: self.size,
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
