use std::path::Path;

use glam::{Mat3, Vec2};
use opencv::core::{
    self, Mat, MatTraitConst, Point2f, Size, TermCriteria, TermCriteria_Type, Vector,
};
use opencv::prelude::*;
use opencv::{imgproc, video, videoio};
use shrimply_math_core::{Time, fraction_as_u32_ratio, frame_rate_from_f64};

const MAXIMUM_FEATURES: i32 = 1_000;
const FEATURE_QUALITY: f64 = 0.01;
const MINIMUM_FEATURE_DISTANCE: f64 = 12.0;
const FEATURE_BLOCK_SIZE: i32 = 3;
const OPTICAL_FLOW_WINDOW: i32 = 20;
const OPTICAL_FLOW_PYRAMID_LEVELS: i32 = 3;
const OPTICAL_FLOW_ITERATIONS: i32 = 20;
const OPTICAL_FLOW_EPSILON: f64 = 0.01;
const MINIMUM_CROP_RATIO: f32 = 0.1;
const MAXIMUM_CROP_RATIO: f32 = 1.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdaptiveWeights {
    Original,
    Flipped,
    ConstantHigh,
    ConstantLow,
}

#[derive(Clone, Copy, Debug)]
pub struct StabilizationOptions {
    pub crop_ratio: f32,
    pub mesh_rows: u32,
    pub mesh_columns: u32,
    pub temporal_smoothing_radius: u32,
    pub optimization_iterations: u32,
    pub adaptive_weights: AdaptiveWeights,
}

#[derive(Clone, Debug)]
pub struct StabilizationChunk {
    pub first_frame: u64,
    pub frame_rate_numerator: u32,
    pub frame_rate_denominator: u32,
    pub grid_width: u32,
    pub grid_height: u32,
    pub source_offsets: Vec<Vec<Vec2>>,
}

pub fn analyze_chunk(
    input: &Path,
    track_id: u32,
    chunk_index: u64,
    chunk_seconds: u32,
    overlap_seconds: u32,
    options: StabilizationOptions,
    cancelled: impl Fn() -> bool,
) -> Result<Option<StabilizationChunk>, String> {
    if cancelled() {
        return Ok(None);
    }
    let mut capture = open_video(input, track_id)?;
    let width = checked_property(&capture, videoio::CAP_PROP_FRAME_WIDTH, "width")? as u32;
    let height = checked_property(&capture, videoio::CAP_PROP_FRAME_HEIGHT, "height")? as u32;
    let fps = capture
        .get(videoio::CAP_PROP_FPS)
        .map_err(|error| error.to_string())?;
    if !fps.is_finite() || fps <= 0.0 {
        return Err("MeshFlow requires a positive frame rate".to_string());
    }
    let frame_rate = frame_rate_from_f64(fps)
        .ok_or_else(|| "MeshFlow frame rate is out of range".to_string())?;
    let (frame_rate_numerator, frame_rate_denominator) = fraction_as_u32_ratio(frame_rate)
        .ok_or_else(|| "MeshFlow frame rate is out of range".to_string())?;
    let central_start =
        Time::from_seconds_u64(chunk_index.saturating_mul(u64::from(chunk_seconds)))
            .as_frame(frame_rate);
    let central_end = Time::from_seconds_u64(
        chunk_index
            .saturating_add(1)
            .saturating_mul(u64::from(chunk_seconds)),
    )
    .as_frame_ceil(frame_rate);
    let overlap = Time::from_seconds_u64(u64::from(overlap_seconds)).as_frame_ceil(frame_rate);
    let analysis_start = central_start.saturating_sub(overlap);
    let analysis_end = central_end.saturating_add(overlap);
    capture
        .set(videoio::CAP_PROP_POS_FRAMES, analysis_start as f64)
        .map_err(|error| error.to_string())?;

    let mut previous = Mat::default();
    if !capture
        .read(&mut previous)
        .map_err(|error| error.to_string())?
        || previous.empty()
    {
        return Err("MeshFlow could not read the first frame".to_string());
    }
    let grid_width = options.mesh_columns.clamp(2, 32) + 1;
    let grid_height = options.mesh_rows.clamp(2, 32) + 1;
    let vertices = grid_width as usize * grid_height as usize;
    let mut paths = vec![vec![Vec2::ZERO; vertices]];
    let mut transforms = vec![Mat3::IDENTITY];
    while analysis_start.saturating_add(paths.len() as u64) < analysis_end {
        if cancelled() {
            return Ok(None);
        }
        let mut current = Mat::default();
        if !capture
            .read(&mut current)
            .map_err(|error| error.to_string())?
            || current.empty()
        {
            break;
        }
        let tracks = tracked_features(&previous, &current)?;
        let (transform, residual, scene_cut) = shrimply_math_geometry::stabilization_mesh_motion(
            &tracks,
            grid_width as usize,
            grid_height as usize,
            width,
            height,
        );
        let global = shrimply_math_geometry::stabilization_forward_warp_offsets(
            transform,
            grid_width as usize,
            grid_height as usize,
            width,
            height,
        );
        let prior = paths.last().expect("MeshFlow path has its identity frame");
        paths.push(
            prior
                .iter()
                .zip(global.iter().zip(residual))
                .map(|(prior, (global, residual))| {
                    if scene_cut {
                        *prior
                    } else {
                        *prior + *global + residual
                    }
                })
                .collect(),
        );
        transforms.push(transform);
        previous = current;
    }
    if paths.is_empty() {
        return Err("MeshFlow chunk contains no frames".to_string());
    }
    let weights = transforms
        .iter()
        .map(|transform| adaptive_weight(*transform, width, height, options.adaptive_weights))
        .collect::<Vec<_>>();
    let Some(smoothed) = shrimply_math_geometry::stabilization_smooth_mesh_paths(
        &paths,
        &weights,
        options.temporal_smoothing_radius as usize,
        options.optimization_iterations as usize,
        &cancelled,
    ) else {
        return Ok(None);
    };
    let first = central_start.saturating_sub(analysis_start) as usize;
    let end = (central_end.saturating_sub(analysis_start) as usize).min(paths.len());
    if first >= end {
        return Err("MeshFlow chunk contains no central frames".to_string());
    }
    let crop_ratio = options
        .crop_ratio
        .clamp(MINIMUM_CROP_RATIO, MAXIMUM_CROP_RATIO);
    let source_offsets = paths[first..end]
        .iter()
        .zip(&smoothed[first..end])
        .map(|(observed, smooth)| {
            observed
                .iter()
                .zip(smooth)
                .enumerate()
                .map(|(index, (observed, smooth))| {
                    let column = index % grid_width as usize;
                    let row = index / grid_width as usize;
                    let x = column as f32 * width.saturating_sub(1) as f32
                        / grid_width.saturating_sub(1) as f32;
                    let y = row as f32 * height.saturating_sub(1) as f32
                        / grid_height.saturating_sub(1) as f32;
                    let center = Vec2::new(
                        width.saturating_sub(1) as f32 * 0.5,
                        height.saturating_sub(1) as f32 * 0.5,
                    );
                    *observed - *smooth + (crop_ratio - 1.0) * (Vec2::new(x, y) - center)
                })
                .collect()
        })
        .collect();
    Ok(Some(StabilizationChunk {
        first_frame: central_start,
        frame_rate_numerator,
        frame_rate_denominator,
        grid_width,
        grid_height,
        source_offsets,
    }))
}

fn tracked_features(previous: &Mat, current: &Mat) -> Result<Vec<[f32; 6]>, String> {
    let mut previous_gray = Mat::default();
    let mut current_gray = Mat::default();
    imgproc::cvt_color_def(previous, &mut previous_gray, imgproc::COLOR_BGR2GRAY)
        .map_err(|error| error.to_string())?;
    imgproc::cvt_color_def(current, &mut current_gray, imgproc::COLOR_BGR2GRAY)
        .map_err(|error| error.to_string())?;
    let mut previous_points = Vector::<Point2f>::new();
    imgproc::good_features_to_track(
        &previous_gray,
        &mut previous_points,
        MAXIMUM_FEATURES,
        FEATURE_QUALITY,
        MINIMUM_FEATURE_DISTANCE,
        &core::no_array(),
        FEATURE_BLOCK_SIZE,
        false,
        0.04,
    )
    .map_err(|error| error.to_string())?;
    if previous_points.len() < 4 {
        return Ok(Vec::new());
    }
    let mut current_points = Vector::<Point2f>::new();
    let mut status = Vector::<u8>::new();
    let mut errors = Vector::<f32>::new();
    video::calc_optical_flow_pyr_lk(
        &previous_gray,
        &current_gray,
        &previous_points,
        &mut current_points,
        &mut status,
        &mut errors,
        Size::new(OPTICAL_FLOW_WINDOW, OPTICAL_FLOW_WINDOW),
        OPTICAL_FLOW_PYRAMID_LEVELS,
        TermCriteria::new(
            TermCriteria_Type::COUNT as i32 | TermCriteria_Type::EPS as i32,
            OPTICAL_FLOW_ITERATIONS,
            OPTICAL_FLOW_EPSILON,
        )
        .map_err(|error| error.to_string())?,
        0,
        0.0001,
    )
    .map_err(|error| error.to_string())?;
    Ok(previous_points
        .iter()
        .zip(current_points.iter())
        .zip(status.iter().zip(errors.iter()))
        .filter_map(|((early, late), (valid, error))| {
            (valid != 0 && error.is_finite()).then_some([
                early.x,
                early.y,
                late.x,
                late.y,
                1.0 / (1.0 + error.max(0.0)),
                1.0,
            ])
        })
        .collect())
}

fn adaptive_weight(transform: Mat3, width: u32, height: u32, definition: AdaptiveWeights) -> f32 {
    match definition {
        AdaptiveWeights::ConstantHigh => 100.0,
        AdaptiveWeights::ConstantLow => 1.0,
        AdaptiveWeights::Original | AdaptiveWeights::Flipped => {
            let translation = (transform.z_axis.x / width.max(1) as f32)
                .hypot(transform.z_axis.y / height.max(1) as f32);
            let scale = transform.x_axis.truncate().length().max(f32::EPSILON);
            let candidate_motion = -1.93 * translation + 0.95;
            let candidate_affine = 5.83 * scale
                + if matches!(definition, AdaptiveWeights::Original) {
                    4.88
                } else {
                    -4.88
                };
            candidate_motion.min(candidate_affine).max(0.0)
        }
    }
}

fn checked_property(
    video: &videoio::VideoCapture,
    property: i32,
    name: &str,
) -> Result<f64, String> {
    let value = video.get(property).map_err(|error| error.to_string())?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("MeshFlow video has invalid {name}"));
    }
    Ok(value)
}

fn open_video(path: &Path, track_id: u32) -> Result<videoio::VideoCapture, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "MeshFlow input path is not UTF-8".to_string())?;
    let capture = if track_id == 0 {
        videoio::VideoCapture::from_file(path, videoio::CAP_ANY)
    } else {
        videoio::VideoCapture::from_file_with_params(
            path,
            videoio::CAP_FFMPEG,
            &Vector::from_slice(&[
                videoio::CAP_PROP_VIDEO_STREAM,
                i32::try_from(track_id).map_err(|_| "MeshFlow stream index is too large")?,
            ]),
        )
    }
    .map_err(|error| error.to_string())?;
    if !capture.is_opened().map_err(|error| error.to_string())? {
        return Err("MeshFlow could not open the selected video stream".to_string());
    }
    Ok(capture)
}
