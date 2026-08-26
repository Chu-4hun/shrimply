#![feature(proc_macro_hygiene)]

use cuda_device::{cuda_module, kernel, thread};

#[cuda_module]
mod device {
    use super::*;

    #[kernel]
    pub fn rgba_to_nv12_luma(
        rgba: *const u32,
        width: u32,
        height: u32,
        y_plane: *mut u8,
        y_pitch: usize,
    ) {
        let i = thread::index_1d().get();
        if i >= width as usize * height as usize {
            return;
        }
        let pixel = load_pixel(rgba, width, i as u32 % width, i as u32 / width);
        unsafe {
            *y_plane.add(i / width as usize * y_pitch + i % width as usize) =
                pixel.to_bt709_ycbcr()[0];
        }
    }

    #[kernel]
    pub fn rgba_to_nv12_chroma(
        rgba: *const u32,
        width: u32,
        height: u32,
        uv_plane: *mut u8,
        uv_pitch: usize,
    ) {
        let cw = width.div_ceil(2).max(1);
        let ch = height.div_ceil(2).max(1);
        let i = thread::index_1d().get();
        if i >= cw as usize * ch as usize {
            return;
        }
        let cx = i as u32 % cw;
        let cy = i as u32 / cw;
        let pixel = average_block(rgba, width, height, cx, cy);
        let offset = cy as usize * uv_pitch + cx as usize * 2;
        let ycbcr = pixel.to_bt709_ycbcr();
        unsafe {
            *uv_plane.add(offset) = ycbcr[1];
            *uv_plane.add(offset + 1) = ycbcr[2];
        }
    }

    #[kernel]
    pub fn rgba_to_p010_luma(
        rgba: *const u32,
        width: u32,
        height: u32,
        y_plane: *mut u16,
        y_pitch: usize,
    ) {
        let i = thread::index_1d().get();
        if i >= width as usize * height as usize {
            return;
        }
        let x = i as u32 % width;
        let y = i as u32 / width;
        let row = y_pitch / core::mem::size_of::<u16>();
        unsafe {
            *y_plane.add(y as usize * row + x as usize) =
                (load_pixel(rgba, width, x, y).to_bt709_ycbcr()[0] as u16) << 8;
        }
    }

    #[kernel]
    pub fn rgba_to_p010_chroma(
        rgba: *const u32,
        width: u32,
        height: u32,
        uv_plane: *mut u16,
        uv_pitch: usize,
    ) {
        let cw = width.div_ceil(2).max(1);
        let ch = height.div_ceil(2).max(1);
        let i = thread::index_1d().get();
        if i >= cw as usize * ch as usize {
            return;
        }
        let cx = i as u32 % cw;
        let cy = i as u32 / cw;
        let pixel = average_block(rgba, width, height, cx, cy);
        let row = uv_pitch / core::mem::size_of::<u16>();
        let offset = cy as usize * row + cx as usize * 2;
        let ycbcr = pixel.to_bt709_ycbcr();
        unsafe {
            *uv_plane.add(offset) = (ycbcr[1] as u16) << 8;
            *uv_plane.add(offset + 1) = (ycbcr[2] as u16) << 8;
        }
    }

    use shrimply_math_color::Color;

    fn load_pixel(rgba: *const u32, width: u32, x: u32, y: u32) -> Color<f32> {
        if rgba.is_null() {
            return Color::<f32>::BLACK;
        }
        Color::from_rgba_u32(unsafe { *rgba.add(y as usize * width as usize + x as usize) })
    }

    fn average_block(rgba: *const u32, width: u32, height: u32, cx: u32, cy: u32) -> Color<f32> {
        let mut out = Color::<f32>::TRANSPARENT;
        let mut count = 0.0;
        for y in cy * 2..(cy * 2 + 2).min(height) {
            for x in cx * 2..(cx * 2 + 2).min(width) {
                let p = load_pixel(rgba, width, x, y);
                out.r += p.r;
                out.g += p.g;
                out.b += p.b;
                count += 1.0;
            }
        }
        Color::new(out.r / count, out.g / count, out.b / count, 1.0)
    }
}

fn main() {}
