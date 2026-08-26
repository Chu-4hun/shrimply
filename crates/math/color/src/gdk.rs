use crate::Color;

impl From<::gdk::RGBA> for Color<f32> {
    fn from(color: ::gdk::RGBA) -> Self {
        Self::new(color.red(), color.green(), color.blue(), color.alpha())
    }
}

impl From<::gdk::RGBA> for Color<u8> {
    fn from(color: ::gdk::RGBA) -> Self {
        Self::from_srgba(Color::<f32>::from(color).to_array())
    }
}

impl From<Color<f32>> for ::gdk::RGBA {
    fn from(color: Color<f32>) -> Self {
        Self::new(color.r, color.g, color.b, color.a)
    }
}

impl From<Color<u8>> for ::gdk::RGBA {
    fn from(color: Color<u8>) -> Self {
        Color::<f32>::from(color).into()
    }
}
