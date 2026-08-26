use cuda_core::DeviceCopy;

pub const MAX_STAGE_INPUTS: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, DeviceCopy)]
pub struct ImageDescriptor {
    pub pixels: *const [f32; 4],
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, DeviceCopy)]
pub struct ConvolutionTerm {
    pub weights: [f32; 16],
    pub offset_x: f32,
    pub offset_y: f32,
    pub input: u32,
    pub activation: u32,
}

#[repr(C)]
#[derive(Clone, Copy, DeviceCopy)]
pub struct ConvolutionParams {
    pub images: [ImageDescriptor; MAX_STAGE_INPUTS],
    pub bias: [f32; 4],
    pub residual: ImageDescriptor,
    pub result_scale: f32,
    pub residual_scale: f32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, DeviceCopy)]
pub struct AlphaParams {
    pub source: *const [f32; 4],
    pub alpha_source: *const u8,
    pub alpha_pitch: usize,
    pub alpha_width: u32,
    pub alpha_height: u32,
    pub width: u32,
    pub height: u32,
}
