use super::*;

const MEDIA_INSPECTION_DELIVERY_INTERVAL: Duration = Duration::from_millis(16);

#[allow(clippy::too_many_arguments)]
pub(crate) fn import_path(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    path: PathBuf,
    x: f64,
    y: f64,
) {
    let (start, y) = {
        let runtime = runtime.borrow();
        let start = Time::from_seconds_f64(x_to_time(
            x,
            runtime.view.scroll_seconds,
            runtime.view.seconds_per_pixel,
        ));
        (
            runtime.snap_repository.snap(start).unwrap_or(start),
            content_y(runtime.view, y),
        )
    };
    import_path_at(
        area,
        project,
        player_state,
        selection_state,
        runtime,
        path,
        start,
        y,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn import_path_at(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    path: PathBuf,
    start: Time,
    y: f64,
) {
    let (canvas_size, default_visual_duration) = {
        let project = project.borrow();
        let runtime = runtime.borrow();
        (project.canvas_size, runtime.default_visual_duration)
    };
    let project = project.clone();
    let player_state = player_state.clone();
    let selection_state = selection_state.clone();
    let runtime_for_result = runtime.clone();
    inspect_media(
        area,
        runtime,
        path,
        canvas_size,
        default_visual_duration,
        move |area, info| {
            import_media_at(
                area,
                &project,
                &player_state,
                &selection_state,
                &runtime_for_result,
                info,
                start,
                y,
            );
        },
    );
}

fn inspect_media(
    area: &gtk::GLArea,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    path: PathBuf,
    canvas_size: CanvasSize,
    default_visual_duration: Time,
    on_ready: impl FnOnce(&gtk::GLArea, import::MediaInfo) + 'static,
) {
    let subscription = import::request_inspection(path, canvas_size, default_visual_duration);
    let mut on_ready = Some(on_ready);
    let handle = shrimply_ui_foundation::resource_pipeline::deliver(
        area.downgrade(),
        subscription,
        MEDIA_INSPECTION_DELIVERY_INTERVAL,
        move |area, event| match event {
            shrimply_resource_pipeline::Event::Finished(info) => {
                if let Err(error) = info.snapshot.ensure_current() {
                    show_error_dialog(area, "Could not import file", &error);
                } else if let Some(on_ready) = on_ready.take() {
                    on_ready(area, (*info).clone());
                }
            }
            shrimply_resource_pipeline::Event::Failed(error) => {
                show_error_dialog(area, "Could not import file", &error);
            }
            shrimply_resource_pipeline::Event::Progress(_)
            | shrimply_resource_pipeline::Event::Cancelled => {}
        },
    );
    let mut runtime = runtime.borrow_mut();
    runtime.resource_jobs.retain(|job| job.is_active());
    runtime.resource_jobs.push(handle);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn import_media_at(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    info: import::MediaInfo,
    start: Time,
    y: f64,
) {
    let changes = {
        let project = project.borrow();
        imported_video_setting_changes(&project, &info)
    };
    if let Some(changes) = changes {
        let area = area.clone();
        let parent = area.clone();
        let project = project.clone();
        let player_state = player_state.clone();
        let selection_state = selection_state.clone();
        let runtime = runtime.clone();
        prompt_for_video_settings(&parent, changes, move |match_video| {
            finish_media_import(
                &area,
                &project,
                &player_state,
                &selection_state,
                &runtime,
                info,
                start,
                y,
                match_video.then_some(changes),
            );
        });
        return;
    }
    finish_media_import(
        area,
        project,
        player_state,
        selection_state,
        runtime,
        info,
        start,
        y,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn finish_media_import(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    info: import::MediaInfo,
    start: Time,
    y: f64,
    settings: Option<(Option<CanvasSize>, Option<Fraction>)>,
) {
    if let Err(error) = info.snapshot.ensure_current() {
        show_error_dialog(area, "Could not import file", &error);
        return;
    }
    let mut project_state = project.borrow_mut();
    if let Some((canvas_size, fps)) = settings {
        if let Some(canvas_size) = canvas_size {
            project_state.canvas_size = canvas_size;
        }
        if let Some(fps) = fps {
            project_state.fps = fps;
        }
    }
    let preview = import::preview(
        &project_state,
        info.duration,
        info.video_streams,
        info.audio_streams,
        start,
        y,
    );
    let result = import::apply(&mut project_state, &info, &preview);
    let duration = project_state.duration();
    crate::project::commit_edit(&project_state, "import-media");
    drop(project_state);

    {
        let mut runtime = runtime.borrow_mut();
        runtime.import_preview = None;
    }
    let project = project.borrow();
    let focused_item = result.selection.first().copied();
    set_timeline_selection(&project, selection_state, result.selection, focused_item);
    player_state::refresh_project(
        player_state,
        ProjectChange {
            duration: Some(duration),
            audio: result.audio,
            audio_beats: result.audio,
            audio_waveforms: result.audio,
            video: result.video,
            live_preview: false,
            captions: result.captions || settings.is_some(),
            inspector: settings.is_some(),
        },
    );
    area.queue_render();
}

fn imported_video_setting_changes(
    project: &Project,
    info: &import::MediaInfo,
) -> Option<(Option<CanvasSize>, Option<Fraction>)> {
    if info.visual_kind != Some(import::VisualMediaKind::Video)
        || project
            .video_tracks
            .iter()
            .flat_map(|track| &track.items)
            .any(|item| item.is_video_media())
    {
        return None;
    }
    let canvas_size = info
        .video_sizes
        .first()
        .copied()
        .filter(|size| size.x > 0 && size.y > 0)
        .map(|size| CanvasSize {
            width: size.x,
            height: size.y,
        })
        .filter(|canvas_size| *canvas_size != project.canvas_size);
    let fps = info.video_fps.filter(|fps| *fps != project.fps);
    (canvas_size.is_some() || fps.is_some()).then_some((canvas_size, fps))
}

fn prompt_for_video_settings(
    area: &gtk::GLArea,
    changes: (Option<CanvasSize>, Option<Fraction>),
    on_choice: impl FnOnce(bool) + 'static,
) {
    let fps_label = changes.1.map(|fps| {
        format!("{:.3}", fraction_as_f64(fps))
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    });
    let body = match (changes.0, fps_label) {
        (Some(size), Some(fps)) => format!(
            "This is the first video in the project. Match the project to its {}×{} resolution and {} FPS?",
            size.width, size.height, fps,
        ),
        (Some(size), None) => format!(
            "This is the first video in the project. Match the project to its {}×{} resolution?",
            size.width, size.height,
        ),
        (None, Some(fps)) => format!(
            "This is the first video in the project. Match the project to its {} FPS?",
            fps,
        ),
        (None, None) => unreachable!("matching video settings requires a difference"),
    };
    let dialog = adw::AlertDialog::new(Some("Match Project to Video?"), Some(&body));
    dialog.add_responses(&[("keep", "Keep Project Settings"), ("match", "Match Video")]);
    dialog.set_close_response("keep");
    dialog.set_default_response(Some("match"));
    dialog.set_response_appearance("match", adw::ResponseAppearance::Suggested);
    dialog.choose(
        Some(area.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        move |response| on_choice(response.as_str() == "match"),
    );
}

pub(crate) fn open_track_import_dialog(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    targets: Vec<TrackKey>,
) {
    if targets.is_empty() {
        return;
    }
    let kind = targets[0].kind;
    if !targets.iter().all(|target| target.kind == kind) {
        show_error_dialog(
            area,
            "Could not import file",
            "Selected tracks must have the same type",
        );
        return;
    }

    let label = "Import to Track";
    let dialog = gtk::FileDialog::builder().title(label).build();
    let area = area.clone();
    let project = project.clone();
    let player_state = player_state.clone();
    let selection_state = selection_state.clone();
    let runtime = runtime.clone();
    shrimply_ui_foundation::file_picker::open(
        label,
        &dialog,
        None::<&gtk::Window>,
        move |result| {
            let Some(path) = result.ok().and_then(|file| file.path()) else {
                return;
            };
            import_path_to_tracks(
                &area,
                &project,
                &player_state,
                &selection_state,
                &runtime,
                path,
                kind,
                targets.iter().map(|target| target.track_index).collect(),
            );
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn import_path_to_tracks(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    path: PathBuf,
    target_kind: TrackKind,
    track_indices: Vec<usize>,
) {
    let Some(file_kind) = import::file_kind(&path) else {
        show_error_dialog(area, "Could not import file", "Unsupported file type");
        return;
    };
    let start = player_state::snapshot(player_state).position;
    if import::direct_media_kind(file_kind) && target_kind != TrackKind::Caption {
        let (canvas_size, default_visual_duration) = {
            let project = project.borrow();
            let runtime = runtime.borrow();
            (project.canvas_size, runtime.default_visual_duration)
        };
        let project = project.clone();
        let player_state = player_state.clone();
        let selection_state = selection_state.clone();
        inspect_media(
            area,
            runtime,
            path,
            canvas_size,
            default_visual_duration,
            move |area, info| {
                let result = {
                    let mut project_state = project.borrow_mut();
                    let result = import::apply_media_to_tracks(
                        &mut project_state,
                        &info,
                        target_kind,
                        &track_indices,
                        start,
                    );
                    if result.is_ok() {
                        crate::project::commit_edit(&project_state, "import-media-to-tracks");
                    }
                    result.map(|result| (result, project_state.duration()))
                };
                finish_track_import(area, &project, &player_state, &selection_state, result);
            },
        );
        return;
    }

    let result = if file_kind == import::FileKind::Vtt {
        if target_kind != TrackKind::Caption {
            Err("VTT files can only be imported to caption tracks".to_string())
        } else {
            let mut project_state = project.borrow_mut();
            let result =
                import::apply_vtt_to_tracks(&mut project_state, &path, &track_indices, start);
            if result.is_ok() {
                crate::project::commit_edit(&project_state, "import-vtt");
            }
            result.map(|result| (result, project_state.duration()))
        }
    } else if target_kind == TrackKind::Caption {
        Err("Only VTT files can be imported to caption tracks".to_string())
    } else {
        Err("MKV and WebM need to be remuxed before track import".to_string())
    };

    finish_track_import(area, project, player_state, selection_state, result);
}

#[allow(clippy::too_many_arguments)]
fn finish_track_import(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    result: Result<(import::ImportResult, Time), String>,
) {
    let (result, duration) = match result {
        Ok(result) => result,
        Err(error) => {
            show_error_dialog(area, "Could not import file", &error);
            return;
        }
    };

    let project = project.borrow();
    let focused_item = result.selection.first().copied();
    set_timeline_selection(&project, selection_state, result.selection, focused_item);
    player_state::refresh_project(
        player_state,
        ProjectChange {
            duration: Some(duration),
            audio: result.audio,
            audio_beats: result.audio,
            audio_waveforms: result.audio,
            video: result.video,
            live_preview: false,
            captions: result.captions,
            inspector: true,
        },
    );
    area.queue_render();
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn ask_remux_then_import(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    path: PathBuf,
    x: f64,
    y: f64,
) {
    let (start, y) = {
        let runtime = runtime.borrow();
        let start = Time::from_seconds_f64(x_to_time(
            x,
            runtime.view.scroll_seconds,
            runtime.view.seconds_per_pixel,
        ));
        (
            runtime.snap_repository.snap(start).unwrap_or(start),
            content_y(runtime.view, y),
        )
    };
    ask_remux_then_import_at(
        area,
        project,
        player_state,
        selection_state,
        runtime,
        path,
        start,
        y,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn ask_remux_then_import_at(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    path: PathBuf,
    start: Time,
    y: f64,
) {
    let dialog = adw::AlertDialog::new(
        Some("Remux MKV/WebM to MP4?"),
        Some(
            "MP4 is the supported timeline import format. The file can be remuxed losslessly first.",
        ),
    );
    dialog.add_responses(&[("cancel", "Cancel"), ("remux", "Remux")]);
    dialog.set_close_response("cancel");
    dialog.set_default_response(Some("remux"));
    dialog.set_response_appearance("remux", adw::ResponseAppearance::Suggested);

    let area = area.clone();
    let parent = area.clone();
    let response_area = area.clone();
    let project = project.clone();
    let player_state = player_state.clone();
    let selection_state = selection_state.clone();
    let runtime = runtime.clone();
    dialog.choose(
        Some(parent.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        move |response| {
            if response.as_str() == "remux" {
                start_remux(
                    &response_area,
                    &project,
                    &player_state,
                    &selection_state,
                    &runtime,
                    path,
                    start,
                    y,
                );
            } else {
                runtime.borrow_mut().import_preview = None;
                response_area.queue_render();
            }
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn start_remux(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    path: PathBuf,
    start: Time,
    y: f64,
) {
    let (tx, rx) = mpsc::channel();
    let original_path = path.clone();
    thread::spawn(move || {
        let _ = tx.send(import::remux_mkv_to_mp4(&path));
    });

    let area = area.clone();
    let project = project.clone();
    let player_state = player_state.clone();
    let selection_state = selection_state.clone();
    let runtime = runtime.clone();
    glib::timeout_add_local(WAVEFORM_POLL_INTERVAL, move || match rx.try_recv() {
        Ok(Ok(remuxed_path)) => {
            let dialog = adw::AlertDialog::new(
                Some("Delete original file?"),
                Some("The file was remuxed. Delete the original to keep only the MP4 copy?"),
            );
            dialog.add_response("keep", "Keep");
            dialog.add_response("delete", "Delete");
            dialog.set_close_response("keep");
            dialog.set_default_response(Some("keep"));
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

            let area_for_choice = area.clone();
            let area_for_choice_parent = area.clone();
            let project_for_choice = project.clone();
            let player_state_for_choice = player_state.clone();
            let selection_state_for_choice = selection_state.clone();
            let runtime_for_choice = runtime.clone();
            let original_path = original_path.clone();
            let remuxed_path = remuxed_path.clone();
            dialog.choose(
                Some(area_for_choice_parent.upcast_ref::<gtk::Widget>()),
                None::<&gio::Cancellable>,
                move |response| {
                    if response.as_str() == "delete"
                        && let Err(error) = std::fs::remove_file(&original_path)
                    {
                        show_error_dialog(
                            &area_for_choice,
                            "Could not delete original file",
                            &format!("Could not delete {}: {error}", original_path.display()),
                        );
                    }
                    import_path_at(
                        &area_for_choice,
                        &project_for_choice,
                        &player_state_for_choice,
                        &selection_state_for_choice,
                        &runtime_for_choice,
                        remuxed_path,
                        start,
                        y,
                    );
                },
            );
            glib::ControlFlow::Break
        }
        Ok(Err(error)) => {
            runtime.borrow_mut().import_preview = None;
            show_error_dialog(&area, "Could not remux source file", &error);
            area.queue_render();
            glib::ControlFlow::Break
        }
        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });
}

pub(crate) fn show_error_dialog(area: &gtk::GLArea, heading: &str, body: &str) {
    let dialog = adw::AlertDialog::new(Some(heading), Some(body));
    dialog.add_response("close", "Close");
    dialog.set_close_response("close");
    dialog.set_default_response(Some("close"));
    dialog.choose(
        Some(area.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        |_| {},
    );
}
