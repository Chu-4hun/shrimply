use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use ffmpeg_next::{format::Pixel, frame};
use shrimply_project::project::{ItemAddress, Project, Time};
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect};
use uuid::Uuid;

use crate::{
    compositor::{EXPORT_ASSETS_LOADING, VideoExportRenderer},
    modifiers::transparent_fill::{TransparentFillMaskCache, cache_key, encode_mask, frame_count},
};

const MAXIMUM_MASK_WORKERS: usize = 6;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Missing,
    Running { completed: u64, total: u64 },
    Complete,
    Cancelled,
    Failed(String),
}

struct Job {
    signature: u64,
    cache_key: String,
    status: Status,
    cancelled: Arc<AtomicBool>,
}

struct Input {
    project: Project,
    item_id: Uuid,
    modifier_id: Uuid,
    cache_key: String,
    start: Time,
    end: Time,
    points: Vec<shrimply_video_modifiers::transparent_fill::TransparentFillPoint>,
    tolerance: shrimply_core::timeline_value::TimelineValue<f32>,
    maximum_gap: u32,
    signature: u64,
}

struct MaskJob {
    frame_index: u64,
    rgba: Vec<u8>,
    seeds: Vec<(u32, u32)>,
    tolerance: f32,
}

struct MaskResult {
    frame_index: u64,
    mask: Result<(Vec<u8>, Vec<u8>), String>,
}

static JOBS: LazyLock<Mutex<HashMap<Uuid, Job>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn analyze(project: Project, address: &ItemAddress, modifier_id: Uuid) -> Result<(), String> {
    let input = prepare(project, address, modifier_id)?;
    let total = frame_total(input.start, input.end, input.project.fps)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut jobs = JOBS
            .lock()
            .expect("transparent fill analysis job lock is poisoned");
        if let Some(previous) = jobs.get(&modifier_id)
            && matches!(previous.status, Status::Running { .. })
        {
            return Err("transparent fill analysis is already running".to_string());
        }
        jobs.insert(
            modifier_id,
            Job {
                signature: input.signature,
                cache_key: input.cache_key.clone(),
                status: Status::Running {
                    completed: 0,
                    total,
                },
                cancelled: cancelled.clone(),
            },
        );
    }
    let spawn = thread::Builder::new()
        .name(format!("transparent-fill-analysis-{modifier_id}"))
        .spawn(move || {
            let signature = input.signature;
            let cache_key = input.cache_key.clone();
            let result = analyze_inner(input, &cancelled);
            let mut jobs = JOBS
                .lock()
                .expect("transparent fill analysis job lock is poisoned");
            let Some(job) = jobs.get_mut(&modifier_id) else {
                return;
            };
            if job.signature != signature || job.cache_key != cache_key {
                return;
            }
            job.status = match result {
                Ok(()) if cancelled.load(Ordering::Acquire) => Status::Cancelled,
                Ok(()) => Status::Complete,
                Err(_) if cancelled.load(Ordering::Acquire) => Status::Cancelled,
                Err(error) => Status::Failed(error),
            };
        });
    if let Err(error) = spawn {
        JOBS.lock()
            .expect("transparent fill analysis job lock is poisoned")
            .remove(&modifier_id);
        return Err(format!("spawn transparent fill analysis: {error}"));
    }
    Ok(())
}

pub fn cancel(modifier_id: Uuid) -> bool {
    let mut jobs = JOBS
        .lock()
        .expect("transparent fill analysis job lock is poisoned");
    let Some(job) = jobs.get_mut(&modifier_id) else {
        return false;
    };
    if !matches!(job.status, Status::Running { .. }) {
        return false;
    }
    job.cancelled.store(true, Ordering::Release);
    job.status = Status::Cancelled;
    true
}

pub fn status(project: &Project, address: &ItemAddress, modifier_id: Uuid) -> Status {
    let current = current(project, address, modifier_id);
    let Ok((signature, key, frames)) = current else {
        return Status::Missing;
    };
    if let Some(status) = {
        let mut jobs = JOBS
            .lock()
            .expect("transparent fill analysis job lock is poisoned");
        jobs.get_mut(&modifier_id).and_then(|job| {
            if job.signature == signature && job.cache_key == key {
                Some(job.status.clone())
            } else {
                job.cancelled.store(true, Ordering::Release);
                None
            }
        })
    } {
        return status;
    }
    if TransparentFillMaskCache::shared().analysis_complete(
        &key,
        project.canvas_size.width,
        project.canvas_size.height,
        frames,
    ) {
        Status::Complete
    } else {
        Status::Missing
    }
}

fn prepare(
    mut project: Project,
    address: &ItemAddress,
    modifier_id: Uuid,
) -> Result<Input, String> {
    let item = project
        .video_item(address)
        .ok_or_else(|| "transparent fill item no longer exists".to_string())?;
    let modifier_index = item
        .modifiers
        .iter()
        .position(|modifier| modifier.id == modifier_id)
        .ok_or_else(|| "transparent fill modifier no longer exists".to_string())?;
    let ModifierEffect::Raster(effect) = &item.modifiers[modifier_index].effect else {
        return Err("selected modifier is not Transparent Fill".to_string());
    };
    let RasterModifierEffect::TransparentFill(fill) = &**effect else {
        return Err("selected modifier is not Transparent Fill".to_string());
    };
    if fill.points.is_empty() {
        return Err("add at least one transparent fill point before analyzing".to_string());
    }
    let key = cache_key(&project, item, modifier_id, modifier_index, fill);
    let input = Input {
        project: project.clone(),
        item_id: item.id,
        modifier_id,
        cache_key: key,
        start: item.start,
        end: item.end,
        points: fill.points.clone(),
        tolerance: fill.tolerance.clone(),
        maximum_gap: fill.maximum_gap,
        signature: fill.prompt_signature(),
    };
    for track in &mut project.video_tracks {
        for other in &mut track.items {
            if other
                .transitions
                .to_next
                .as_ref()
                .is_some_and(|transition| transition.target_item_id == input.item_id)
            {
                other.transitions.to_next = None;
            }
        }
    }
    project
        .video_item_mut(address)
        .expect("transparent fill item disappeared from cloned project")
        .modifiers
        .truncate(modifier_index);
    Ok(Input { project, ..input })
}

fn current(
    project: &Project,
    address: &ItemAddress,
    modifier_id: Uuid,
) -> Result<(u64, String, u64), String> {
    let item = project
        .video_item(address)
        .ok_or_else(|| "transparent fill item no longer exists".to_string())?;
    let (index, modifier) = item
        .modifiers
        .iter()
        .enumerate()
        .find(|(_, modifier)| modifier.id == modifier_id)
        .ok_or_else(|| "transparent fill modifier no longer exists".to_string())?;
    let ModifierEffect::Raster(effect) = &modifier.effect else {
        return Err("selected modifier is not Transparent Fill".to_string());
    };
    let RasterModifierEffect::TransparentFill(fill) = &**effect else {
        return Err("selected modifier is not Transparent Fill".to_string());
    };
    Ok((
        fill.prompt_signature(),
        cache_key(project, item, modifier_id, index, fill),
        frame_count(project, item).ok_or("project frame rate must be positive")?,
    ))
}

fn analyze_inner(input: Input, cancelled: &AtomicBool) -> Result<(), String> {
    let cache = TransparentFillMaskCache::shared();
    cache.begin_analysis(&input.cache_key, input.modifier_id)?;
    let result = (|| {
        let width = input.project.canvas_size.width.max(1);
        let height = input.project.canvas_size.height.max(1);
        let frames = shrimply_math_core::frame_range(input.start, input.end, input.project.fps)
            .ok_or("project frame rate must be positive for transparent fill")?;
        let total = frames.end.saturating_sub(frames.start);
        let mut renderer = VideoExportRenderer::new(48_000)?;
        let item = input
            .project
            .video_tracks
            .iter()
            .flat_map(|track| &track.items)
            .find(|item| item.id == input.item_id)
            .ok_or("transparent fill source item disappeared")?;
        let worker_count = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .saturating_sub(1)
            .clamp(1, MAXIMUM_MASK_WORKERS)
            .min(usize::try_from(total).unwrap_or(usize::MAX).max(1));
        let (job_sender, job_receiver) = mpsc::sync_channel::<MaskJob>(worker_count);
        let job_receiver = Mutex::new(job_receiver);
        let (result_sender, result_receiver) = mpsc::channel::<MaskResult>();
        thread::scope(|scope| -> Result<(), String> {
            for _ in 0..worker_count {
                let result_sender = result_sender.clone();
                let job_receiver = &job_receiver;
                scope.spawn(move || {
                    loop {
                        let job = match job_receiver
                            .lock()
                            .expect("transparent fill worker queue lock is poisoned")
                            .recv()
                        {
                            Ok(job) => job,
                            Err(_) => break,
                        };
                        let mask = if cancelled.load(Ordering::Acquire) {
                            Err("transparent fill analysis cancelled".to_string())
                        } else {
                            shrimply_math_color::transparent_fill_mask(
                                &job.rgba,
                                width,
                                height,
                                &job.seeds,
                                job.tolerance,
                                input.maximum_gap,
                            )
                            .and_then(|mask| {
                                let png = encode_mask(&mask, width, height)?;
                                Ok((mask, png))
                            })
                        };
                        if result_sender
                            .send(MaskResult {
                                frame_index: job.frame_index,
                                mask,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            }
            drop(result_sender);

            let mut submitted = 0_u64;
            let mut completed = 0_u64;
            let store_result = |result: MaskResult, completed: u64| -> Result<u64, String> {
                let (mask, png) = result.mask?;
                cache.insert_staged_encoded(
                    &input.cache_key,
                    i64::try_from(result.frame_index)
                        .map_err(|_| "transparent fill frame is too large")?,
                    &mask,
                    png,
                )?;
                let completed = completed + 1;
                update_progress(
                    input.modifier_id,
                    input.signature,
                    &input.cache_key,
                    completed,
                    total,
                );
                Ok(completed)
            };

            for frame_index in frames {
                if cancelled.load(Ordering::Acquire) {
                    return Err("transparent fill analysis cancelled".to_string());
                }
                let position = shrimply_math_core::time_from_frame(frame_index, input.project.fps)
                    .ok_or("project frame rate must be positive for transparent fill")?
                    .max(input.start);
                let composited = loop {
                    match renderer.render_cache_item(&input.project, position, input.item_id) {
                        Ok(frame) => break frame,
                        Err(error) if error == EXPORT_ASSETS_LOADING => {
                            if cancelled.load(Ordering::Acquire) {
                                return Err("transparent fill analysis cancelled".to_string());
                            }
                            thread::yield_now();
                        }
                        Err(error) => return Err(error),
                    }
                };
                let mut output = frame::Video::new(Pixel::RGBA, width, height);
                renderer.copy_to_rgba_frame(composited, &mut output)?;
                let row_bytes = width as usize * 4;
                let stride = output.stride(0);
                let mut rgba = Vec::with_capacity(row_bytes * height as usize);
                for row in output.data(0).chunks_exact(stride).take(height as usize) {
                    rgba.extend_from_slice(&row[..row_bytes]);
                }
                let local_time = shrimply_project::project::generated_item_time(item, position)
                    .unwrap_or(Time::ZERO);
                let seeds = input
                    .points
                    .iter()
                    .map(|point| {
                        let point = point
                            .position
                            .value_at(local_time)
                            .clamp(glam::Vec2::ZERO, glam::Vec2::ONE);
                        (
                            (point.x * width.saturating_sub(1) as f32).round() as u32,
                            (point.y * height.saturating_sub(1) as f32).round() as u32,
                        )
                    })
                    .collect();
                job_sender
                    .send(MaskJob {
                        frame_index,
                        rgba,
                        seeds,
                        tolerance: input.tolerance.value_at(local_time),
                    })
                    .map_err(|_| "transparent fill mask workers stopped unexpectedly")?;
                submitted += 1;
                while let Ok(result) = result_receiver.try_recv() {
                    completed = store_result(result, completed)?;
                }
            }
            drop(job_sender);
            while completed < submitted {
                completed = store_result(
                    result_receiver
                        .recv()
                        .map_err(|_| "transparent fill mask workers stopped unexpectedly")?,
                    completed,
                )?;
            }
            Ok(())
        })?;
        cache.complete_analysis(&input.cache_key, width, height, total)
    })();
    if result.is_err() {
        cache.abort_analysis(&input.cache_key);
    }
    result
}

fn update_progress(modifier_id: Uuid, signature: u64, cache_key: &str, completed: u64, total: u64) {
    let mut jobs = JOBS
        .lock()
        .expect("transparent fill analysis job lock is poisoned");
    if let Some(job) = jobs.get_mut(&modifier_id)
        && job.signature == signature
        && job.cache_key == cache_key
        && !job.cancelled.load(Ordering::Acquire)
    {
        job.status = Status::Running { completed, total };
    }
}

fn frame_total(start: Time, end: Time, fps: shrimply_math_core::Fraction) -> Result<u64, String> {
    let frames = shrimply_math_core::frame_range(start, end, fps)
        .ok_or("project frame rate must be positive for transparent fill")?;
    if frames.is_empty() {
        return Err("cannot analyze an item shorter than one project frame".to_string());
    }
    Ok(frames.end - frames.start)
}
