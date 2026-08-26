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
use shrimply_math_core::{Time, time_from_frame};
use shrimply_mcp::protocol::{
    ActiveScopeSnapshot, BridgeCommand, BridgeRequest, BridgeResponse, EditRequest, EditResponse,
    LiveSnapshot, PlayerSnapshot, ScopeRef, ViewFrameResponse,
};
use shrimply_preview_ui::video::compositor::{EXPORT_ASSETS_LOADING, VideoExportRenderer};
use shrimply_project::project::Project;
use shrimply_state::{
    player_state::{self, ProjectChange, SharedPlayerState},
    preferences::{self, SharedPreferences},
};
use shrimply_timeline::selection_state::{self, SharedSelectionState};
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
                        BridgeCommand::ViewFrame { frame } => {
                            view_frame(&project, frame, work.canceled.clone()).await
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
            let state = player_state::snapshot(player);
            let project = project.borrow();
            let actual_frame = frame.min(state.duration.as_frame(project.fps));
            let actual = time_from_frame(actual_frame, project.fps)
                .ok_or_else(|| "seek frame exceeds the supported exact range".to_string())?;
            player_state::set_position(player, actual);
            serde_json::to_value(shrimply_mcp::query::frame_time(actual_frame, project.fps)?)
                .map_err(|error| format!("could not serialize playhead: {error}"))
        }
        BridgeCommand::ViewFrame { .. } => {
            unreachable!("frame rendering is prepared asynchronously")
        }
        BridgeCommand::Apply(_) => unreachable!("edit commands are prepared asynchronously"),
    }
}

async fn view_frame(
    live: &Rc<RefCell<Project>>,
    frame: u64,
    canceled: Arc<AtomicBool>,
) -> Result<Value, String> {
    let project = live.borrow().clone();
    let frame = frame.min(project.duration().as_frame(project.fps));
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
