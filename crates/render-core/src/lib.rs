pub mod math;

pub use shrimply_math_color::{Color, ColorCorrectionParams, LayerBlendMode};

#[derive(Clone, Copy)]
pub struct AffineStabilizationParams {
    pub input: *const u32,
    pub width: u32,
    pub height: u32,
    pub source_transform: glam::Mat3,
}

#[derive(Clone, Copy)]
pub struct CornerPinParams {
    pub input: *const u32,
    pub width: u32,
    pub height: u32,
    pub inverse_homography: glam::Mat3,
    pub corners: [glam::Vec2; 4],
    pub perspective: f32,
}

#[derive(Clone, Copy)]
pub struct LayerCompositeParams {
    pub source: *const u32,
    pub clipping_base: *const u32,
    pub source_pitch: usize,
    pub clipping_base_pitch: usize,
    pub width: u32,
    pub mode: LayerBlendMode,
    pub opacity: f32,
    pub clipping_base_opacity: f32,
    pub noise_seed: u32,
}

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum DitheringPattern {
    Bayer2x2,
    Bayer4x4,
    Bayer8x8,
}

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum DitheringColorMode {
    Color,
    Grayscale,
    Palette,
}

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum MorphologyOperation {
    Erode,
    Dilate,
}

#[derive(Clone, Copy)]
pub struct BulgePinchParams {
    pub center: glam::Vec2,
    pub radius: f32,
    pub strength: f32,
}

#[derive(Clone, Copy)]
pub struct ChannelMixerParams {
    pub matrix: glam::Mat3,
}

#[derive(Clone, Copy)]
pub struct ChromaticAberrationParams {
    pub offsets: [f32; 4],
}

#[derive(Clone, Copy)]
pub struct ColorizeDuotoneParams {
    pub shadow: Color,
    pub highlight: Color,
}

#[derive(Clone, Copy)]
pub struct RadialBlurParams {
    pub center: glam::Vec2,
    pub angle: f32,
    pub samples: u32,
}

#[derive(Clone, Copy)]
pub struct ZoomBlurParams {
    pub center: glam::Vec2,
    pub strength: f32,
    pub samples: u32,
}

#[derive(Clone, Copy)]
pub struct DitheringParams {
    pub pattern: DitheringPattern,
    pub color_mode: DitheringColorMode,
    pub levels: f32,
    pub amount: f32,
    pub palette: *const u32,
    pub palette_len: u32,
}

#[derive(Clone, Copy)]
pub struct DropShadowParams {
    pub offset: glam::Vec2,
    pub radius: u32,
    pub color: u32,
}

#[derive(Clone, Copy)]
pub struct EdgeDetectionParams {
    pub amount: f32,
    pub edge: Color,
    pub background: Color,
}

#[derive(Clone, Copy)]
pub struct KaleidoscopeParams {
    pub center: glam::Vec2,
    pub segments: u32,
    pub rotation: f32,
}

#[derive(Clone, Copy)]
pub struct MaskParams {
    pub mask: *const u32,
    pub input_width: u32,
    pub mask_width: u32,
    pub mask_height: u32,
    pub transform: glam::Mat3,
    pub luminance: bool,
    pub invert: bool,
}

#[derive(Clone, Copy)]
pub struct AlphaMaskParams {
    pub mask: *const u32,
    pub input_width: u32,
    pub input_height: u32,
    pub mask_width: u32,
    pub mask_height: u32,
}

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum ShapeAlphaMaskKind {
    Rectangle,
    Ellipse,
    Polygon,
}

#[derive(Clone, Copy)]
pub struct ShapeAlphaMaskParams {
    pub base: *const u32,
    pub input_width: u32,
    pub canvas_to_local: glam::Mat3,
    pub local_width: f32,
    pub local_height: f32,
    pub center: glam::Vec2,
    pub size: glam::Vec2,
    pub rotation_degrees: f32,
    pub feather: f32,
    pub rounding: f32,
    pub shape: ShapeAlphaMaskKind,
    pub vertices: *const glam::Vec2,
    pub vertex_count: u32,
    pub invert: bool,
}

#[derive(Clone, Copy)]
pub struct ScanlinesCrtParams {
    pub spacing: f32,
    pub intensity: f32,
    pub curvature: f32,
    pub mask: f32,
}

#[derive(Clone, Copy)]
pub struct HalftoneParams {
    pub size: f32,
    pub angle: f32,
    pub contrast: f32,
    pub mode: u32,
    pub channel_offset: f32,
    pub channel_angle_offset: f32,
}

#[derive(Clone, Copy)]
pub struct ThresholdParams {
    pub threshold: f32,
    pub low: Color,
    pub high: Color,
}

#[derive(Clone, Copy)]
pub struct TwirlParams {
    pub center: glam::Vec2,
    pub radius: f32,
    pub angle: f32,
}

#[derive(Clone, Copy)]
pub struct WaveRippleParams {
    pub amplitude: f32,
    pub wavelength: f32,
    pub angle: f32,
    pub phase: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoSampleMethod {
    Nearest,
    Bilinear,
    #[default]
    Bicubic,
    Mitchell,
    Lanczos2,
    Fsr1Easu,
    NvidiaImageScaling,
    Anime4k,
    Anime4kSrgan,
    Xbrz,
    Lanczos3,
}

#[derive(Clone, Copy)]
pub enum TextureAddressMode {
    Transparent,
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    BlurredMirror,
    Stochastic,
}

#[derive(Clone, Copy)]
pub enum LayerKind {
    Nv12,
    Rgba,
}

#[derive(Clone, Copy)]
pub struct Nv12LayerParams {
    pub kind: LayerKind,
    pub y_plane: *const u8,
    pub uv_plane: *const u8,
    pub rgba: *const u32,
    pub y_pitch: usize,
    pub uv_pitch: usize,
    pub rgba_pitch: usize,
    pub source_width: u32,
    pub source_height: u32,
    pub canvas_width: u32,
    pub inverse: glam::Mat3,
    pub motion_transform_offset: u32,
    pub motion_transform_count: u32,
    pub motion_sample_count: u32,
    pub opacity: f32,
    pub sample_method: VideoSampleMethod,
    pub blend_mode: LayerBlendMode,
    pub crop: [f32; 4],
    pub padding: [f32; 4],
    pub address_mode: TextureAddressMode,
}

#[derive(Clone, Copy)]
pub struct ChromaKeyParams {
    pub key: Color,
    pub similarity: f32,
    pub softness: f32,
    pub spill: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VisualTransitionMaskKind {
    Wipe,
    Iris,
    Dissolve,
    ClockWipe,
    TriangularFold,
    StreakWipe,
}

#[derive(Clone, Copy)]
pub struct VisualTransitionMaskParams {
    pub kind: VisualTransitionMaskKind,
    pub visibility: f32,
    pub angle_degrees: f32,
    pub softness: f32,
    pub center: glam::Vec2,
    pub grain_size: u32,
    pub line_variation: f32,
}

#[derive(Clone, Copy)]
pub struct Sam2ProxyParams {
    pub input_width: u32,
    pub input_height: u32,
    pub model_size: u32,
}

#[derive(Clone, Copy)]
pub struct Sam2MaskParams {
    pub output_width: u32,
    pub output_height: u32,
    pub mask_size: u32,
    pub threshold: f32,
    pub softness: f32,
    pub invert: bool,
    pub quantization_scale: f32,
}

#[derive(Clone, Copy)]
pub struct TransparentFillMaskParams {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}
