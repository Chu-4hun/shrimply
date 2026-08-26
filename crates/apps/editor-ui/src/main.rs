use shrimply_export_ui as export;
use shrimply_support::crash;
mod header_menu;
mod mcp;
use shrimply_inspector_ui as inspector;
use shrimply_state::{player_state, preview_focus};
mod preferences;
use shrimply_preview_ui as video_player;
use shrimply_timeline::selection_state;
use shrimply_timeline_ui as timeline;
use shrimply_ui_foundation::project_settings::ProjectSettingsSelector;

pub use shrimply_audio as audio;
pub use shrimply_project::project;

use crate::preferences::store as preferences_store;
use adw::prelude::*;
use ffmpeg_next as ffmpeg;
use gtk::{gio, glib};
use shrimply_math_core::Fraction;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;

const DEFAULT_WINDOW_WIDTH: i32 = 1800;
const DEFAULT_WINDOW_HEIGHT: i32 = 1100;
const DEFAULT_INSPECTOR_WIDTH: i32 = inspector::INSPECTOR_MIN_WIDTH;
const DEFAULT_TOP_PANEL_HEIGHT: i32 = 660;
const LOADING_SPINNER_SIZE: i32 = 48;

fn main() -> glib::ExitCode {
    let mut gdk_disabled = std::env::var("GDK_DISABLE").unwrap_or_default();
    if !gdk_disabled.split(',').any(|feature| feature == "dmabuf") {
        if !gdk_disabled.is_empty() {
            gdk_disabled.push(',');
        }
        gdk_disabled.push_str("dmabuf");
        // SAFETY: This is the first operation in main, before GTK or any threads start.
        unsafe { std::env::set_var("GDK_DISABLE", gdk_disabled) };
    }
    crash::install();
    shrimply_support::diagnostics::init();
    crash::install_glib_hooks();
    let mut args = std::env::args_os().skip(1);
    let Some(project_path) = args.next().map(PathBuf::from) else {
        eprintln!(
            "usage: shrimply-editor PROJECT.shrimp|PROJECT.json|TIMELINE.otio|PROJECT.kdenlive"
        );
        return glib::ExitCode::FAILURE;
    };
    if args.next().is_some() {
        eprintln!(
            "usage: shrimply-editor PROJECT.shrimp|PROJECT.json|TIMELINE.otio|PROJECT.kdenlive"
        );
        return glib::ExitCode::FAILURE;
    }
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Shrimply started");

    let app = adw::Application::new(
        Some("dev.shrimply.Shrimply.Editor"),
        gio::ApplicationFlags::NON_UNIQUE,
    );

    app.connect_activate(move |app| begin_project_load(app, project_path.clone()));
    let status = app.run_with_args(&[env!("CARGO_BIN_NAME")]);
    let save_result = project::shutdown_history();
    project::clear_project_file_locks();
    if let Err(error) = save_result {
        tracing::error!("Could not save project during shutdown: {error}");
        return glib::ExitCode::FAILURE;
    }
    status
}

fn build_ui(window: &adw::ApplicationWindow, project: project::Project) {
    let playback_performance = shrimply_playback_performance::open(Arc::new(project.clone()));
    let project = Rc::new(RefCell::new(project));
    project
        .borrow()
        .watch_assets()
        .unwrap_or_else(|error| panic!("could not watch project assets: {error}"));
    let duration = project.borrow().duration();
    let player_state = player_state::new(duration);
    let asset_changes = shrimply_asset::subscribe();
    let asset_player_state = player_state.clone();
    let asset_project = project.clone();
    glib::spawn_future_local(async move {
        while let Ok(change) = asset_changes.recv().await {
            let (audio, video) = {
                let project = asset_project.borrow();
                (
                    project.uses_audio_asset(&change.path),
                    project.uses_video_asset(&change.path),
                )
            };
            if !audio && !video {
                continue;
            }
            tracing::info!(
                path = %change.path.display(),
                revision = change.revision,
                audio,
                video,
                "project asset changed"
            );
            player_state::refresh_project(
                &asset_player_state,
                player_state::ProjectChange {
                    audio,
                    audio_beats: audio,
                    audio_waveforms: audio,
                    video,
                    ..Default::default()
                },
            );
        }
    });
    let watched_project = project.clone();
    player_state::connect_named(&player_state, "watch project assets", move |event| {
        if matches!(event, player_state::PlayerEvent::Project(_))
            && let Err(error) = watched_project.borrow().watch_assets()
        {
            tracing::error!(%error, "could not watch project assets");
        }
    });
    if let Some(position) = project.borrow().cursor_position {
        player_state::set_position(&player_state, position.clamp(project::Time::ZERO, duration));
    }
    let cursor_project = project.clone();
    let cursor_player_state = player_state.clone();
    let pending_cursor = Rc::new(Cell::new(None));
    let cursor_update_scheduled = Rc::new(Cell::new(false));
    player_state::connect_named(&player_state, "persist project cursor", move |event| {
        if !matches!(event, player_state::PlayerEvent::State) {
            return;
        }
        let snapshot = player_state::snapshot(&cursor_player_state);
        let position = snapshot
            .position
            .clamp(project::Time::ZERO, snapshot.duration);
        pending_cursor.set(Some(position));
        if cursor_update_scheduled.replace(true) {
            return;
        }
        let cursor_project = cursor_project.clone();
        let pending_cursor = pending_cursor.clone();
        let cursor_update_scheduled = cursor_update_scheduled.clone();
        glib::idle_add_local_once(move || {
            cursor_update_scheduled.set(false);
            let Some(position) = pending_cursor.take() else {
                return;
            };
            let mut project = cursor_project.borrow_mut();
            if project.cursor_position == Some(position) {
                return;
            }
            project.cursor_position = Some(position);
            project::save_view_state(&project);
        });
    });
    let selection_state = selection_state::new();
    let preview_focus = preview_focus::new();
    let property_clipboard = shrimply_property_transfer::new_clipboard();
    let preferences = preferences_store::open_with_defaults();
    let mcp_server = RefCell::new(Some(
        mcp::start(
            project.clone(),
            player_state.clone(),
            selection_state.clone(),
            preferences.clone(),
        )
        .unwrap_or_else(|error| panic!("could not start live MCP bridge: {error}")),
    ));
    window.connect_destroy(move |_| {
        mcp_server.borrow_mut().take();
    });
    shrimply_blender::set_binary(preferences_store::snapshot(&preferences).blender_binary);
    audio::pneuma::set_server_url(&preferences_store::snapshot(&preferences).compute_server_url);
    let audio_levels = Arc::new(audio::AudioLevels::default());

    let video_player = video_player::new(
        project.clone(),
        player_state.clone(),
        playback_performance.clone(),
        selection_state.clone(),
        preview_focus.clone(),
        preferences.clone(),
        audio_levels.clone(),
    );
    let inspector = inspector::new(
        project.clone(),
        player_state.clone(),
        selection_state.clone(),
        preview_focus,
        preferences.clone(),
        video_player.clone(),
        property_clipboard.clone(),
    );
    let top = gtk::Paned::new(gtk::Orientation::Horizontal);
    top.set_wide_handle(false);
    top.set_start_child(Some(&inspector));
    top.set_end_child(Some(&video_player));
    top.set_position(DEFAULT_INSPECTOR_WIDTH);
    top.set_resize_start_child(true);
    top.set_resize_end_child(true);
    top.set_shrink_start_child(false);
    top.set_shrink_end_child(false);

    let timeline = timeline::new(
        project.clone(),
        player_state.clone(),
        playback_performance,
        selection_state.clone(),
        preferences.clone(),
        audio_levels,
        property_clipboard,
    );
    let layout = gtk::Paned::new(gtk::Orientation::Vertical);
    layout.set_wide_handle(false);
    layout.set_start_child(Some(&top));
    layout.set_end_child(Some(&timeline));
    layout.set_position(DEFAULT_TOP_PANEL_HEIGHT);
    layout.set_resize_start_child(true);
    layout.set_resize_end_child(true);
    layout.set_shrink_start_child(false);
    layout.set_shrink_end_child(false);

    let (header_bar, title) = header_bar();
    let inspector_toggle = gtk::ToggleButton::builder()
        .active(true)
        .icon_name("dock-left-symbolic")
        .tooltip_text("Toggle Inspector")
        .build();
    let inspector_for_toggle = inspector.clone();
    inspector_toggle.connect_toggled(move |button| {
        inspector_for_toggle.set_visible(button.is_active());
    });
    let timeline_toggle = gtk::ToggleButton::builder()
        .active(true)
        .icon_name("dock-bottom-symbolic")
        .tooltip_text("Toggle Timeline")
        .build();
    timeline_toggle.connect_toggled(move |button| {
        timeline.set_visible(button.is_active());
    });
    let panel_toggles = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    panel_toggles.append(&inspector_toggle);
    panel_toggles.append(&timeline_toggle);
    header_bar.pack_start(&panel_toggles);
    let project_name = Rc::new(RefCell::new(project.borrow().name.clone()));
    let commit_status = Rc::new(RefCell::new(project::CommitStatus::Idle));
    let status_project_name = project_name.clone();
    let current_commit_status = commit_status.clone();
    let status_title = title.clone();
    let status_window = window.clone();
    project::connect_commit_status(move |status| {
        update_project_title(
            &status_window,
            &status_title,
            &status_project_name.borrow(),
            &status,
        );
        *current_commit_status.borrow_mut() = status;
    });

    let name_project = project.clone();
    let name_window = window.clone();
    let name_commit_status = commit_status.clone();
    player_state::connect_named(&player_state, "editor project name", move |event| {
        if !matches!(event, player_state::PlayerEvent::Project(_)) {
            return;
        }
        let name = name_project.borrow().name.clone();
        if *project_name.borrow() == name {
            return;
        }
        *project_name.borrow_mut() = name.clone();
        update_project_title(&name_window, &title, &name, &name_commit_status.borrow());
        if let Err(error) =
            shrimply_support::recent_projects::touch(&project::active_project_path(), &name)
        {
            tracing::warn!("Could not update recent projects: {error}");
        }
    });

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header_bar);
    toolbar.set_content(Some(&layout));

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&toolbar));

    window.set_visible(false);
    window.set_default_size(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT);
    window.set_content(Some(&toasts));

    header_menu::add(
        &header_bar,
        window,
        &toasts,
        project.clone(),
        player_state.clone(),
        preferences.clone(),
    );
    window.present();
    video_player.grab_focus();
}

fn update_project_title(
    window: &adw::ApplicationWindow,
    title: &gtk::Label,
    project_name: &str,
    status: &project::CommitStatus,
) {
    let label = match status {
        project::CommitStatus::InProgress(action) => format!("{project_name} — {action}"),
        project::CommitStatus::SavePending => format!("{project_name} — Unsaved"),
        project::CommitStatus::Saving => format!("{project_name} — Saving"),
        project::CommitStatus::SaveFailed(_) => {
            format!("{project_name} — Unsaved — Save failed")
        }
        project::CommitStatus::Idle => project_name.to_string(),
    };
    title.set_label(&label);
    title.set_tooltip_text(match status {
        project::CommitStatus::SaveFailed(error) => Some(error),
        _ => None,
    });
    window.set_title(Some(&label));
}

fn begin_project_load(app: &adw::Application, path: PathBuf) {
    shrimply_ui_foundation::icons::register_bundled();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Shrimply")
        .default_width(640)
        .default_height(360)
        .build();
    window.set_content(Some(&project_loading_view(&path)));
    if has_otio_extension(&path) {
        window.present();
        choose_otio_destination(app, &window, path);
        return;
    }
    if has_kdenlive_extension(&path) {
        window.present();
        choose_kdenlive_destination(app, &window, path);
        return;
    }
    let app = app.clone();
    let window_for_load = window.clone();
    window.add_tick_callback(move |_, _| {
        start_project_load(&app, &window_for_load, path.clone());
        glib::ControlFlow::Break
    });
    window.present();
}

fn choose_kdenlive_destination(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    source: PathBuf,
) {
    let label = "Import Kdenlive as Shrimply Project";
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Shrimply projects"));
    filter.add_pattern("*.shrimp");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let initial_name = source
        .file_stem()
        .map(|name| format!("{}.shrimp", name.to_string_lossy()))
        .unwrap_or_else(|| "imported.shrimp".to_string());
    let dialog = gtk::FileDialog::builder()
        .title(label)
        .initial_name(initial_name)
        .filters(&filters)
        .default_filter(&filter)
        .build();
    let app = app.clone();
    let window = window.clone();
    let parent = window.clone();
    shrimply_ui_foundation::file_picker::save(
        label,
        &dialog,
        Some(parent.upcast_ref::<gtk::Window>()),
        move |result| {
            let Ok(file) = result else {
                app.quit();
                return;
            };
            let Some(mut destination) = file.path() else {
                show_project_load_error(
                    &app,
                    &window,
                    "Could not import Kdenlive project",
                    "The selected location does not have a local path.",
                );
                return;
            };
            if destination
                .extension()
                .and_then(|extension| extension.to_str())
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("shrimp"))
            {
                destination.set_extension("shrimp");
            }
            start_kdenlive_import(&app, &window, source.clone(), destination);
        },
    );
}

enum KdenliveImportMessage {
    Progress(&'static str),
    Finished(Result<(PathBuf, Vec<String>), String>),
}

fn start_kdenlive_import(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    source: PathBuf,
    destination: PathBuf,
) {
    let (sender, receiver) = async_channel::bounded(1);
    window.set_content(Some(&project_loading_view_with_subtitle(
        "Reading and converting Kdenlive timeline…",
    )));
    thread::spawn(move || {
        let _ = sender.send_blocking(KdenliveImportMessage::Progress(
            "Reading and converting Kdenlive timeline…",
        ));
        let result: Result<_, String> = (|| {
            let import =
                shrimply_kdenlive::from_file(&source).map_err(|error| error.to_string())?;
            let _ = sender.send_blocking(KdenliveImportMessage::Progress(
                "Validating imported project…",
            ));
            let native =
                project::from_json_value(import.project).map_err(|error| error.to_string())?;
            let _ =
                sender.send_blocking(KdenliveImportMessage::Progress("Writing Shrimply project…"));
            project::create_project_file(&destination, &native)
                .map_err(|error| error.to_string())?;
            Ok((destination, import.warnings))
        })();
        let _ = sender.send_blocking(KdenliveImportMessage::Finished(result));
    });
    let app = app.clone();
    let window = window.clone();
    glib::spawn_future_local(async move {
        while let Ok(message) = receiver.recv().await {
            match message {
                KdenliveImportMessage::Progress(subtitle) => {
                    window.set_content(Some(&project_loading_view_with_subtitle(subtitle)));
                }
                KdenliveImportMessage::Finished(result) => {
                    match result {
                        Ok((path, warnings)) if warnings.is_empty() => {
                            load_project(&app, &window, path)
                        }
                        Ok((path, warnings)) => {
                            show_kdenlive_warnings(&app, &window, path, warnings)
                        }
                        Err(error) => show_project_load_error(
                            &app,
                            &window,
                            "Could not import Kdenlive project",
                            &error,
                        ),
                    }
                    return;
                }
            }
        }
        show_project_load_error(
            &app,
            &window,
            "Could not import Kdenlive project",
            "The Kdenlive importer stopped unexpectedly.",
        );
    });
}

fn show_kdenlive_warnings(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    path: PathBuf,
    warnings: Vec<String>,
) {
    let dialog = adw::AlertDialog::new(
        Some("Kdenlive project imported with limitations"),
        Some(&warnings.join("\n")),
    );
    dialog.add_response("open", "Open Project");
    dialog.set_default_response(Some("open"));
    dialog.set_close_response("open");
    let app = app.clone();
    let window = window.clone();
    let parent = window.clone();
    dialog.choose(
        Some(parent.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        move |_| load_project(&app, &window, path),
    );
}

fn choose_otio_destination(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    source: PathBuf,
) {
    let label = "Import OTIO as Shrimply Project";
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Shrimply projects"));
    filter.add_pattern("*.shrimp");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let initial_name = source
        .file_stem()
        .map(|name| format!("{}.shrimp", name.to_string_lossy()))
        .unwrap_or_else(|| "imported.shrimp".to_string());
    let dialog = gtk::FileDialog::builder()
        .title(label)
        .initial_name(initial_name)
        .filters(&filters)
        .default_filter(&filter)
        .build();
    let app = app.clone();
    let window = window.clone();
    let parent = window.clone();
    shrimply_ui_foundation::file_picker::save(
        label,
        &dialog,
        Some(parent.upcast_ref::<gtk::Window>()),
        move |result| {
            let Ok(file) = result else {
                app.quit();
                return;
            };
            let Some(mut destination) = file.path() else {
                show_project_load_error(
                    &app,
                    &window,
                    "Could not import OTIO",
                    "The selected location does not have a local path.",
                );
                return;
            };
            if destination
                .extension()
                .and_then(|extension| extension.to_str())
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("shrimp"))
            {
                destination.set_extension("shrimp");
            }
            choose_otio_settings(&app, &window, source.clone(), destination);
        },
    );
}

fn choose_otio_settings(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    source: PathBuf,
    destination: PathBuf,
) {
    let selector = ProjectSettingsSelector::new();
    let content = adw::PreferencesGroup::builder()
        .title("Project Settings")
        .build();
    content.add(&selector.preset);
    content.add(&selector.width);
    content.add(&selector.height);
    content.add(&selector.fps);
    let dialog = adw::AlertDialog::builder()
        .heading("OTIO Project Settings")
        .body("OTIO does not include the Kdenlive project profile.")
        .extra_child(&content)
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("import", "Import")]);
    dialog.set_default_response(Some("import"));
    dialog.set_close_response("cancel");
    let app = app.clone();
    let window = window.clone();
    let parent = window.clone();
    dialog.choose(
        Some(parent.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        move |answer| {
            if answer != "import" {
                app.quit();
                return;
            }
            let Some((canvas_size, fps)) = selector.settings() else {
                show_project_load_error(
                    &app,
                    &window,
                    "Could not import OTIO",
                    "The selected project settings are invalid.",
                );
                return;
            };
            start_otio_import(&app, &window, source, destination, canvas_size, fps);
        },
    );
}

fn start_otio_import(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    source: PathBuf,
    destination: PathBuf,
    canvas_size: project::CanvasSize,
    fps: Fraction,
) {
    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        let result = shrimply_otio::from_file(&source, canvas_size, fps).and_then(|import| {
            let native = project::from_json_value(import.project)?;
            project::create_project_file(&destination, &native)?;
            Ok((destination, import.warnings))
        });
        let _ = sender.send_blocking(result);
    });
    let app = app.clone();
    let window = window.clone();
    glib::spawn_future_local(async move {
        let Ok(result) = receiver.recv().await else {
            show_project_load_error(
                &app,
                &window,
                "Could not import OTIO",
                "The OTIO importer stopped unexpectedly.",
            );
            return;
        };
        match result {
            Ok((path, warnings)) if warnings.is_empty() => load_project(&app, &window, path),
            Ok((path, warnings)) => show_otio_warnings(&app, &window, path, warnings),
            Err(error) => show_project_load_error(&app, &window, "Could not import OTIO", &error),
        }
    });
}

fn show_otio_warnings(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    path: PathBuf,
    warnings: Vec<String>,
) {
    let dialog = adw::AlertDialog::new(
        Some("OTIO imported with limitations"),
        Some(&warnings.join("\n")),
    );
    dialog.add_response("open", "Open Project");
    dialog.set_default_response(Some("open"));
    dialog.set_close_response("open");
    let app = app.clone();
    let window = window.clone();
    let parent = window.clone();
    dialog.choose(
        Some(parent.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        move |_| load_project(&app, &window, path),
    );
}

fn has_otio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("otio"))
}

fn has_kdenlive_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("kdenlive"))
}

fn load_project(app: &adw::Application, window: &adw::ApplicationWindow, path: PathBuf) {
    window.set_content(Some(&project_loading_view(&path)));
    start_project_load(app, window, path);
}

fn project_loading_view(path: &Path) -> adw::ToolbarView {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| path.to_string_lossy());
    project_loading_view_with_subtitle(filename.as_ref())
}

fn project_loading_view_with_subtitle(subtitle: &str) -> adw::ToolbarView {
    let spinner = adw::Spinner::new();
    spinner.set_size_request(LOADING_SPINNER_SIZE, LOADING_SPINNER_SIZE);
    let status = gtk::Label::builder()
        .label("Loading project…")
        .css_classes(["title-3"])
        .build();
    let subtitle = gtk::Label::builder()
        .label(subtitle)
        .css_classes(["dim-label"])
        .ellipsize(pango::EllipsizeMode::Middle)
        .max_width_chars(48)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
    body.set_halign(gtk::Align::Center);
    body.set_valign(gtk::Align::Center);
    body.set_hexpand(true);
    body.set_vexpand(true);
    body.append(&spinner);
    body.append(&status);
    body.append(&subtitle);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(
        &adw::HeaderBar::builder()
            .title_widget(&gtk::Box::new(gtk::Orientation::Horizontal, 0))
            .build(),
    );
    toolbar.set_content(Some(&body));
    toolbar
}

fn start_project_load(app: &adw::Application, window: &adw::ApplicationWindow, path: PathBuf) {
    let (sender, receiver) = async_channel::bounded(1);
    let worker_path = path.clone();
    thread::spawn(move || {
        let _ = sender.send_blocking(project::prepare_project(&worker_path));
    });
    let app = app.clone();
    let window = window.clone();
    glib::spawn_future_local(async move {
        let Ok(result) = receiver.recv().await else {
            show_project_load_error(
                &app,
                &window,
                "Could not open project",
                "The project loader stopped unexpectedly.",
            );
            return;
        };
        match result {
            Ok(prepared) => {
                let project = project::activate_project(prepared);
                if let Err(error) = shrimply_support::recent_projects::touch(&path, &project.name) {
                    tracing::warn!("Could not update recent projects: {error}");
                }
                ffmpeg::init().expect("FFmpeg should initialize");
                ffmpeg::util::log::set_level(ffmpeg::util::log::Level::Error);
                build_ui(&window, project);
            }
            Err(project::ProjectLoadError::LockedByOtherInstance { pid }) => {
                show_project_lock_dialog(&app, &window, path, pid);
            }
            Err(project::ProjectLoadError::Other(error)) => {
                show_project_load_error(&app, &window, "Could not open project", &error);
            }
        }
    });
}

fn show_project_lock_dialog(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    path: PathBuf,
    pid: u32,
) {
    let dialog = adw::AlertDialog::builder()
        .heading("Project is in use")
        .body(format!(
            "The project lock is held by another editor process (PID {pid})."
        ))
        .prefer_wide_layout(false)
        .build();
    dialog.add_responses(&[
        ("close", "Close"),
        ("stop", "Stop Other Editor"),
        ("retry", "Retry"),
    ]);
    dialog.set_default_response(Some("retry"));
    dialog.set_close_response("close");
    dialog.set_response_appearance("stop", adw::ResponseAppearance::Destructive);
    let app = app.clone();
    let window = window.clone();
    let parent = window.clone();
    dialog.choose(
        Some(parent.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        move |answer| match answer.as_str() {
            "retry" => load_project(&app, &window, path),
            "stop" => {
                if project::terminate_project_process(pid) {
                    load_project(&app, &window, path);
                } else {
                    let dialog = adw::AlertDialog::new(
                        Some("Could not stop other editor"),
                        Some("Shrimply could not signal the other process."),
                    );
                    dialog.add_response("close", "Close");
                    dialog.set_close_response("close");
                    dialog.set_default_response(Some("close"));
                    let parent = window.clone();
                    dialog.choose(
                        Some(parent.upcast_ref::<gtk::Widget>()),
                        None::<&gio::Cancellable>,
                        move |_| show_project_lock_dialog(&app, &window, path, pid),
                    );
                }
            }
            _ => app.quit(),
        },
    );
}

fn show_project_load_error(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    heading: &str,
    message: &str,
) {
    let dialog = adw::AlertDialog::new(Some(heading), Some(message));
    dialog.add_response("close", "Close");
    dialog.set_close_response("close");
    dialog.set_default_response(Some("close"));
    let app = app.clone();
    dialog.choose(
        Some(window.upcast_ref::<gtk::Widget>()),
        None::<&gio::Cancellable>,
        move |_| app.quit(),
    );
}

fn header_bar() -> (adw::HeaderBar, gtk::Label) {
    let title = gtk::Label::builder()
        .label("Shrimply")
        .css_classes(["title"])
        .build();

    (
        adw::HeaderBar::builder().title_widget(&title).build(),
        title,
    )
}
