use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use std::sync::Arc;

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
    pub fn visual_transition_mask(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        params: shrimply_render_core::VisualTransitionMaskParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let pixel = unsafe { *input.add(i) };
        let x = (i as u32 % width) as f32;
        let y = (i as u32 / width) as f32;
        let visibility = params.visibility.clamp(0.0, 1.0);
        let alpha = match params.kind {
            shrimply_render_core::VisualTransitionMaskKind::Wipe => wipe_alpha(
                x,
                y,
                width,
                height,
                visibility,
                params.angle_degrees,
                params.softness,
            ),
            shrimply_render_core::VisualTransitionMaskKind::Iris => iris_alpha(
                x,
                y,
                width,
                height,
                visibility,
                params.center.x,
                params.center.y,
                params.softness,
                params.grain_size != 0,
            ),
            shrimply_render_core::VisualTransitionMaskKind::Dissolve => {
                dissolve_alpha(x as u32, y as u32, visibility, params.grain_size)
            }
            shrimply_render_core::VisualTransitionMaskKind::ClockWipe => clock_wipe_alpha(
                x,
                y,
                visibility,
                params.angle_degrees,
                params.softness,
                params.center.x,
                params.center.y,
                params.grain_size != 0,
            ),
            shrimply_render_core::VisualTransitionMaskKind::TriangularFold => {
                *output = triangular_fold_pixel(
                    input,
                    x,
                    y,
                    width,
                    height,
                    visibility,
                    params.angle_degrees,
                    params.softness,
                    params.grain_size,
                );
                return;
            }
            shrimply_render_core::VisualTransitionMaskKind::StreakWipe => math::streak_wipe_alpha(
                math::Vec2::new(x + 0.5, y + 0.5),
                math::Vec2::new(width.max(1) as f32, height.max(1) as f32),
                visibility,
                params.angle_degrees,
                params.grain_size as f32,
                params.line_variation,
                params.softness,
            ),
        };
        let [r, g, b, a] = math::Color::from_rgba_u32(pixel).to_array();
        *output = math::Color::new(r, g, b, a * alpha).to_rgba_u32();
    }

    #[kernel]
    pub fn origami_transition(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        vertices: *const f32,
        grid: u32,
        visibility: f32,
    ) {
        let index = thread::index_1d();
        let i = index.get();
        let Some(output) = out.get_mut(index) else {
            return;
        };
        if visibility >= 0.999_9 {
            *output = unsafe { *input.add(i) };
            return;
        }
        if visibility <= 0.0 {
            *output = 0;
            return;
        }

        let point = math::Vec2::new(
            (i as u32 % width) as f32 + 0.5,
            (i as u32 / width) as f32 + 0.5,
        );
        let grid = grid.clamp(2, 6);
        let stride = grid + 1;
        let mut best = [f32::MIN, 0.0, 0.0, 1.0];
        let mut row = 0;
        while row < grid {
            let mut column = 0;
            while column < grid {
                let i00 = row * stride + column;
                let i10 = i00 + 1;
                let i01 = i00 + stride;
                let i11 = i01 + 1;
                let u0 = column as f32 / grid as f32;
                let u1 = (column + 1) as f32 / grid as f32;
                let v0 = row as f32 / grid as f32;
                let v1 = (row + 1) as f32 / grid as f32;
                let first = triangle_hit(
                    point,
                    vertex(vertices, i00),
                    vertex(vertices, i10),
                    vertex(vertices, i11),
                    math::Vec2::new(u0, v0),
                    math::Vec2::new(u1, v0),
                    math::Vec2::new(u1, v1),
                    visibility,
                );
                if first[0] > best[0] {
                    best = first;
                }
                let second = triangle_hit(
                    point,
                    vertex(vertices, i00),
                    vertex(vertices, i11),
                    vertex(vertices, i01),
                    math::Vec2::new(u0, v0),
                    math::Vec2::new(u1, v1),
                    math::Vec2::new(u0, v1),
                    visibility,
                );
                if second[0] > best[0] {
                    best = second;
                }
                column += 1;
            }
            row += 1;
        }

        if best[0] == f32::MIN {
            *output = 0;
            return;
        }
        let source = sample_bilinear(input, width, height, best[1], best[2]);
        let [r, g, b, a] = math::Color::from_rgba_u32(source).to_array();
        *output = math::Color::new(r * best[3], g * best[3], b * best[3], a).to_rgba_u32();
    }

    #[kernel]
    pub fn transition_gaussian_blur_horizontal(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        *output = unsafe { math::gaussian_horizontal_rgba(input, i, width, radius) };
    }

    #[kernel]
    pub fn transition_gaussian_blur_vertical(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        radius: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        *output = unsafe { math::gaussian_vertical_rgba(input, i, width, height, radius) };
    }

    #[kernel]
    pub fn transition_pixelate(
        input: *const u32,
        width: u32,
        height: u32,
        mut out: DisjointSlice<u32>,
        block_width: u32,
        block_height: u32,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let x = i as u32 % width;
        let y = i as u32 / width;
        let block_width = block_width.max(1);
        let block_height = block_height.max(1);
        let sample_x = (x / block_width * block_width + block_width / 2).min(width - 1);
        let sample_y = (y / block_height * block_height + block_height / 2).min(height - 1);
        *output = unsafe { *input.add((sample_y * width + sample_x) as usize) };
    }

    #[allow(clippy::too_many_arguments)]
    fn wipe_alpha(
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        visibility: f32,
        angle_degrees: f32,
        softness: f32,
    ) -> f32 {
        let radians = angle_degrees.to_radians();
        let direction_x = math::sin_f32(radians + core::f32::consts::FRAC_PI_2);
        let direction_y = math::sin_f32(radians);
        let nx = x / width.max(1) as f32 - 0.5;
        let ny = y / height.max(1) as f32 - 0.5;
        let extent = (direction_x.abs() + direction_y.abs()).max(0.000_01);
        let position = (nx * direction_x + ny * direction_y) / extent + 0.5;
        let softness = softness.clamp(0.0, 0.5);
        let edge = visibility * (1.0 + softness * 2.0) - softness;
        1.0 - math::smoothstep(edge - softness, edge + softness + 0.000_01, position)
    }

    #[allow(clippy::too_many_arguments)]
    fn iris_alpha(
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        visibility: f32,
        center_x: f32,
        center_y: f32,
        softness: f32,
        from_inside: bool,
    ) -> f32 {
        let width = width.max(1) as f32;
        let height = height.max(1) as f32;
        let center_x = center_x.clamp(0.0, width);
        let center_y = center_y.clamp(0.0, height);
        let dx = x - center_x;
        let dy = y - center_y;
        let far_x = center_x.max(width - center_x);
        let far_y = center_y.max(height - center_y);
        let maximum = (far_x * far_x + far_y * far_y).sqrt().max(0.000_01);
        let distance = (dx * dx + dy * dy).sqrt() / maximum;
        let distance = if from_inside {
            distance
        } else {
            1.0 - distance
        };
        let softness = softness.clamp(0.0, 0.5);
        let radius = visibility * (1.0 + softness * 2.0) - softness;
        1.0 - math::smoothstep(radius - softness, radius + softness + 0.000_01, distance)
    }

    fn dissolve_alpha(x: u32, y: u32, visibility: f32, grain_size: u32) -> f32 {
        let grain = grain_size.max(1);
        let mut hash = x / grain * 0x8da6_b343 ^ y / grain * 0xd816_3841 ^ 19;
        hash ^= hash >> 13;
        hash = hash.wrapping_mul(0x85eb_ca6b);
        hash ^= hash >> 16;
        ((hash & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32 <= visibility) as u32 as f32
    }

    #[allow(clippy::too_many_arguments)]
    fn clock_wipe_alpha(
        x: f32,
        y: f32,
        visibility: f32,
        start_angle_degrees: f32,
        softness: f32,
        center_x: f32,
        center_y: f32,
        clockwise: bool,
    ) -> f32 {
        let tau = core::f32::consts::TAU;
        let start = start_angle_degrees.to_radians();
        let mut angle = (math::atan2_f32(y - center_y, x - center_x) - start) % tau;
        if angle < 0.0 {
            angle += tau;
        }
        if !clockwise && angle > 0.0 {
            angle = tau - angle;
        }
        let position = angle / tau;
        let softness = softness.clamp(0.0, 0.25);
        let edge = visibility * (1.0 + softness * 2.0) - softness;
        1.0 - math::smoothstep(edge - softness, edge + softness + 0.000_01, position)
    }

    #[allow(clippy::too_many_arguments)]
    fn triangular_fold_pixel(
        input: *const u32,
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        visibility: f32,
        angle_degrees: f32,
        depth: f32,
        fold_size: u32,
    ) -> u32 {
        let width_f = width.max(1) as f32;
        let height_f = height.max(1) as f32;
        if visibility >= 0.999_9 {
            return unsafe { *input.add((y as u32 * width + x as u32) as usize) };
        }

        let cell_size = fold_size.clamp(32, 512) as f32;
        let cell_x = math::floor_f32(x / cell_size) as i32;
        let cell_y = math::floor_f32(y / cell_size) as i32;
        let origin_x = cell_x as f32 * cell_size;
        let origin_y = cell_y as f32 * cell_size;
        let u = ((x - origin_x) / cell_size).clamp(0.0, 1.0);
        let v = ((y - origin_y) / cell_size).clamp(0.0, 1.0);

        let radians = angle_degrees.to_radians();
        let direction_x = math::sin_f32(radians + core::f32::consts::FRAC_PI_2);
        let direction_y = math::sin_f32(radians);
        let normalized_x = (origin_x + cell_size * 0.5) / width_f - 0.5;
        let normalized_y = (origin_y + cell_size * 0.5) / height_f - 0.5;
        let extent = (direction_x.abs() + direction_y.abs()).max(0.000_01);
        let sweep = (normalized_x * direction_x + normalized_y * direction_y) / extent + 0.5;

        let backslash = (cell_x + cell_y) & 1 == 0;
        let diagonal = if backslash { u - v } else { u + v - 1.0 };
        let side_delay = if diagonal >= 0.0 { 0.07 } else { 0.0 };
        let jitter = math::hash_unit(cell_x, cell_y, 29);
        let delay = (sweep.clamp(0.0, 1.0) * 0.55 + jitter * 0.14 + side_delay).min(0.82);
        let panel_progress = ((visibility - delay) / (1.0 - delay).max(0.000_01)).clamp(0.0, 1.0);
        if panel_progress <= 0.0 {
            return 0;
        }
        let fold = math::smoothstep(0.0, 1.0, panel_progress);
        let distance = diagonal.abs();
        if distance > fold {
            return 0;
        }

        let source_diagonal = diagonal / fold.max(0.000_1);
        let adjustment = (source_diagonal - diagonal) * 0.5;
        let (source_u, source_v) = if backslash {
            (u + adjustment, v - adjustment)
        } else {
            (u + adjustment, v + adjustment)
        };
        let source_x =
            (origin_x + source_u.clamp(0.0, 1.0) * cell_size).clamp(0.0, width_f - 1.0) as u32;
        let source_y =
            (origin_y + source_v.clamp(0.0, 1.0) * cell_size).clamp(0.0, height_f - 1.0) as u32;
        let source = unsafe { *input.add((source_y * width + source_x) as usize) };
        let [r, g, b, a] = math::Color::from_rgba_u32(source).to_array();

        let fold_height = math::sin_f32(fold * core::f32::consts::PI).max(0.0);
        let side_light = if diagonal >= 0.0 { 1.0 } else { -0.65 };
        let crease = 1.0 - math::smoothstep(0.0, 0.045, distance);
        let moving_edge = math::smoothstep((fold - 0.08).max(0.0), fold, distance);
        let depth = depth.clamp(0.0, 1.0);
        let shade =
            (1.0 + side_light * fold_height * depth * 0.34 + crease * fold_height * depth * 0.22
                - moving_edge * fold_height * depth * 0.42)
                .clamp(0.28, 1.55);
        let coverage = 1.0 - math::smoothstep((fold - 0.025).max(0.0), fold + 0.000_01, distance);
        math::Color::new(r * shade, g * shade, b * shade, a * coverage).to_rgba_u32()
    }

    fn vertex(vertices: *const f32, index: u32) -> [f32; 3] {
        let offset = index as usize * 3;
        unsafe {
            [
                *vertices.add(offset),
                *vertices.add(offset + 1),
                *vertices.add(offset + 2),
            ]
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn triangle_hit(
        point: math::Vec2,
        a: [f32; 3],
        b: [f32; 3],
        c: [f32; 3],
        uv_a: math::Vec2,
        uv_b: math::Vec2,
        uv_c: math::Vec2,
        visibility: f32,
    ) -> [f32; 4] {
        let denominator = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if denominator.abs() < 0.000_01 {
            return [f32::MIN, 0.0, 0.0, 1.0];
        }
        let wa =
            ((b[1] - c[1]) * (point.x - c[0]) + (c[0] - b[0]) * (point.y - c[1])) / denominator;
        let wb =
            ((c[1] - a[1]) * (point.x - c[0]) + (a[0] - c[0]) * (point.y - c[1])) / denominator;
        let wc = 1.0 - wa - wb;
        if wa < -0.000_5 || wb < -0.000_5 || wc < -0.000_5 {
            return [f32::MIN, 0.0, 0.0, 1.0];
        }

        let edge_ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let edge_ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let normal = [
            edge_ab[1] * edge_ac[2] - edge_ab[2] * edge_ac[1],
            edge_ab[2] * edge_ac[0] - edge_ab[0] * edge_ac[2],
            edge_ab[0] * edge_ac[1] - edge_ab[1] * edge_ac[0],
        ];
        let normal_length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
            .sqrt()
            .max(0.000_01);
        let light = (normal[0] * 0.32 - normal[1] * 0.38 + normal[2] * 0.87) / normal_length;
        let fold = 1.0 - visibility;
        let crease = 1.0 - math::smoothstep(0.0, 0.035, wa.min(wb).min(wc));
        let mut folded_shade = 0.84 + light.abs() * 0.16;
        folded_shade += (1.0 - folded_shade) * crease * 0.7;
        if normal[2] < 0.0 {
            folded_shade *= 0.92;
        }
        let shade = 1.0 - (1.0 - folded_shade) * fold;
        [
            wa * a[2] + wb * b[2] + wc * c[2],
            wa * uv_a.x + wb * uv_b.x + wc * uv_c.x,
            wa * uv_a.y + wb * uv_b.y + wc * uv_c.y,
            shade.clamp(0.78, 1.0),
        ]
    }

    fn sample_bilinear(input: *const u32, width: u32, height: u32, u: f32, v: f32) -> u32 {
        let x = u.clamp(0.0, 1.0) * width.saturating_sub(1) as f32;
        let y = v.clamp(0.0, 1.0) * height.saturating_sub(1) as f32;
        unsafe { math::sample_bilinear_rgba(input, width, height, x, y) }
    }
}
