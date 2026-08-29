use std::cell::RefCell;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use gtk::{gdk, glib, prelude::*};
use serde_json::{Value, json};
use shrimply_asset::Asset;
use shrimply_math_core::{Time, fraction_new, time_from_frame};
use shrimply_mcp::protocol::{
    ActiveScopeSnapshot, AnalyzeTransparentFillRequest, AnalyzeTransparentFillResponse,
    BridgeCommand, BridgeRequest, BridgeResponse, EditOperationResult, EditRequest, EditResponse,
    GenerateTtsRequest, ListTtsModelsResponse, LiveSnapshot, PlayerSnapshot, ScopeRef,
    TtsInputValue, ViewFrameResponse,
};
use shrimply_preview_ui::video::compositor::{EXPORT_ASSETS_LOADING, VideoExportRenderer};
use shrimply_project::project::Project;
use shrimply_state::{
    player_state::{self, ProjectChange, SharedPlayerState},
    preferences::{self, SharedPreferences},
};
use shrimply_timeline::selection_state::{self, SharedSelectionState};
use shrimply_video::transparent_fill_analysis::Status as TransparentFillStatus;
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect};
use uuid::Uuid;

mod imports;

const SOCKET_READ_LIMIT: u64 = 16 * 1024 * 1024;
const SOCKET_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const EDIT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(29 * 60);
const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const WORK_QUEUE_CAPACITY: usize = 32;
const FRAME_RENDER_AUDIO_SAMPLE_RATE: u32 = 48_000;

struct Work {
    request: BridgeRequest,
    response: mpsc::Sender<BridgeResponse>,
    canceled: Arc<AtomicBool>,
}

struct EditorSelection {
    items: Vec<shrimply_project::project::ItemAddress>,
    focused_item: Option<shrimply_project::project::ItemAddress>,
    tracks: Vec<shrimply_project::project::TrackAddress>,
    focused_track: Option<shrimply_project::project::TrackAddress>,
    gap: Option<selection_state::TrackAddressGap>,
    active_scope: shrimply_project::project::SequenceScopeId,
}

impl EditorSelection {
    fn capture(selection: &SharedSelectionState, project: &Project) -> Self {
        Self {
            items: selection_state::selected_item_addresses(selection, project),
            focused_item: selection_state::focused_item_address(selection, project),
            tracks: selection_state::selected_track_addresses(selection, project),
            focused_track: selection_state::focused_track_address(selection, project),
            gap: selection_state::selected_gap_address(selection, project),
            active_scope: selection_state::active_scope(selection),
        }
    }

    fn reconcile(mut self, project: &Project) -> Self {
        self.items.retain(|address| project.item(address).is_some());
        self.focused_item = self
            .focused_item
            .filter(|address| self.items.contains(address));
        self.tracks
            .retain(|address| project.track(address).is_some());
        self.focused_track = self
            .focused_track
            .filter(|address| self.tracks.contains(address));
        self.gap = self.gap.filter(|gap| project.track(&gap.track).is_some());
        if project.sequence_id_for_scope(&self.active_scope).is_none() {
            self.active_scope = shrimply_project::project::SequenceScopeId::root();
        }
        self
    }

    fn restore(self, selection: &SharedSelectionState, project: &Project) {
        let items = self.items;
        let focused_item = self.focused_item;
        let tracks = self.tracks;
        let focused_track = self.focused_track;
        if !items.is_empty() {
            selection_state::set_selected_item_addresses(selection, project, items, focused_item);
        } else if !tracks.is_empty() {
            selection_state::set_selected_track_addresses(
                selection,
                project,
                tracks,
                focused_track,
            );
        } else if self.gap.is_some() {
            selection_state::set_selected_gap_address(selection, project, self.gap);
        } else {
            selection_state::set_selected_items(selection, Vec::new(), None);
            selection_state::set_active_scope(selection, self.active_scope);
        }
    }
}

pub struct Server {
    socket_path: PathBuf,
    stop: Arc<AtomicBool>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.socket_path);
        let _ = fs::remove_file(&self.socket_path);
    }
}

struct BoundSocket(PathBuf);

impl BoundSocket {
    fn keep(mut self) -> PathBuf {
        std::mem::take(&mut self.0)
    }
}

impl Drop for BoundSocket {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.0);
        }
    }
}

pub fn start(
    project: Rc<RefCell<Project>>,
    player_state: SharedPlayerState,
    selection_state: SharedSelectionState,
    preferences: SharedPreferences,
) -> Result<Server, String> {
    runtime_directory()?;
    let socket_path =
        shrimply_mcp::bridge::socket_path(std::process::id()).map_err(|error| error.to_string())?;
    if socket_path.exists() {
        let metadata = socket_path.metadata().map_err(|error| {
            format!(
                "could not inspect MCP socket {}: {error}",
                socket_path.display()
            )
        })?;
        if metadata.uid() != effective_uid() {
            return Err(format!(
                "refusing MCP socket owned by another user: {}",
                socket_path.display()
            ));
        }
        return Err(format!(
            "MCP socket already exists: {}",
            socket_path.display()
        ));
    }
    let listener = UnixListener::bind(&socket_path).map_err(|error| {
        format!(
            "could not bind MCP socket {}: {error}",
            socket_path.display()
        )
    })?;
    let socket = BoundSocket(socket_path);
    fs::set_permissions(&socket.0, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "could not secure MCP socket {}: {error}",
            socket.0.display()
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("could not configure MCP socket: {error}"))?;

    let (sender, receiver) = async_channel::bounded::<Work>(WORK_QUEUE_CAPACITY);
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    drop(
        thread::Builder::new()
            .name("shrimply-mcp-bridge".to_string())
            .spawn(move || accept_loop(listener, sender, worker_stop))
            .map_err(|error| format!("could not start MCP bridge: {error}"))?,
    );

    glib::spawn_future_local(async move {
        while let Ok(work) = receiver.recv().await {
            let project_path = shrimply_project::project::normalized_project_path(
                &shrimply_project::project::active_project_path(),
            );
            let project_path_text = project_path
                .to_str()
                .ok_or_else(|| "active project path is not valid UTF-8".to_string());
            let result = if work.canceled.load(Ordering::Acquire) {
                Err("MCP client canceled the request".to_string())
            } else {
                match project_path_text.as_deref() {
                    Ok(path) if path == work.request.project_path => match work.request.command {
                        BridgeCommand::Apply(request) => {
                            apply_edit(
                                &project,
                                &player_state,
                                &selection_state,
                                preferences::snapshot(&preferences).default_visual_duration,
                                request,
                                work.canceled.clone(),
                            )
                            .await
                        }
                        BridgeCommand::ListTtsModels => {
                            list_tts_models(&preferences, work.canceled.clone()).await
                        }
                        BridgeCommand::GenerateTts(request) => {
                            generate_tts(
                                &project,
                                &player_state,
                                &selection_state,
                                &preferences,
                                request,
                                work.canceled.clone(),
                            )
                            .await
                        }
                        BridgeCommand::ViewFrame { frame } => {
                            view_frame(&project, frame, work.canceled.clone()).await
                        }
                        BridgeCommand::AnalyzeTransparentFill(request) => {
                            analyze_transparent_fill(
                                &project,
                                &player_state,
                                request,
                                work.canceled.clone(),
                            )
                            .await
                        }
                        command => handle_command(
                            &project,
                            &player_state,
                            &selection_state,
                            command,
                            &work.canceled,
                        ),
                    },
                    Ok(_) => Err(format!(
                        "MCP request named {}, but this editor owns {}",
                        work.request.project_path,
                        project_path.display()
                    )),
                    Err(error) => Err(error.clone()),
                }
            };
            let response = BridgeResponse {
                project_path: project_path_text.unwrap_or_default().to_string(),
                result: result.as_ref().ok().cloned(),
                error: result.err(),
            };
            let _ = work.response.send(response);
        }
    });

    let socket_path = socket.keep();
    Ok(Server { socket_path, stop })
}

fn runtime_directory() -> Result<PathBuf, String> {
    let root = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "XDG_RUNTIME_DIR is not set; cannot expose the live MCP bridge".to_string()
        })?;
    let directory = root.join("shrimply");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    let metadata = directory
        .metadata()
        .map_err(|error| format!("could not inspect {}: {error}", directory.display()))?;
    if metadata.uid() != effective_uid() {
        return Err(format!(
            "runtime directory is owned by another user: {}",
            directory.display()
        ));
    }
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not secure {}: {error}", directory.display()))?;
    Ok(directory)
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid has no arguments and no memory safety preconditions.
    unsafe { libc::geteuid() }
}

fn accept_loop(listener: UnixListener, sender: async_channel::Sender<Work>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok(_) if stop.load(Ordering::Acquire) => break,
            Ok((stream, _)) => {
                let sender = sender.clone();
                let stop = stop.clone();
                thread::spawn(move || serve_connection(stream, &sender, &stop));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("MCP bridge accept failed: {error}"),
        }
    }
}

fn serve_connection(
    mut stream: UnixStream,
    sender: &async_channel::Sender<Work>,
    stop: &AtomicBool,
) {
    stream
        .set_read_timeout(Some(SOCKET_REQUEST_TIMEOUT))
        .expect("MCP request timeout must be configurable");
    stream
        .set_write_timeout(Some(SOCKET_REQUEST_TIMEOUT))
        .expect("MCP response timeout must be configurable");
    let response = (|| {
        let mut line = String::new();
        BufReader::new(&stream)
            .take(SOCKET_READ_LIMIT)
            .read_line(&mut line)
            .map_err(|error| format!("could not read MCP bridge request: {error}"))?;
        let request: BridgeRequest = serde_json::from_str(&line)
            .map_err(|error| format!("malformed MCP bridge request: {error}"))?;
        let (response, receiver) = mpsc::channel();
        let canceled = Arc::new(AtomicBool::new(false));
        sender
            .send_blocking(Work {
                request,
                response,
                canceled: canceled.clone(),
            })
            .map_err(|_| "GTK MCP executor stopped".to_string())?;
        stream
            .set_nonblocking(true)
            .map_err(|error| format!("could not monitor MCP client: {error}"))?;
        let deadline = Instant::now() + EDIT_EXECUTION_TIMEOUT;
        loop {
            if stop.load(Ordering::Acquire) {
                canceled.store(true, Ordering::Release);
                break Err("editor MCP bridge is shutting down".to_string());
            }
            match receiver.recv_timeout(CANCELLATION_POLL_INTERVAL) {
                Ok(response) => break Ok(response),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    canceled.store(true, Ordering::Release);
                    break Err("editor MCP executor dropped the response".to_string());
                }
                Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() >= deadline => {
                    canceled.store(true, Ordering::Release);
                    break Err("editor MCP response timed out".to_string());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let mut byte = [0];
                    match stream.read(&mut byte) {
                        Ok(0) => {
                            canceled.store(true, Ordering::Release);
                            break Err("MCP client disconnected".to_string());
                        }
                        Ok(_) => {
                            canceled.store(true, Ordering::Release);
                            break Err("MCP client sent more than one request".to_string());
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(error) => {
                            canceled.store(true, Ordering::Release);
                            break Err(format!("could not monitor MCP client: {error}"));
                        }
                    }
                }
            }
        }
    })()
    .unwrap_or_else(|error| BridgeResponse {
        project_path: String::new(),
        result: None,
        error: Some(error),
    });
    let _ = stream.set_nonblocking(false);
    if serde_json::to_writer(&mut stream, &response).is_ok() {
        let _ = stream.write_all(b"\n");
    }
}

fn handle_command(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    selection: &SharedSelectionState,
    command: BridgeCommand,
    canceled: &AtomicBool,
) -> Result<Value, String> {
    if canceled.load(Ordering::Acquire) {
        return Err("MCP client canceled the request".to_string());
    }
    match command {
        BridgeCommand::Handshake => Ok(json!({ "connected": true })),
        BridgeCommand::Snapshot => serde_json::to_value(snapshot(project, player, selection)?)
            .map_err(|error| format!("could not serialize live snapshot: {error}")),
        BridgeCommand::Seek { frame } => {
            let project = project.borrow();
            let position = time_from_frame(frame, project.fps)
                .ok_or_else(|| "frame exceeds the supported exact range".to_string())?;
            player_state::seek_time(player, position);
            serde_json::to_value(shrimply_mcp::query::frame_time(frame, project.fps)?)
                .map_err(|error| format!("could not serialize playhead: {error}"))
        }
        BridgeCommand::ViewFrame { .. } => {
            unreachable!("frame rendering is prepared asynchronously")
        }
        BridgeCommand::AnalyzeTransparentFill(_) => {
            unreachable!("modifier analysis is prepared asynchronously")
        }
        BridgeCommand::ListTtsModels => {
            unreachable!("TTS model discovery is prepared asynchronously")
        }
        BridgeCommand::GenerateTts(_) => {
            unreachable!("TTS generation is prepared asynchronously")
        }
        BridgeCommand::Apply(_) => unreachable!("edit commands are prepared asynchronously"),
    }
}

async fn analyze_transparent_fill(
    live: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    request: AnalyzeTransparentFillRequest,
    canceled: Arc<AtomicBool>,
) -> Result<Value, String> {
    let address = shrimply_mcp::query::model_item_address(&request.address)?;
    let modifier_id = Uuid::parse_str(&request.modifier_id)
        .map_err(|error| format!("invalid modifier_id {:?}: {error}", request.modifier_id))?;
    let mut project = live.borrow().clone();
    let fill = project
        .video_item_mut(&address)
        .ok_or_else(|| "Transparent Fill analysis requires a video clip address".to_string())?
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == modifier_id)
        .ok_or_else(|| format!("modifier {modifier_id} does not exist on the addressed clip"))
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::TransparentFill(fill) => Ok(fill),
                _ => Err(format!("modifier {modifier_id} is not Transparent Fill")),
            },
            _ => Err(format!("modifier {modifier_id} is not Transparent Fill")),
        })?;
    if fill.points.is_empty() {
        return Err("add at least one transparent fill point before analyzing".to_string());
    }
    fill.analysis_generation = fill.analysis_generation.wrapping_add(1).max(1);
    let generation = fill.analysis_generation;
    shrimply_project::project::commit_edit_checked(&project, "MCP analyze Transparent Fill")?;
    *live.borrow_mut() = project.clone();
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    shrimply_video::transparent_fill_analysis::analyze(project, &address, modifier_id)?;

    loop {
        if canceled.load(Ordering::Acquire) {
            shrimply_video::transparent_fill_analysis::cancel(modifier_id);
            return Err("MCP client canceled Transparent Fill analysis".to_string());
        }
        let status = {
            let project = live.borrow();
            shrimply_video::transparent_fill_analysis::status(&project, &address, modifier_id)
        };
        match status {
            TransparentFillStatus::Running { .. } => {
                glib::timeout_future(CANCELLATION_POLL_INTERVAL).await;
            }
            TransparentFillStatus::Complete => break,
            TransparentFillStatus::Failed(error) => {
                return Err(format!("Transparent Fill analysis failed: {error}"));
            }
            TransparentFillStatus::Cancelled => {
                return Err("Transparent Fill analysis was canceled".to_string());
            }
            TransparentFillStatus::Missing => {
                shrimply_video::transparent_fill_analysis::cancel(modifier_id);
                return Err(
                    "Transparent Fill inputs changed while analysis was running; retry analysis"
                        .to_string(),
                );
            }
        }
    }

    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    serde_json::to_value(AnalyzeTransparentFillResponse {
        address: request.address,
        modifier_id: request.modifier_id,
        analysis_generation: generation,
        revision: player_state::snapshot(player).revision,
    })
    .map_err(|error| format!("could not serialize Transparent Fill analysis result: {error}"))
}

async fn view_frame(
    live: &Rc<RefCell<Project>>,
    frame: u64,
    canceled: Arc<AtomicBool>,
) -> Result<Value, String> {
    let project = live.borrow().clone();
    let position = time_from_frame(frame, project.fps)
        .ok_or_else(|| "frame exceeds the supported exact range".to_string())?;
    let fps = project.fps;
    let canvas = project.canvas_size;
    let (sender, receiver) = async_channel::bounded(1);
    let render_canceled = canceled.clone();
    thread::Builder::new()
        .name("shrimply-mcp-frame".to_string())
        .spawn(move || {
            let result = (|| {
                let mut renderer = VideoExportRenderer::new(FRAME_RENDER_AUDIO_SAMPLE_RATE)?;
                let rendered = loop {
                    if render_canceled.load(Ordering::Acquire) {
                        return Err("MCP client canceled the frame render".to_string());
                    }
                    match renderer.render(&project, position, 0) {
                        Ok(frame) => break frame,
                        Err(error) if error == EXPORT_ASSETS_LOADING => thread::yield_now(),
                        Err(error) => return Err(error),
                    }
                };
                let mut rgba = ffmpeg_next::frame::Video::new(
                    ffmpeg_next::format::Pixel::RGBA,
                    canvas.width,
                    canvas.height,
                );
                renderer.copy_to_rgba_frame(rendered, &mut rgba)?;
                let row_bytes = canvas.width as usize * std::mem::size_of::<u32>();
                let mut pixels = Vec::with_capacity(row_bytes * canvas.height as usize);
                for row in rgba
                    .data(0)
                    .chunks_exact(rgba.stride(0))
                    .take(canvas.height as usize)
                {
                    pixels.extend_from_slice(&row[..row_bytes]);
                }
                Ok::<_, String>(pixels)
            })();
            let _ = sender.send_blocking(result);
        })
        .map_err(|error| format!("could not start MCP frame renderer: {error}"))?;

    let pixels = loop {
        if canceled.load(Ordering::Acquire) {
            return Err("MCP client canceled the frame render".to_string());
        }
        match receiver.try_recv() {
            Ok(result) => break result?,
            Err(async_channel::TryRecvError::Empty) => {
                glib::timeout_future(CANCELLATION_POLL_INTERVAL).await;
            }
            Err(async_channel::TryRecvError::Closed) => {
                return Err("MCP frame renderer stopped without a result".to_string());
            }
        }
    };
    let width = i32::try_from(canvas.width).map_err(|_| "canvas width is too large")?;
    let height = i32::try_from(canvas.height).map_err(|_| "canvas height is too large")?;
    let texture = gdk::MemoryTexture::new(
        width,
        height,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(pixels),
        canvas.width as usize * std::mem::size_of::<u32>(),
    );
    let png = glib::base64_encode(texture.save_to_png_bytes().as_ref()).to_string();
    serde_json::to_value(ViewFrameResponse {
        frame: shrimply_mcp::query::frame_time(frame, fps)?,
        png,
    })
    .map_err(|error| format!("could not serialize rendered frame: {error}"))
}

fn snapshot(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    selection: &SharedSelectionState,
) -> Result<LiveSnapshot, String> {
    let project = project.borrow();
    let player_snapshot = player_state::snapshot(player);
    let active_scope = selection_state::active_scope(selection);
    let asset_revisions = project
        .assets()
        .into_iter()
        .filter_map(|asset| {
            asset.snapshot().ok().map(|snapshot| {
                (
                    asset.path().to_string_lossy().into_owned(),
                    snapshot.revision(),
                )
            })
        })
        .collect();
    Ok(LiveSnapshot {
        project_path: shrimply_project::project::normalized_project_path(
            &shrimply_project::project::active_project_path(),
        )
        .to_str()
        .ok_or_else(|| "active project path is not valid UTF-8".to_string())?
        .to_string(),
        project: project.clone(),
        player: PlayerSnapshot {
            position: player_snapshot.position,
            duration: player_snapshot.duration,
            playing: player_snapshot.playing,
            revision: player_snapshot.revision,
        },
        active_scope: ActiveScopeSnapshot {
            instance_path: active_scope
                .instance_ids()
                .iter()
                .map(Uuid::to_string)
                .collect(),
            video_paths: project
                .sequence_paths_for_scope(shrimply_project::project::ItemKind::Video, &active_scope)
                .into_iter()
                .map(|path| ScopeRef {
                    sequence_path: path.iter().map(Uuid::to_string).collect(),
                })
                .collect(),
            audio_paths: project
                .sequence_paths_for_scope(shrimply_project::project::ItemKind::Audio, &active_scope)
                .into_iter()
                .map(|path| ScopeRef {
                    sequence_path: path.iter().map(Uuid::to_string).collect(),
                })
                .collect(),
        },
        focused_item: selection_state::focused_item_address(selection, &project)
            .as_ref()
            .map(shrimply_mcp::query::protocol_item_address),
        selected_items: selection_state::selected_item_addresses(selection, &project)
            .iter()
            .map(shrimply_mcp::query::protocol_item_address)
            .collect(),
        focused_track: selection_state::focused_track_address(selection, &project)
            .as_ref()
            .map(shrimply_mcp::query::protocol_track_address),
        selected_tracks: selection_state::selected_track_addresses(selection, &project)
            .iter()
            .map(shrimply_mcp::query::protocol_track_address)
            .collect(),
        asset_revisions,
    })
}

async fn list_tts_models(
    preferences: &SharedPreferences,
    canceled: Arc<AtomicBool>,
) -> Result<Value, String> {
    let preferences = preferences::snapshot(preferences);
    let server_url = preferences.compute_server_url;
    let preferred = preferences.last_tts_model;
    let (sender, receiver) = async_channel::bounded(1);
    thread::Builder::new()
        .name("shrimply-mcp-tts-models".to_string())
        .spawn(move || {
            let _ = sender.send_blocking(shrimply_tts::models(&server_url));
        })
        .map_err(|error| format!("could not start TTS model discovery: {error}"))?;
    let models = loop {
        if canceled.load(Ordering::Acquire) {
            return Err("MCP client canceled TTS model discovery".to_string());
        }
        match receiver.try_recv() {
            Ok(result) => break result?,
            Err(async_channel::TryRecvError::Empty) => {
                glib::timeout_future(CANCELLATION_POLL_INTERVAL).await;
            }
            Err(async_channel::TryRecvError::Closed) => {
                return Err("TTS model discovery worker stopped without a result".to_string());
            }
        }
    };
    let default_model = models
        .iter()
        .find(|model| model.id == preferred)
        .or_else(|| models.first())
        .map(|model| model.id.clone());
    let models = models
        .into_iter()
        .map(|model| {
            serde_json::to_value(model)
                .map_err(|error| format!("could not serialize TTS model: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_value(ListTtsModelsResponse {
        models,
        default_model,
    })
    .map_err(|error| format!("could not serialize TTS model response: {error}"))
}

struct StagedSpeech {
    staging: PathBuf,
    final_path: PathBuf,
    promoted: bool,
}

impl StagedSpeech {
    fn promote(&mut self) -> Result<(), String> {
        fs::rename(&self.staging, &self.final_path).map_err(|error| {
            format!(
                "could not promote generated speech {}: {error}",
                self.final_path.display()
            )
        })?;
        self.promoted = true;
        Ok(())
    }

    fn rollback(&mut self) {
        fs::rename(&self.final_path, &self.staging).unwrap_or_else(|error| {
            panic!(
                "could not roll back generated speech {}: {error}",
                self.final_path.display()
            )
        });
        self.promoted = false;
    }
}

impl Drop for StagedSpeech {
    fn drop(&mut self) {
        if !self.promoted && self.staging.exists() {
            fs::remove_file(&self.staging).unwrap_or_else(|error| {
                panic!(
                    "could not clean staged generated speech {}: {error}",
                    self.staging.display()
                )
            });
        }
    }
}

struct GeneratedTts {
    speech: StagedSpeech,
    duration: Time,
    settings: shrimply_tts::TtsSettings,
}

async fn generate_tts(
    live: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    selection: &SharedSelectionState,
    preferences: &SharedPreferences,
    request: GenerateTtsRequest,
    canceled: Arc<AtomicBool>,
) -> Result<Value, String> {
    if request.text.trim().is_empty() {
        return Err("text must not be empty".to_string());
    }
    let project_path = shrimply_project::project::normalized_project_path(
        &shrimply_project::project::active_project_path(),
    );
    let mut project = live.borrow().clone();
    let original = project_content_fingerprint(&project)?;
    let playhead_frame = player_state::current_time(player).as_frame(project.fps);
    let active_scope = selection_state::active_scope(selection);
    let preferences = preferences::snapshot(preferences);
    let cancellation =
        shrimply_server_client::CancellationToken::new(&preferences.compute_server_url)?;
    let worker_cancellation = cancellation.clone();
    let worker_path = project_path.clone();
    let worker_request = request.clone();
    let (sender, receiver) = async_channel::bounded(1);
    thread::Builder::new()
        .name("shrimply-mcp-generate-tts".to_string())
        .spawn(move || {
            let result = prepare_generated_tts(
                worker_path,
                preferences.compute_server_url,
                preferences.last_tts_model,
                worker_request,
                worker_cancellation,
            );
            let _ = sender.send_blocking(result);
        })
        .map_err(|error| format!("could not start TTS generation worker: {error}"))?;
    let mut cancellation_sent = false;
    let mut generated = loop {
        if canceled.load(Ordering::Acquire) && !cancellation_sent {
            cancellation.cancel();
            cancellation_sent = true;
        }
        match receiver.try_recv() {
            Ok(result) => break result?,
            Err(async_channel::TryRecvError::Empty) => {
                glib::timeout_future(CANCELLATION_POLL_INTERVAL).await;
            }
            Err(async_channel::TryRecvError::Closed) => {
                return Err("TTS generation worker stopped without a result".to_string());
            }
        }
    };
    if canceled.load(Ordering::Acquire) {
        return Err("MCP client canceled TTS generation".to_string());
    }
    let current_path = shrimply_project::project::normalized_project_path(
        &shrimply_project::project::active_project_path(),
    );
    if current_path != project_path {
        return Err("project path changed while TTS was being generated".to_string());
    }
    if project_content_fingerprint(&live.borrow())? != original {
        return Err("project changed while TTS was being generated; retry the request".to_string());
    }
    let editor_selection = EditorSelection::capture(selection, &project);
    let mutation = imports::insert_generated_tts(
        &mut project,
        &request,
        playhead_frame,
        active_scope,
        generated.duration,
        generated.settings,
        generated.speech.final_path.clone(),
    )?;
    project
        .validate()
        .map_err(|error| format!("generated TTS edit is invalid: {error}"))?;
    let changed_presentations = shrimply_mcp::query::presentations_affected_by_items(
        &project,
        &mutation.changed_item_ids.iter().copied().collect(),
    )?;
    let mut presentations = changed_presentations.clone();
    presentations.extend(mutation.deleted_presentations.clone());
    let result = EditOperationResult {
        index: 0,
        operation: "generate_tts".to_string(),
        changed_addresses: changed_presentations
            .iter()
            .map(|clip| clip.address.clone())
            .collect(),
        deleted_addresses: mutation.deleted_addresses,
        changed_tracks: mutation.changed_tracks,
        presentations,
    };
    let duration = project.duration();
    let frame_rate = project.fps;
    let duration_frame = shrimply_mcp::query::frame_time_from_time(duration, frame_rate, true);
    if canceled.load(Ordering::Acquire) {
        return Err("MCP client canceled TTS generation before commit".to_string());
    }
    generated.speech.promote()?;
    if canceled.load(Ordering::Acquire) {
        generated.speech.rollback();
        return Err("MCP client canceled TTS generation before commit".to_string());
    }
    if let Err(error) = shrimply_project::project::commit_edit_checked(&project, "MCP generate TTS")
    {
        generated.speech.rollback();
        return Err(format!("MCP TTS edit could not be committed: {error}"));
    }
    *live.borrow_mut() = project;
    editor_selection.restore(selection, &live.borrow());
    player_state::refresh_project(
        player,
        ProjectChange {
            duration: Some(duration),
            frame_rate: Some(frame_rate),
            audio: true,
            audio_beats: true,
            audio_waveforms: true,
            inspector: true,
            ..Default::default()
        },
    );
    serde_json::to_value(EditResponse {
        operations: vec![result],
        duration: duration_frame,
        revision: player_state::snapshot(player).revision,
    })
    .map_err(|error| format!("could not serialize TTS edit result: {error}"))
}

fn prepare_generated_tts(
    project_path: PathBuf,
    server_url: String,
    preferred_model: String,
    request: GenerateTtsRequest,
    cancellation: shrimply_server_client::CancellationToken,
) -> Result<GeneratedTts, String> {
    let models = shrimply_tts::models(&server_url)?;
    let model = request
        .model
        .as_ref()
        .and_then(|id| models.iter().find(|model| &model.id == id))
        .or_else(|| models.iter().find(|model| model.id == preferred_model))
        .or_else(|| models.first())
        .cloned()
        .ok_or_else(|| "the compute server provided no TTS models".to_string())?;
    if let Some(requested) = &request.model
        && requested != &model.id
    {
        return Err(format!("TTS model {requested:?} is not available"));
    }
    let mut settings = shrimply_tts::TtsSettings::default();
    shrimply_tts::sync_settings(&mut settings, &model);
    for (key, input) in request.inputs {
        let definition = model
            .inputs
            .iter()
            .find(|definition| definition.key() == key)
            .ok_or_else(|| format!("TTS model {} has no input {key:?}", model.id))?;
        if definition.purpose() == Some(shrimply_tts::InputPurpose::Text) {
            return Err(format!(
                "TTS input {key:?} is controlled by the top-level text field"
            ));
        }
        let value = match (definition, input) {
            (shrimply_tts::InputDefinition::Text { .. }, TtsInputValue::Text { value }) => {
                shrimply_tts::TtsValue::Text { value }
            }
            (shrimply_tts::InputDefinition::Select { .. }, TtsInputValue::Select { value }) => {
                shrimply_tts::TtsValue::Select { value }
            }
            (shrimply_tts::InputDefinition::Audio { .. }, TtsInputValue::Audio { path }) => {
                let path = PathBuf::from(&path).canonicalize().map_err(|error| {
                    format!("could not resolve TTS audio input {path:?}: {error}")
                })?;
                if !path.is_file() {
                    return Err(format!(
                        "TTS audio input is not a regular file: {}",
                        path.display()
                    ));
                }
                shrimply_tts::TtsValue::Audio {
                    value: Asset::new(path),
                }
            }
            (shrimply_tts::InputDefinition::Toggle { .. }, TtsInputValue::Toggle { value }) => {
                shrimply_tts::TtsValue::Toggle { value }
            }
            (shrimply_tts::InputDefinition::Number { .. }, TtsInputValue::Number { value }) => {
                if value.denominator <= 0 {
                    return Err(format!(
                        "TTS numeric input {key:?} requires a positive denominator"
                    ));
                }
                shrimply_tts::TtsValue::Number {
                    value: fraction_new(value.numerator, value.denominator),
                }
            }
            (shrimply_tts::InputDefinition::Table { .. }, TtsInputValue::Table { rows }) => {
                shrimply_tts::TtsValue::Table { rows }
            }
            _ => return Err(format!("TTS input {key:?} has the wrong value type")),
        };
        settings.inputs.insert(key, value);
    }
    shrimply_tts::set_text(&mut settings, &model, request.text);
    let speech_request = shrimply_tts::speech_request(
        &model,
        &settings,
        shrimply_audio::recording::transcode_to_wav,
    )?;
    let speech = shrimply_tts::synthesize(&server_url, &cancellation, &speech_request, |_| {
        !cancellation.is_cancelled()
    })?;
    let directory = project_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("media/tts");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    let id = Uuid::new_v4();
    let staging = directory.join(format!(".{id}.staging.opus"));
    let final_path = directory.join(format!("{id}.opus"));
    if staging.exists() || final_path.exists() {
        return Err("generated TTS destination already exists".to_string());
    }
    let duration = shrimply_audio::recording::save_wav_as_opus(&speech.wav, &staging)?;
    shrimply_tts::apply_speed_factor(&mut settings, &model, speech.speed_factor);
    Ok(GeneratedTts {
        speech: StagedSpeech {
            staging,
            final_path,
            promoted: false,
        },
        duration,
        settings,
    })
}

async fn apply_edit(
    live: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    selection: &SharedSelectionState,
    default_visual_duration: Time,
    request: EditRequest,
    canceled: Arc<AtomicBool>,
) -> Result<Value, String> {
    if canceled.load(Ordering::Acquire) {
        return Err("MCP client canceled the edit".to_string());
    }
    let project_path = shrimply_project::project::normalized_project_path(
        &shrimply_project::project::active_project_path(),
    );
    let project = live.borrow().clone();
    let original = project_content_fingerprint(&project)?;
    let playhead_frame = player_state::current_time(player).as_frame(project.fps);
    let active_scope = selection_state::active_scope(selection);
    let history_label = request.history_label.clone();
    let worker_path = project_path.clone();
    let (sender, receiver) = async_channel::bounded(1);
    thread::Builder::new()
        .name("shrimply-mcp-edit".to_string())
        .spawn(move || {
            let result = imports::prepare(
                project,
                &request,
                playhead_frame,
                active_scope,
                default_visual_duration,
                &worker_path,
            );
            let _ = sender.send_blocking(result);
        })
        .map_err(|error| format!("could not start MCP edit worker: {error}"))?;

    let mut prepared = loop {
        if canceled.load(Ordering::Acquire) {
            return Err("MCP client canceled the edit".to_string());
        }
        match receiver.try_recv() {
            Ok(result) => break result?,
            Err(async_channel::TryRecvError::Empty) => {
                glib::timeout_future(CANCELLATION_POLL_INTERVAL).await;
            }
            Err(async_channel::TryRecvError::Closed) => {
                return Err("MCP edit worker stopped without a result".to_string());
            }
        }
    };
    let current_path = shrimply_project::project::normalized_project_path(
        &shrimply_project::project::active_project_path(),
    );
    if current_path != project_path {
        return Err("project path changed while the MCP edit was being prepared".to_string());
    }
    {
        let current = live.borrow();
        if project_content_fingerprint(&current)? != original {
            return Err(
                "project changed while the MCP edit was being prepared; retry the edit".to_string(),
            );
        }
        prepared.project.cursor_position = current.cursor_position;
        prepared.project.timeline_zoom = current.timeline_zoom;
        prepared.project.expanded_sequence_paths = current.expanded_sequence_paths.clone();
    }
    let duration = prepared.project.duration();
    let frame_rate = prepared.project.fps;
    let duration_frame =
        shrimply_mcp::query::frame_time_from_time(duration, prepared.project.fps, true);
    let results = prepared.results()?;
    prepared.ensure_linked_sources_current()?;
    if canceled.load(Ordering::Acquire) {
        return Err("MCP client canceled the edit before commit".to_string());
    }
    let editor_selection =
        EditorSelection::capture(selection, &live.borrow()).reconcile(&prepared.project);
    prepared.promote()?;
    if canceled.load(Ordering::Acquire) {
        prepared.discard_promoted();
        return Err("MCP client canceled the edit before commit".to_string());
    }
    if let Err(error) =
        shrimply_project::project::commit_edit_checked(&prepared.project, &history_label)
    {
        prepared.discard_promoted();
        return Err(format!(
            "MCP edit could not be committed to project history: {error}"
        ));
    }
    *live.borrow_mut() = prepared.project;
    editor_selection.restore(selection, &live.borrow());
    player_state::refresh_project(
        player,
        ProjectChange {
            duration: Some(duration),
            frame_rate: Some(frame_rate),
            audio: true,
            audio_beats: true,
            audio_waveforms: true,
            video: true,
            live_preview: true,
            captions: true,
            inspector: true,
        },
    );
    serde_json::to_value(EditResponse {
        operations: results,
        duration: duration_frame,
        revision: player_state::snapshot(player).revision,
    })
    .map_err(|error| format!("could not serialize MCP edit result: {error}"))
}

fn project_content_fingerprint(project: &Project) -> Result<Vec<u8>, String> {
    let mut content = project.clone();
    content.cursor_position = None;
    content.timeline_zoom = None;
    content.expanded_sequence_paths.clear();
    serde_json::to_vec(&content)
        .map_err(|error| format!("could not fingerprint live project: {error}"))
}
