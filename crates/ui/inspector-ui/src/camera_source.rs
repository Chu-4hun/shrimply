use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    thread,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;
use shrimply_3dgs::{
    COLMAP_TRACKING_MODEL, CameraSource, TrackingCameraSource, TrackingSettings,
    VGGT_SLAM_TRACKING_MODEL,
};
use shrimply_project::project::{Project, VideoItemContent};
use shrimply_video::camera_reconstruction::{self, AnalysisStatus};
use uuid::Uuid;

use crate::{
    InspectedItem as SelectedItem, InspectorContext,
    player_state::{self, ProjectChange},
    section::InspectorSection,
    ui::{dropdown, enum_dropdown},
};

thread_local! {
    static MODEL_CACHE: RefCell<HashMap<String, Result<Vec<String>, String>>> = RefCell::default();
    static MODEL_REQUESTS: RefCell<HashSet<String>> = RefCell::default();
}

pub(super) fn add_controls(
    section: &InspectorSection,
    source: &CameraSource,
    context: &InspectorContext,
) -> bool {
    let source_control = source_dropdown(source, context);
    section.add_control_row("Camera source", &source_control);
    let CameraSource::Tracking(source) = source else {
        return true;
    };
    let server_url = shrimply_state::preferences::snapshot(&context.preferences).compute_server_url;
    let models = tracking_models(&server_url, context);
    add_tracking_controls(
        section,
        source,
        context,
        &server_url,
        models.as_ref(),
        &source_control,
    );
    false
}

fn source_dropdown(source: &CameraSource, context: &InspectorContext) -> gtk::DropDown {
    let key = context
        .selected_item
        .clone()
        .expect("camera controls require a selected item");
    let project = context.project.borrow();
    let mut choices = vec![(None, "Custom".to_string())];
    choices.extend(
        project
            .video_tracks_for_path(key.sequence_path())
            .into_iter()
            .flat_map(|tracks| tracks.iter())
            .enumerate()
            .filter(|(_, track)| track.id != key.track_id())
            .map(|(index, track)| (Some(track.id), format!("Visual track {index}"))),
    );
    let selected_id = match source {
        CameraSource::Custom => None,
        CameraSource::Tracking(source) => Some(source.track_id),
    };
    if choices.iter().all(|(id, _)| *id != selected_id) {
        let id = selected_id.expect("only tracked sources can be unavailable");
        choices.push((Some(id), format!("Unavailable ({id})")));
    }
    drop(project);
    let context = context.detached();
    let old_settings = match source {
        CameraSource::Custom => TrackingSettings::default(),
        CameraSource::Tracking(source) => source.settings.clone(),
    };
    dropdown(selected_id, choices, move |track_id| {
        let source = track_id.map_or(CameraSource::Custom, |track_id| {
            CameraSource::Tracking(TrackingCameraSource {
                track_id,
                settings: old_settings.clone(),
            })
        });
        update_source(&context, source, "edit-camera-source");
    })
}

fn add_tracking_controls(
    section: &InspectorSection,
    source: &TrackingCameraSource,
    context: &InspectorContext,
    server_url: &str,
    models: Option<&Result<Vec<String>, String>>,
    source_control: &gtk::DropDown,
) {
    let key = context
        .selected_item
        .clone()
        .expect("camera controls require a selected item");
    let camera_item_id = selected_item_id(&context.project.borrow(), key.clone())
        .expect("camera controls require a selected 3D item");
    let status = project_status(&context.project.borrow(), key.clone(), source);
    let running = matches!(
        status,
        AnalysisStatus::Queued
            | AnalysisStatus::Loading
            | AnalysisStatus::Analyzing { .. }
            | AnalysisStatus::Cancelling
    );
    let cancellable = matches!(
        status,
        AnalysisStatus::Queued | AnalysisStatus::Loading | AnalysisStatus::Analyzing { .. }
    );
    source_control.set_sensitive(!running);

    let available = models.and_then(|models| models.as_ref().ok());
    let mut choices = available
        .cloned()
        .unwrap_or_else(|| vec![source.settings.model.clone()]);
    if !choices.contains(&source.settings.model) {
        choices.push(source.settings.model.clone());
    }
    let selected = choices
        .iter()
        .position(|model| model == &source.settings.model)
        .expect("selected 3D tracking model must be listed");
    let labels = choices
        .iter()
        .map(|model| {
            let label = match model.as_str() {
                COLMAP_TRACKING_MODEL => "COLMAP".to_string(),
                VGGT_SLAM_TRACKING_MODEL => "VGGT-SLAM".to_string(),
                _ => model.clone(),
            };
            if available.is_some_and(|available| !available.contains(model)) {
                format!("{label} (Unavailable)")
            } else {
                label
            }
        })
        .collect::<Vec<_>>();
    let selected_models = choices.clone();
    let model_context = context.detached();
    let model = dropdown(selected, labels.into_iter().enumerate(), move |index| {
        let selected = selected_models[index].clone();
        update_settings(&model_context, "edit-3d-tracking-model", move |settings| {
            settings.model = selected
        });
    });
    model.set_sensitive(!running && available.is_some_and(|models| !models.is_empty()));
    section.add_control_row("Tracking method", &model);

    if source.settings.model == COLMAP_TRACKING_MODEL {
        let quality_context = context.detached();
        let quality = enum_dropdown(source.settings.quality, move |quality| {
            update_settings(&quality_context, "edit-colmap-quality", move |settings| {
                settings.quality = quality
            });
        });
        quality.set_sensitive(!running);
        section.add_control_row("Quality", &quality);

        let camera_model_context = context.detached();
        let camera_model = enum_dropdown(source.settings.camera_model, move |camera_model| {
            update_settings(
                &camera_model_context,
                "edit-colmap-camera-model",
                move |settings| settings.camera_model = camera_model,
            );
        });
        camera_model.set_sensitive(!running);
        section.add_control_row("Camera model", &camera_model);
    }

    let fps = gtk::SpinButton::with_range(1.0, 60.0, 1.0);
    fps.set_value(f64::from(source.settings.analysis_fps));
    fps.set_digits(0);
    fps.set_sensitive(!running);
    let fps_context = context.detached();
    fps.connect_value_changed(move |spin| {
        let fps = spin.value_as_int().clamp(1, 60) as u32;
        update_settings(&fps_context, "edit-colmap-analysis-fps", move |settings| {
            settings.analysis_fps = fps
        });
    });
    section.add_control_row("Analysis FPS", &fps);

    let status_row = adw::ActionRow::builder()
        .title("Analysis status")
        .subtitle(match models {
            None => "Checking compute server...".to_string(),
            Some(Err(error)) => format!("Server unavailable: {error}"),
            Some(Ok(models)) if !models.contains(&source.settings.model) => {
                "Selected tracking method is unavailable".to_string()
            }
            _ => status_label(&status),
        })
        .build();
    let spinner = adw::Spinner::new();
    spinner.set_size_request(18, 18);
    spinner.set_visible(running);
    status_row.add_suffix(&spinner);
    section.add_wide_control(&status_row);

    let matching = camera_reconstruction::has_matching_cache(camera_item_id, source);
    let can_analyze = available.is_some_and(|models| models.contains(&source.settings.model));
    let button = gtk::Button::builder()
        .label(if cancellable {
            "Cancel"
        } else if matches!(status, AnalysisStatus::Cancelling) {
            "Cancelling…"
        } else if matching || matches!(status, AnalysisStatus::OutOfDate) {
            "Analyze Again"
        } else {
            "Analyze"
        })
        .sensitive(cancellable || (!running && can_analyze))
        .halign(gtk::Align::Fill)
        .build();
    let analyze_context = context.detached();
    let analyze_source = source.clone();
    let server_url = server_url.to_string();
    button.connect_clicked(move |_| {
        let current = camera_reconstruction::status(camera_item_id, &analyze_source);
        let active = matches!(
            current,
            AnalysisStatus::Queued | AnalysisStatus::Loading | AnalysisStatus::Analyzing { .. }
        );
        if active {
            camera_reconstruction::cancel(camera_item_id, &analyze_source);
        } else {
            let project = analyze_context.project.borrow().clone();
            camera_reconstruction::analyze(
                project,
                camera_item_id,
                analyze_source.clone(),
                server_url.clone(),
            );
        }
        refresh(&analyze_context);
    });
    section.add_wide_control(&button);

    if running {
        let poll_context = context.detached();
        let poll_source = source.clone();
        let poll_status_row = status_row.clone();
        let poll_spinner = spinner.clone();
        let poll_button = button.clone();
        let mut poll_status = status;
        glib::timeout_add_local(Duration::from_millis(150), move || {
            if poll_button.parent().is_none() {
                return glib::ControlFlow::Break;
            }
            let status = camera_reconstruction::status(camera_item_id, &poll_source);
            if status != poll_status {
                update_analysis_controls(
                    &poll_status_row,
                    &poll_spinner,
                    &poll_button,
                    &status,
                    matching,
                    can_analyze,
                );
                poll_status = status.clone();
            }
            if matches!(
                status,
                AnalysisStatus::Queued
                    | AnalysisStatus::Analyzing { .. }
                    | AnalysisStatus::Cancelling
                    | AnalysisStatus::Loading
            ) {
                return glib::ControlFlow::Continue;
            }
            refresh(&poll_context);
            glib::ControlFlow::Break
        });
    }
}

fn project_status(
    project: &Project,
    key: SelectedItem,
    source: &TrackingCameraSource,
) -> AnalysisStatus {
    let Some(track) = project
        .video_tracks_for_path(key.sequence_path())
        .into_iter()
        .flat_map(|tracks| tracks.iter())
        .find(|track| track.id == source.track_id)
    else {
        return AnalysisStatus::MissingSourceTrack;
    };
    if key.track_id() == track.id {
        return AnalysisStatus::Failed {
            error: "A 3D item cannot track its own visual track".to_string(),
        };
    }
    if track.items.is_empty() || track.items.iter().all(|item| item.end <= item.start) {
        return AnalysisStatus::EmptySourceTrack;
    }
    let item_id =
        selected_item_id(project, key.clone()).expect("camera controls require a 3D item");
    camera_reconstruction::status(item_id, source)
}

fn status_label(status: &AnalysisStatus) -> String {
    match status {
        AnalysisStatus::NotAnalyzed => "Not analyzed".to_string(),
        AnalysisStatus::OutOfDate => "Out of date".to_string(),
        AnalysisStatus::Queued => "Queued".to_string(),
        AnalysisStatus::Loading => "Loading tracking model".to_string(),
        AnalysisStatus::Analyzing {
            message,
            completed_frames,
            total_frames,
        } if *total_frames != 0 => {
            format!("{message} {completed_frames}/{total_frames}")
        }
        AnalysisStatus::Analyzing { message, .. } => message.clone(),
        AnalysisStatus::Cancelling => "Cancelling".to_string(),
        AnalysisStatus::Cancelled => "Cancelled".to_string(),
        AnalysisStatus::Ready { sample_count } => format!("Ready ({sample_count} samples)"),
        AnalysisStatus::Failed { error } => format!("Failed: {error}"),
        AnalysisStatus::MissingSourceTrack => "Source track unavailable".to_string(),
        AnalysisStatus::EmptySourceTrack => "Source track is empty".to_string(),
    }
}

fn update_analysis_controls(
    status_row: &adw::ActionRow,
    spinner: &adw::Spinner,
    button: &gtk::Button,
    status: &AnalysisStatus,
    matching: bool,
    can_analyze: bool,
) {
    let running = matches!(
        status,
        AnalysisStatus::Queued
            | AnalysisStatus::Loading
            | AnalysisStatus::Analyzing { .. }
            | AnalysisStatus::Cancelling
    );
    let cancellable = matches!(
        status,
        AnalysisStatus::Queued | AnalysisStatus::Loading | AnalysisStatus::Analyzing { .. }
    );
    status_row.set_subtitle(&status_label(status));
    spinner.set_visible(running);
    button.set_label(if cancellable {
        "Cancel"
    } else if matches!(status, AnalysisStatus::Cancelling) {
        "Cancelling…"
    } else if matching
        || matches!(
            status,
            AnalysisStatus::OutOfDate | AnalysisStatus::Ready { .. }
        )
    {
        "Analyze Again"
    } else {
        "Analyze"
    });
    button.set_sensitive(cancellable || (!running && can_analyze));
}

fn update_source(context: &InspectorContext, source: CameraSource, commit_name: &'static str) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(current) = selected_source_mut(&mut project, key.clone()) else {
        return;
    };
    if *current == source {
        return;
    }
    *current = source;
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    refresh(context);
}

fn update_settings(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut TrackingSettings),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(CameraSource::Tracking(source)) = selected_source_mut(&mut project, key.clone())
    else {
        return;
    };
    let previous = source.settings.clone();
    update(&mut source.settings);
    if source.settings == previous {
        return;
    }
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    refresh(context);
}

fn tracking_models(
    server_url: &str,
    context: &InspectorContext,
) -> Option<Result<Vec<String>, String>> {
    if let Some(models) = MODEL_CACHE.with(|cache| cache.borrow().get(server_url).cloned()) {
        return Some(models);
    }
    if MODEL_REQUESTS.with(|requests| requests.borrow_mut().insert(server_url.to_string())) {
        let url = server_url.to_string();
        let request_url = url.clone();
        let refresh_context = context.detached();
        let (sender, receiver) = async_channel::bounded(1);
        thread::spawn(move || {
            let result = shrimply_server_client::server_status(&request_url).and_then(|status| {
                let models = status
                    .capabilities
                    .into_iter()
                    .filter_map(|capability| {
                        capability.strip_prefix("3dtracking:").map(str::to_string)
                    })
                    .collect::<Vec<_>>();
                (!models.is_empty())
                    .then_some(models)
                    .ok_or_else(|| "server does not advertise 3D tracking".to_string())
            });
            let _ = sender.send_blocking(result);
        });
        glib::spawn_future_local(async move {
            let Ok(result) = receiver.recv().await else {
                return;
            };
            MODEL_REQUESTS.with(|requests| {
                requests.borrow_mut().remove(&url);
            });
            MODEL_CACHE.with(|cache| {
                cache.borrow_mut().insert(url, result);
            });
            refresh(&refresh_context);
        });
    }
    None
}

fn refresh(context: &InspectorContext) {
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
}

fn selected_item_id(project: &Project, key: SelectedItem) -> Option<Uuid> {
    project.video_item(&key).map(|item| item.id)
}

fn selected_source_mut(project: &mut Project, key: SelectedItem) -> Option<&mut CameraSource> {
    let item = project.video_item_mut(&key)?;
    match &mut item.content {
        VideoItemContent::Obj(scene) => Some(&mut scene.camera.source),
        VideoItemContent::Gaussian(scene) => Some(&mut scene.camera.source),
        _ => None,
    }
}
