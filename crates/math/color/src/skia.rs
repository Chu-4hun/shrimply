use crate::Color;

impl From<Color<u8>> for skia_safe::Color {
    fn from(color: Color<u8>) -> Self {
        Self::from_argb(color.a, color.r, color.g, color.b)
    }
}

impl From<Color<f32>> for skia_safe::Color {
    fn from(color: Color<f32>) -> Self {
        let color = Color::<u8>::from_srgba(color.to_array());
        color.into()
    }
}

impl From<Color<f32>> for skia_safe::Color4f {
    fn from(color: Color<f32>) -> Self {
        Self::new(color.r, color.g, color.b, color.a)
    }
}
