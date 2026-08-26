use shrimply_math_color::Color;
use skia_safe::{AlphaType, ColorType, ImageInfo, surfaces};

const FONT_SCALE: f32 = 0.06;
const MIN_FONT_SIZE: f32 = 18.0;
const MAX_FONT_SIZE: f32 = 72.0;

pub fn render(
    width: u32,
    height: u32,
    text: &str,
    background: Color<u8>,
    foreground: Color<u8>,
) -> Result<Vec<u8>, String> {
    let surface_width = i32::try_from(width).map_err(|_| "loading screen width is too large")?;
    let surface_height = i32::try_from(height).map_err(|_| "loading screen height is too large")?;
    let mut surface = surfaces::raster_n32_premul((surface_width, surface_height))
        .ok_or_else(|| "could not allocate loading screen".to_string())?;
    surface.canvas().clear(skia_safe::Color::from(background));
    let font_size =
        (surface_height.min(surface_width) as f32 * FONT_SCALE).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
    shrimply_preview_skia::text(
        surface.canvas(),
        glam::Vec2::new(surface_width as f32 * 0.5, surface_height as f32 * 0.5),
        text,
        font_size,
        foreground.into(),
    );

    let row_bytes = width as usize * 4;
    let mut pixels = vec![0; row_bytes * height as usize];
    let image_info = ImageInfo::new(
        (surface_width, surface_height),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    if !surface.read_pixels(&image_info, &mut pixels, row_bytes, (0, 0)) {
        return Err("could not read loading screen pixels".to_string());
    }
    Ok(pixels)
}
