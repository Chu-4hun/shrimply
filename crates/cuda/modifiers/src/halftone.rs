use cuda_core::CudaContext;
use cuda_device::{DisjointSlice, cuda_module, kernel, thread};
use cuda_host::EmbeddedModuleError;
use shrimply_render_core::HalftoneParams;
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
    const RGB_MODE: u32 = 1;
    const CMYK_MODE: u32 = 2;
    const TRIANGLE_HALF: f32 = 0.5;
    const TRIANGLE_HEIGHT_THIRD: f32 = 0.288_675_13;
    const TRIANGLE_HEIGHT_TWO_THIRDS: f32 = 0.577_350_26;
    const SQUARE_HALF: f32 = 0.5;
    const YELLOW_ANGLE_SCALE: f32 = 1.5;

    #[kernel]
    pub fn halftone(
        input: *const u32,
        width: u32,
        mut out: DisjointSlice<u32>,
        params: HalftoneParams,
    ) {
        let idx = thread::index_1d();
        let i = idx.get();
        let Some(output) = out.get_mut(idx) else {
            return;
        };
        let color = math::Color::from_rgba_u32(unsafe { *input.add(i) });
        let x = i as u32 % width;
        let pixel_y = i as u32 / width;
        let cell = params.size.max(1.0);
        if params.mode == RGB_MODE {
            let separation = params.channel_offset.max(0.0);
            let red = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle - params.channel_angle_offset,
                -separation * TRIANGLE_HALF,
                -separation * TRIANGLE_HEIGHT_THIRD,
                color.r,
                params.contrast,
            );
            let green = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle + params.channel_angle_offset,
                separation * TRIANGLE_HALF,
                -separation * TRIANGLE_HEIGHT_THIRD,
                color.g,
                params.contrast,
            );
            let blue = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle,
                0.0,
                separation * TRIANGLE_HEIGHT_TWO_THIRDS,
                color.b,
                params.contrast,
            );
            *output = math::Color::new(red, green, blue, color.a).to_rgba_u32();
        } else if params.mode == CMYK_MODE {
            let separation = params.channel_offset.max(0.0);
            let [cyan_amount, magenta_amount, yellow_amount, key_amount] = color.to_cmyk();
            let cyan = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle - params.channel_angle_offset,
                -separation * SQUARE_HALF,
                -separation * SQUARE_HALF,
                cyan_amount,
                params.contrast,
            );
            let magenta = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle + params.channel_angle_offset,
                separation * SQUARE_HALF,
                -separation * SQUARE_HALF,
                magenta_amount,
                params.contrast,
            );
            let yellow = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle - params.channel_angle_offset * YELLOW_ANGLE_SCALE,
                -separation * SQUARE_HALF,
                separation * SQUARE_HALF,
                yellow_amount,
                params.contrast,
            );
            let key = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle,
                separation * SQUARE_HALF,
                separation * SQUARE_HALF,
                key_amount,
                params.contrast,
            );
            *output = math::Color::new(
                (1.0 - cyan) * (1.0 - key),
                (1.0 - magenta) * (1.0 - key),
                (1.0 - yellow) * (1.0 - key),
                color.a,
            )
            .to_rgba_u32();
        } else {
            let value = channel_dot(
                x as f32,
                pixel_y as f32,
                cell,
                params.angle,
                0.0,
                0.0,
                color.rec709_luma(),
                params.contrast,
            );
            *output = math::Color::new(value, value, value, color.a).to_rgba_u32();
        }
    }

    fn channel_dot(
        x: f32,
        y: f32,
        cell: f32,
        angle: f32,
        offset_x: f32,
        offset_y: f32,
        channel: f32,
        contrast: f32,
    ) -> f32 {
        let radians = angle.to_radians();
        let cell_x = (x * radians.cos() - y * radians.sin() - offset_x) / cell;
        let cell_y = (x * radians.sin() + y * radians.cos() - offset_y) / cell;
        let px = cell_x - math::floor_f32(cell_x) - 0.5;
        let py = cell_y - math::floor_f32(cell_y) - 0.5;
        let distance = (px * px + py * py).sqrt() * 2.0;
        let value = ((channel - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
        if distance < value.sqrt() { 1.0 } else { 0.0 }
    }
}
