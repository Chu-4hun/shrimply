use core::f32::consts::PI;

use crate::math;

use shrimply_math_color::Color;

use super::{Nv12LayerParams, TextureAddressMode, VideoSampleMethod};

mod fsr;
mod nis;
mod xbrz;

#[derive(Clone, Copy)]
pub(super) struct Nv12Sample {
    pub(super) luma: f32,
    pub(super) cb: f32,
    pub(super) cr: f32,
    pub(super) alpha: f32,
}

impl From<Nv12Sample> for Color<f32> {
    fn from(sample: Nv12Sample) -> Self {
        Self::from_bt709_ycbcr(sample.luma, sample.cb, sample.cr, sample.alpha)
    }
}

#[derive(Clone, Copy)]
enum ChromaComponent {
    Cb,
    Cr,
}

pub(super) fn sample_nv12(params: &Nv12LayerParams, source_x: f32, source_y: f32) -> Nv12Sample {
    match params.address_mode {
        TextureAddressMode::BlurredMirror => {
            let distance = outside_distance(params, source_x, source_y);
            if distance > 0.0 {
                return sample_nv12_blurred(params, source_x, source_y, distance);
            }
        }
        TextureAddressMode::Stochastic => {
            let distance = outside_distance(params, source_x, source_y);
            if distance > 0.0 {
                return sample_nv12_stochastic(params, source_x, source_y, distance);
            }
        }
        _ => {}
    }
    sample_nv12_plain(params, source_x, source_y)
}

fn sample_nv12_plain(params: &Nv12LayerParams, source_x: f32, source_y: f32) -> Nv12Sample {
    let luma_x = source_x - 0.5;
    let luma_y = source_y - 0.5;
    let chroma_x = (luma_x - 0.5) * 0.5;
    let chroma_y = (luma_y - 0.5) * 0.5;
    let alpha = sample_coverage(params, luma_x, luma_y);
    if matches!(params.sample_method, VideoSampleMethod::Fsr1Easu) {
        return fsr::sample_nv12(params, luma_x, luma_y, alpha);
    }
    let mut luma = sample_luma(params, luma_x, luma_y);
    let mut cb = sample_chroma(params, chroma_x, chroma_y, ChromaComponent::Cb) - 0.5;
    let mut cr = sample_chroma(params, chroma_x, chroma_y, ChromaComponent::Cr) - 0.5;
    if matches!(params.address_mode, TextureAddressMode::Transparent) && alpha > 0.000_001 {
        luma /= alpha;
        cb /= alpha;
        cr /= alpha;
    }
    Nv12Sample {
        luma,
        cb,
        cr,
        alpha,
    }
}

fn sample_coverage(params: &Nv12LayerParams, x: f32, y: f32) -> f32 {
    if !matches!(params.address_mode, TextureAddressMode::Transparent) {
        return 1.0;
    }
    let valid = |column, row| {
        (address_index(
            column,
            params.source_width,
            params.crop[3],
            params.crop[1],
            params.address_mode,
        )
        .is_some()
            && address_index(
                row,
                params.source_height,
                params.crop[0],
                params.crop[2],
                params.address_mode,
            )
            .is_some()) as u8 as f32
    };
    match params.sample_method {
        VideoSampleMethod::Nearest => valid(
            math::floor_f32(x + 0.5) as i32,
            math::floor_f32(y + 0.5) as i32,
        ),
        VideoSampleMethod::Bilinear => bilinear_coverage(x, y, valid),
        VideoSampleMethod::Fsr1Easu => bilinear_coverage(x, y, valid),
        VideoSampleMethod::NvidiaImageScaling => bilinear_coverage(x, y, valid),
        VideoSampleMethod::Anime4k | VideoSampleMethod::Anime4kSrgan => {
            bilinear_coverage(x, y, valid)
        }
        VideoSampleMethod::Xbrz => valid(
            math::floor_f32(x + 0.5) as i32,
            math::floor_f32(y + 0.5) as i32,
        ),
        method => sample_weighted(x, y, method, valid),
    }
    .clamp(0.0, 1.0)
}

pub(super) fn sample_rgba(params: &Nv12LayerParams, source_x: f32, source_y: f32) -> Color<f32> {
    let sample = match params.address_mode {
        TextureAddressMode::BlurredMirror => {
            let distance = outside_distance(params, source_x, source_y);
            if distance > 0.0 {
                sample_rgba_blurred(params, source_x, source_y, distance)
            } else {
                sample_rgba_plain(params, source_x, source_y)
            }
        }
        TextureAddressMode::Stochastic => {
            let distance = outside_distance(params, source_x, source_y);
            if distance > 0.0 {
                sample_rgba_stochastic(params, source_x, source_y, distance)
            } else {
                sample_rgba_plain(params, source_x, source_y)
            }
        }
        _ => sample_rgba_plain(params, source_x, source_y),
    };
    let alpha = sample.a.clamp(0.0, 1.0);
    Color::new(
        sample.r.clamp(0.0, alpha),
        sample.g.clamp(0.0, alpha),
        sample.b.clamp(0.0, alpha),
        alpha,
    )
    .unpremultiply()
}

fn sample_rgba_plain(params: &Nv12LayerParams, source_x: f32, source_y: f32) -> Color<f32> {
    let x = source_x - 0.5;
    let y = source_y - 0.5;
    match params.sample_method {
        VideoSampleMethod::Nearest => load_rgba(
            params,
            math::floor_f32(x + 0.5) as i32,
            math::floor_f32(y + 0.5) as i32,
        ),
        VideoSampleMethod::Fsr1Easu => fsr::sample_rgba(params, x, y),
        VideoSampleMethod::NvidiaImageScaling => nis::sample_rgba(params, x, y),
        VideoSampleMethod::Xbrz => xbrz::sample_rgba(params, x, y),
        VideoSampleMethod::Bicubic
        | VideoSampleMethod::Mitchell
        | VideoSampleMethod::Lanczos2
        | VideoSampleMethod::Lanczos3 => sample_rgba_weighted(params, x, y, params.sample_method),
        VideoSampleMethod::Bilinear
        | VideoSampleMethod::Anime4k
        | VideoSampleMethod::Anime4kSrgan => sample_rgba_bilinear(params, x, y),
    }
}

fn sample_nv12_blurred(
    params: &Nv12LayerParams,
    source_x: f32,
    source_y: f32,
    distance: f32,
) -> Nv12Sample {
    let radius = (distance * 0.2).min(12.0);
    let offset = radius * 0.707_106_77;
    let center = sample_nv12_plain(params, source_x, source_y);
    let a = sample_nv12_plain(params, source_x - offset, source_y - offset);
    let b = sample_nv12_plain(params, source_x + offset, source_y - offset);
    let c = sample_nv12_plain(params, source_x - offset, source_y + offset);
    let d = sample_nv12_plain(params, source_x + offset, source_y + offset);
    Nv12Sample {
        luma: center.luma * 0.4 + (a.luma + b.luma + c.luma + d.luma) * 0.15,
        cb: center.cb * 0.4 + (a.cb + b.cb + c.cb + d.cb) * 0.15,
        cr: center.cr * 0.4 + (a.cr + b.cr + c.cr + d.cr) * 0.15,
        alpha: center.alpha * 0.4 + (a.alpha + b.alpha + c.alpha + d.alpha) * 0.15,
    }
}

fn sample_rgba_blurred(
    params: &Nv12LayerParams,
    source_x: f32,
    source_y: f32,
    distance: f32,
) -> Color<f32> {
    let radius = (distance * 0.2).min(12.0);
    let offset = radius * 0.707_106_77;
    let center = sample_rgba_plain(params, source_x, source_y);
    let a = sample_rgba_plain(params, source_x - offset, source_y - offset);
    let b = sample_rgba_plain(params, source_x + offset, source_y - offset);
    let c = sample_rgba_plain(params, source_x - offset, source_y + offset);
    let d = sample_rgba_plain(params, source_x + offset, source_y + offset);
    Color::new(
        center.r * 0.4 + (a.r + b.r + c.r + d.r) * 0.15,
        center.g * 0.4 + (a.g + b.g + c.g + d.g) * 0.15,
        center.b * 0.4 + (a.b + b.b + c.b + d.b) * 0.15,
        center.a * 0.4 + (a.a + b.a + c.a + d.a) * 0.15,
    )
}

fn sample_nv12_stochastic(
    params: &Nv12LayerParams,
    source_x: f32,
    source_y: f32,
    distance: f32,
) -> Nv12Sample {
    let base = sample_nv12_plain(params, source_x, source_y);
    let samples = stochastic_samples(params, source_x, source_y);
    let a = sample_nv12_plain(params, samples[0].0, samples[0].1);
    let b = sample_nv12_plain(params, samples[1].0, samples[1].1);
    let c = sample_nv12_plain(params, samples[2].0, samples[2].1);
    let synthesized = Nv12Sample {
        luma: a.luma * samples[0].2 + b.luma * samples[1].2 + c.luma * samples[2].2,
        cb: a.cb * samples[0].2 + b.cb * samples[1].2 + c.cb * samples[2].2,
        cr: a.cr * samples[0].2 + b.cr * samples[1].2 + c.cr * samples[2].2,
        alpha: a.alpha * samples[0].2 + b.alpha * samples[1].2 + c.alpha * samples[2].2,
    };
    lerp_nv12(base, synthesized, stochastic_mix(params, distance))
}

fn sample_rgba_stochastic(
    params: &Nv12LayerParams,
    source_x: f32,
    source_y: f32,
    distance: f32,
) -> Color<f32> {
    let base = sample_rgba_plain(params, source_x, source_y);
    let samples = stochastic_samples(params, source_x, source_y);
    let a = sample_rgba_plain(params, samples[0].0, samples[0].1);
    let b = sample_rgba_plain(params, samples[1].0, samples[1].1);
    let c = sample_rgba_plain(params, samples[2].0, samples[2].1);
    let synthesized = Color::new(
        a.r * samples[0].2 + b.r * samples[1].2 + c.r * samples[2].2,
        a.g * samples[0].2 + b.g * samples[1].2 + c.g * samples[2].2,
        a.b * samples[0].2 + b.b * samples[1].2 + c.b * samples[2].2,
        a.a * samples[0].2 + b.a * samples[1].2 + c.a * samples[2].2,
    );
    base.lerp(synthesized, stochastic_mix(params, distance))
}

fn stochastic_samples(
    params: &Nv12LayerParams,
    source_x: f32,
    source_y: f32,
) -> [(f32, f32, f32); 3] {
    let (left, top, width, height) = crop_rect(params);
    let u = (source_x - left) / width;
    let v = (source_y - top) / height;
    let lattice_x = u - v * 0.577_350_26;
    let lattice_y = v * 1.154_700_5;
    let cell_x = math::floor_f32(lattice_x) as i32;
    let cell_y = math::floor_f32(lattice_y) as i32;
    let x = lattice_x - cell_x as f32;
    let y = lattice_y - cell_y as f32;
    let (vertices, weights) = if x + y < 1.0 {
        (
            [(cell_x, cell_y), (cell_x + 1, cell_y), (cell_x, cell_y + 1)],
            [1.0 - x - y, x, y],
        )
    } else {
        (
            [
                (cell_x + 1, cell_y + 1),
                (cell_x, cell_y + 1),
                (cell_x + 1, cell_y),
            ],
            [x + y - 1.0, 1.0 - x, 1.0 - y],
        )
    };
    let sample = |vertex: (i32, i32), weight: f32| {
        let offset_x = (math::hash_unit(vertex.0, vertex.1, 0) - 0.5) * width;
        let offset_y = (math::hash_unit(vertex.0, vertex.1, 1) - 0.5) * height;
        (source_x + offset_x, source_y + offset_y, weight)
    };
    [
        sample(vertices[0], weights[0]),
        sample(vertices[1], weights[1]),
        sample(vertices[2], weights[2]),
    ]
}

fn stochastic_mix(params: &Nv12LayerParams, distance: f32) -> f32 {
    let (_, _, width, height) = crop_rect(params);
    math::smoothstep(0.0, (width.min(height) * 0.05).clamp(8.0, 32.0), distance)
}

fn outside_distance(params: &Nv12LayerParams, source_x: f32, source_y: f32) -> f32 {
    let (left, top, width, height) = crop_rect(params);
    let right = left + width;
    let bottom = top + height;
    let x = (left - source_x).max(source_x - right).max(0.0);
    let y = (top - source_y).max(source_y - bottom).max(0.0);
    (x * x + y * y).sqrt()
}

fn crop_rect(params: &Nv12LayerParams) -> (f32, f32, f32, f32) {
    let left = params.source_width as f32 * params.crop[3];
    let top = params.source_height as f32 * params.crop[0];
    let width = (params.source_width as f32 * (1.0 - params.crop[1] - params.crop[3])).max(1.0);
    let height = (params.source_height as f32 * (1.0 - params.crop[0] - params.crop[2])).max(1.0);
    (left, top, width, height)
}

fn lerp_nv12(a: Nv12Sample, b: Nv12Sample, amount: f32) -> Nv12Sample {
    Nv12Sample {
        luma: math::lerp(a.luma, b.luma, amount),
        cb: math::lerp(a.cb, b.cb, amount),
        cr: math::lerp(a.cr, b.cr, amount),
        alpha: math::lerp(a.alpha, b.alpha, amount),
    }
}

fn bilinear_coverage(x: f32, y: f32, valid: impl Fn(i32, i32) -> f32) -> f32 {
    let x0 = math::floor_f32(x) as i32;
    let y0 = math::floor_f32(y) as i32;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    math::lerp(
        math::lerp(valid(x0, y0), valid(x0 + 1, y0), tx),
        math::lerp(valid(x0, y0 + 1), valid(x0 + 1, y0 + 1), tx),
        ty,
    )
}

fn sample_rgba_bilinear(params: &Nv12LayerParams, x: f32, y: f32) -> Color<f32> {
    let x0 = math::floor_f32(x) as i32;
    let y0 = math::floor_f32(y) as i32;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let top = load_rgba(params, x0, y0).lerp(load_rgba(params, x0 + 1, y0), tx);
    let bottom = load_rgba(params, x0, y0 + 1).lerp(load_rgba(params, x0 + 1, y0 + 1), tx);
    top.lerp(bottom, ty)
}

fn sample_rgba_weighted(
    params: &Nv12LayerParams,
    x: f32,
    y: f32,
    method: VideoSampleMethod,
) -> Color<f32> {
    let radius = sample_radius(method);
    let base_x = math::floor_f32(x) as i32;
    let base_y = math::floor_f32(y) as i32;
    let mut value = Color::new(0.0, 0.0, 0.0, 0.0);
    let mut weight_sum = 0.0;
    for row in 1 - radius..=radius {
        let wy = filter_weight(y - (base_y + row) as f32, method);
        for column in 1 - radius..=radius {
            let wx = filter_weight(x - (base_x + column) as f32, method);
            let weight = wx * wy;
            let sample = load_rgba(params, base_x + column, base_y + row);
            value.r += sample.r * weight;
            value.g += sample.g * weight;
            value.b += sample.b * weight;
            value.a += sample.a * weight;
            weight_sum += weight;
        }
    }
    if weight_sum.abs() <= 0.000_001 {
        return Color::TRANSPARENT;
    }
    Color::new(
        value.r / weight_sum,
        value.g / weight_sum,
        value.b / weight_sum,
        value.a / weight_sum,
    )
}

fn sample_luma(params: &Nv12LayerParams, x: f32, y: f32) -> f32 {
    match params.sample_method {
        VideoSampleMethod::Nearest => load_luma(
            params,
            math::floor_f32(x + 0.5) as i32,
            math::floor_f32(y + 0.5) as i32,
        ),
        VideoSampleMethod::Bilinear => sample_luma_bilinear(params, x, y),
        VideoSampleMethod::Fsr1Easu => sample_luma_bilinear(params, x, y),
        VideoSampleMethod::NvidiaImageScaling => nis::sample_luma(params, x, y),
        VideoSampleMethod::Anime4k | VideoSampleMethod::Anime4kSrgan => {
            sample_luma_bilinear(params, x, y)
        }
        VideoSampleMethod::Xbrz => xbrz::sample_luma(params, x, y),
        method => sample_luma_weighted(params, x, y, method),
    }
}

fn sample_chroma(params: &Nv12LayerParams, x: f32, y: f32, component: ChromaComponent) -> f32 {
    match params.sample_method {
        VideoSampleMethod::Nearest => load_chroma(
            params,
            math::floor_f32(x + 0.5) as i32,
            math::floor_f32(y + 0.5) as i32,
            component,
        ),
        VideoSampleMethod::Bilinear => sample_chroma_bilinear(params, x, y, component),
        VideoSampleMethod::Fsr1Easu => sample_chroma_bilinear(params, x, y, component),
        VideoSampleMethod::NvidiaImageScaling => sample_chroma_bilinear(params, x, y, component),
        VideoSampleMethod::Anime4k | VideoSampleMethod::Anime4kSrgan => {
            sample_chroma_bilinear(params, x, y, component)
        }
        VideoSampleMethod::Xbrz => sample_chroma_bilinear(params, x, y, component),
        method => sample_chroma_weighted(params, x, y, component, method),
    }
}

fn sample_luma_bilinear(params: &Nv12LayerParams, x: f32, y: f32) -> f32 {
    let x0 = math::floor_f32(x) as i32;
    let y0 = math::floor_f32(y) as i32;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let top = math::lerp(load_luma(params, x0, y0), load_luma(params, x0 + 1, y0), tx);
    let bottom = math::lerp(
        load_luma(params, x0, y0 + 1),
        load_luma(params, x0 + 1, y0 + 1),
        tx,
    );
    math::lerp(top, bottom, ty)
}

fn sample_chroma_bilinear(
    params: &Nv12LayerParams,
    x: f32,
    y: f32,
    component: ChromaComponent,
) -> f32 {
    let x0 = math::floor_f32(x) as i32;
    let y0 = math::floor_f32(y) as i32;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let top = math::lerp(
        load_chroma(params, x0, y0, component),
        load_chroma(params, x0 + 1, y0, component),
        tx,
    );
    let bottom = math::lerp(
        load_chroma(params, x0, y0 + 1, component),
        load_chroma(params, x0 + 1, y0 + 1, component),
        tx,
    );
    math::lerp(top, bottom, ty)
}

fn sample_luma_weighted(
    params: &Nv12LayerParams,
    x: f32,
    y: f32,
    method: VideoSampleMethod,
) -> f32 {
    sample_weighted(x, y, method, |column, row| load_luma(params, column, row))
}

fn sample_chroma_weighted(
    params: &Nv12LayerParams,
    x: f32,
    y: f32,
    component: ChromaComponent,
    method: VideoSampleMethod,
) -> f32 {
    sample_weighted(x, y, method, |column, row| {
        load_chroma(params, column, row, component)
    })
}

fn sample_weighted(
    x: f32,
    y: f32,
    method: VideoSampleMethod,
    load: impl Fn(i32, i32) -> f32,
) -> f32 {
    let radius = sample_radius(method);
    let base_x = math::floor_f32(x) as i32;
    let base_y = math::floor_f32(y) as i32;
    let mut value = 0.0;
    let mut weight_sum = 0.0;
    for row in 1 - radius..=radius {
        let wy = filter_weight(y - (base_y + row) as f32, method);
        for column in 1 - radius..=radius {
            let wx = filter_weight(x - (base_x + column) as f32, method);
            let weight = wx * wy;
            value += load(base_x + column, base_y + row) * weight;
            weight_sum += weight;
        }
    }
    if weight_sum.abs() > 0.000_001 {
        (value / weight_sum).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn load_luma(params: &Nv12LayerParams, x: i32, y: i32) -> f32 {
    let Some(sample_x) = address_index(
        x,
        params.source_width,
        params.crop[3],
        params.crop[1],
        params.address_mode,
    ) else {
        return 0.0;
    };
    let Some(sample_y) = address_index(
        y,
        params.source_height,
        params.crop[0],
        params.crop[2],
        params.address_mode,
    ) else {
        return 0.0;
    };
    let offset = sample_y * params.y_pitch + sample_x;
    (unsafe { *params.y_plane.add(offset) }) as f32 / 255.0
}

fn load_chroma(params: &Nv12LayerParams, x: i32, y: i32, component: ChromaComponent) -> f32 {
    let Some(sample_x) = address_index(
        x,
        chroma_width(params),
        params.crop[3],
        params.crop[1],
        params.address_mode,
    ) else {
        return 0.5;
    };
    let Some(sample_y) = address_index(
        y,
        chroma_height(params),
        params.crop[0],
        params.crop[2],
        params.address_mode,
    ) else {
        return 0.5;
    };
    let component_offset = match component {
        ChromaComponent::Cb => 0,
        ChromaComponent::Cr => 1,
    };
    let offset = sample_y * params.uv_pitch + sample_x * 2 + component_offset;
    (unsafe { *params.uv_plane.add(offset) }) as f32 / 255.0
}

fn load_rgba(params: &Nv12LayerParams, x: i32, y: i32) -> Color<f32> {
    let Some(sample_x) = address_index(
        x,
        params.source_width,
        params.crop[3],
        params.crop[1],
        params.address_mode,
    ) else {
        return Color::<f32>::TRANSPARENT;
    };
    let Some(sample_y) = address_index(
        y,
        params.source_height,
        params.crop[0],
        params.crop[2],
        params.address_mode,
    ) else {
        return Color::<f32>::TRANSPARENT;
    };
    let row = params.rgba_pitch / core::mem::size_of::<u32>();
    let offset = sample_y * row + sample_x;
    Color::from_rgba_u32(unsafe { *params.rgba.add(offset) }).premultiply()
}

fn address_index(
    index: i32,
    size: u32,
    crop_start: f32,
    crop_end: f32,
    mode: TextureAddressMode,
) -> Option<usize> {
    let last = size.max(1) as i32 - 1;
    let first = (math::ceil_f32(size as f32 * crop_start - 0.5) as i32).clamp(0, last);
    let last = (math::floor_f32(size as f32 * (1.0 - crop_end) - 0.5) as i32).clamp(first, last);
    let addressed = match mode {
        TextureAddressMode::Transparent => {
            if index < first || index > last {
                return None;
            }
            index
        }
        TextureAddressMode::ClampToEdge => index.clamp(first, last),
        TextureAddressMode::Repeat => math::repeat_index(index, first, last - first + 1),
        TextureAddressMode::MirrorRepeat
        | TextureAddressMode::BlurredMirror
        | TextureAddressMode::Stochastic => {
            math::mirror_repeat_index(index, first, last - first + 1)
        }
    };
    Some(addressed as usize)
}

fn chroma_width(params: &Nv12LayerParams) -> u32 {
    params.source_width.div_ceil(2).max(1)
}

fn chroma_height(params: &Nv12LayerParams) -> u32 {
    params.source_height.div_ceil(2).max(1)
}

fn sample_radius(method: VideoSampleMethod) -> i32 {
    match method {
        VideoSampleMethod::Lanczos3 => 3,
        VideoSampleMethod::Bicubic | VideoSampleMethod::Mitchell | VideoSampleMethod::Lanczos2 => 2,
        VideoSampleMethod::Nearest
        | VideoSampleMethod::Bilinear
        | VideoSampleMethod::Fsr1Easu
        | VideoSampleMethod::NvidiaImageScaling
        | VideoSampleMethod::Anime4k
        | VideoSampleMethod::Anime4kSrgan
        | VideoSampleMethod::Xbrz => 1,
    }
}

fn filter_weight(value: f32, method: VideoSampleMethod) -> f32 {
    match method {
        VideoSampleMethod::Mitchell => mitchell_weight(value),
        VideoSampleMethod::Lanczos2 => lanczos_weight(value, 2.0),
        VideoSampleMethod::Lanczos3 => lanczos_weight(value, 3.0),
        VideoSampleMethod::Fsr1Easu => 0.0,
        VideoSampleMethod::NvidiaImageScaling => 0.0,
        VideoSampleMethod::Anime4k | VideoSampleMethod::Anime4kSrgan => 0.0,
        VideoSampleMethod::Xbrz => 0.0,
        VideoSampleMethod::Bicubic | VideoSampleMethod::Nearest | VideoSampleMethod::Bilinear => {
            catmull_rom_weight(value)
        }
    }
}

fn catmull_rom_weight(value: f32) -> f32 {
    let x = value.abs();
    if x <= 1.0 {
        1.5 * x * x * x - 2.5 * x * x + 1.0
    } else if x < 2.0 {
        -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
    } else {
        0.0
    }
}

fn mitchell_weight(value: f32) -> f32 {
    let x = value.abs();
    if x <= 1.0 {
        1.166_666_6 * x * x * x - 2.0 * x * x + 0.888_888_9
    } else if x < 2.0 {
        -0.388_888_9 * x * x * x + 2.0 * x * x - 3.333_333_3 * x + 1.777_777_8
    } else {
        0.0
    }
}

fn lanczos_weight(value: f32, radius: f32) -> f32 {
    let x = value.abs();
    if x < 0.000_001 {
        1.0
    } else if x >= radius {
        0.0
    } else {
        sinc(x) * sinc(x / radius)
    }
}

fn sinc(value: f32) -> f32 {
    let x = PI * value;
    math::sin_f32(x) / x
}
