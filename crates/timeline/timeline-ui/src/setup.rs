use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use shrimply_resource_pipeline::{Event, JobContext, Pipeline, Processor};

use super::*;

static NEXT_AUDIO_RESOURCE_REQUEST: AtomicU64 = AtomicU64::new(1);
static WAVEFORM_PIPELINE: OnceLock<Pipeline<WaveformRequest, WaveformProcessor>> = OnceLock::new();
static BEAT_PIPELINE: OnceLock<Pipeline<BeatRequest, BeatProcessor>> = OnceLock::new();

#[derive(Clone)]
struct WaveformRequest {
    id: u64,
    project: Arc<Project>,
    chunks_per_second: u32,
}

impl PartialEq for WaveformRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for WaveformRequest {}

impl Hash for WaveformRequest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

struct WaveformProcessor;

impl Processor<WaveformRequest> for WaveformProcessor {
    type Progress = Vec<(uuid::Uuid, waveform::WaveformUpdate)>;
    type Output = WaveformMap;

    fn process(
        &self,
        request: WaveformRequest,
        context: &JobContext<Self::Progress>,
    ) -> Result<Self::Output, String> {
        let mut waveforms = WaveformMap::new();
        waveform::load_project_waveforms_cancellable(
            &request.project,
            request.chunks_per_second,
            || context.is_cancelled(),
            |id, update| {
                context.report(vec![(id, update.clone())]);
                waveform::apply_update(&mut waveforms, id, update);
            },
        );
        Ok(waveforms)
    }
}

#[derive(Clone)]
struct BeatRequest {
    id: u64,
    project: Arc<Project>,
}

impl PartialEq for BeatRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for BeatRequest {}

impl Hash for BeatRequest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

struct BeatProcessor;

impl Processor<BeatRequest> for BeatProcessor {
    type Progress = (uuid::Uuid, beat::BeatUpdate);
    type Output = BeatMap;

    fn process(
        &self,
        request: BeatRequest,
        context: &JobContext<Self::Progress>,
    ) -> Result<Self::Output, String> {
        let mut beats = BeatMap::new();
        beat::load_project_beats_cancellable(
            &request.project,
            || context.is_cancelled(),
            |id, update| {
                context.report((id, update.clone()));
                beat::apply_update(&mut beats, id, update);
            },
        );
        Ok(beats)
    }
}

pub(super) fn start_waveform_loader(
    area: &gtk::GLArea,
    project: Rc<RefCell<Project>>,
    runtime: Rc<RefCell<TimelineRuntime>>,
) {
    let project = Arc::new(project.borrow().clone());
    let chunks_per_second =
        waveform_chunks_per_second_from_frame_step(frame_step_seconds(&project));
    let request = WaveformRequest {
        id: next_audio_resource_request(),
        project,
        chunks_per_second,
    };
    let (_, subscription) = waveform_pipeline().request(request);
    let runtime_for_delivery = runtime.clone();
    let handle = shrimply_ui_foundation::resource_pipeline::deliver(
        area.downgrade(),
        subscription,
        WAVEFORM_POLL_INTERVAL,
        move |area, event| {
            let mut runtime = runtime_for_delivery.borrow_mut();
            match event {
                Event::Progress(progress) => {
                    for (id, update) in progress.iter() {
                        waveform::apply_update(&mut runtime.waveforms, *id, update.clone());
                    }
                    area.queue_render();
                }
                Event::Finished(waveforms) => {
                    for (id, waveform) in waveforms.iter() {
                        runtime.waveforms.insert(*id, waveform.clone());
                    }
                    area.queue_render();
                }
                Event::Failed(error) => tracing::warn!(%error, "Could not load audio waveforms"),
                Event::Cancelled => {}
            }
        },
    );
    runtime.borrow_mut().waveform_job = Some(handle);
}

pub(super) fn start_beat_loader(
    area: &gtk::GLArea,
    project: Rc<RefCell<Project>>,
    runtime: Rc<RefCell<TimelineRuntime>>,
) {
    let project = Arc::new(project.borrow().clone());
    beat::begin_loading(&project);
    beat::retain_enabled(&mut runtime.borrow_mut().beats, &project);
    let request = BeatRequest {
        id: next_audio_resource_request(),
        project,
    };
    let (_, subscription) = beat_pipeline().request(request);
    let runtime_for_delivery = runtime.clone();
    let handle = shrimply_ui_foundation::resource_pipeline::deliver(
        area.downgrade(),
        subscription,
        BEAT_POLL_INTERVAL,
        move |area, event| {
            let mut runtime = runtime_for_delivery.borrow_mut();
            match event {
                Event::Progress(progress) => {
                    let (id, update) = &*progress;
                    beat::apply_update(&mut runtime.beats, *id, update.clone());
                    area.queue_render();
                }
                Event::Finished(beats) => {
                    for (id, state) in beats.iter() {
                        runtime.beats.insert(*id, state.clone());
                    }
                    area.queue_render();
                }
                Event::Failed(error) => tracing::warn!(%error, "Could not load audio beats"),
                Event::Cancelled => {}
            }
        },
    );
    runtime.borrow_mut().beat_job = Some(handle);
}

fn next_audio_resource_request() -> u64 {
    NEXT_AUDIO_RESOURCE_REQUEST.fetch_add(1, Ordering::Relaxed)
}

fn waveform_pipeline() -> &'static Pipeline<WaveformRequest, WaveformProcessor> {
    WAVEFORM_PIPELINE.get_or_init(|| {
        Pipeline::new_with_progress_merge(
            WaveformProcessor,
            |job| {
                std::thread::spawn(job);
            },
            |current, mut next| current.append(&mut next),
        )
    })
}

fn beat_pipeline() -> &'static Pipeline<BeatRequest, BeatProcessor> {
    BEAT_PIPELINE.get_or_init(|| {
        Pipeline::new(BeatProcessor, |job| {
            std::thread::spawn(job);
        })
    })
}

pub(super) fn timeline_tool_button(icon_name: &str) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_child(Some(&gtk::Image::from_icon_name(icon_name)));
    button.set_size_request(SIDEBAR_ICON_SIZE, SIDEBAR_ICON_SIZE);
    button.add_css_class("flat");
    button
}

pub(super) fn timeline_magnet_from_preference(value: &str) -> bool {
    value == "true"
}

pub(super) fn timeline_beat_grid_from_preference(value: &str) -> bool {
    value == "true"
}

pub(super) fn drag_collision_mode_from_preference(value: &str) -> DragCollisionMode {
    match value {
        "block" => DragCollisionMode::Block,
        "new_track" => DragCollisionMode::NewTrack,
        _ => DragCollisionMode::Overwrite,
    }
}
