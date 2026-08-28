use rmcp::handler::server::wrapper::Json;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, handler::server::wrapper::Parameters,
    model::*, service::RequestContext, tool, tool_handler, tool_router,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bridge::{Bridge, BridgeError};
use crate::protocol::*;
use crate::query;

const EDIT_API: &str = r#"Call connect_project with an absolute project path before using this API.
Shrimply MCP edits operate on the connected editor's live in-memory project.
All public times are zero-based integer frames. Clip and track mutations require full concrete
addresses. Direct edits create one undoable history action. run_edit_script validates its ordered,
typed operations against a clone and installs them atomically as one history action. File imports
copy into media/imported/<uuid> by default; set link=true explicitly to retain external paths.
Imports without targets use an existing compatible track with room and do not create tracks.
Use create_track explicitly, or collision=new_track to allow insertion to create one as a fallback.
Undo removes imported clips while retaining their durable project-media files so redo remains valid.
Caption text is the text field of set_clip_properties. Query expressions to obtain their stable IDs,
then use set_expression with the owning clip address and expression ID. Expression edits can also be
included in run_edit_script for one atomic, undoable history action.
insert_captions bulk-inserts exact frame ranges into an existing caption track, or creates a new
track when track is omitted. It can set the track language and copy styling from source captions.
get_track returns one fully addressed track and up to 500 timeline-ordered clips in one call.

Example direct move:
{"address":{"kind":"video","sequence_path":[],"track_id":"…","item_id":"…"},"start_frame":120}

Example script:
{"frame":120,"operations":[{"type":"move_clip","args":{"address":{"kind":"video","sequence_path":[],"track_id":"…","item_id":"…"},"offset_frames":24,"collision":"reject"}}]}"#;

#[derive(Clone, Default)]
pub struct ShrimplyServer {
    bridge: Arc<RwLock<Option<Bridge>>>,
}

impl ShrimplyServer {
    pub fn new() -> Self {
        Self::default()
    }

    fn connected_bridge(&self) -> Result<Bridge, McpError> {
        self.bridge
            .read()
            .expect("Shrimply MCP project connection lock was poisoned")
            .clone()
            .ok_or_else(|| mcp_error("no project is connected; call connect_project first"))
    }

    async fn request(
        &self,
        command: BridgeCommand,
        context: &RequestContext<RoleServer>,
    ) -> Result<serde_json::Value, McpError> {
        let bridge = self.connected_bridge()?;
        let canceled = Arc::new(AtomicBool::new(false));
        let worker_canceled = canceled.clone();
        let mut worker = tokio::task::spawn_blocking(move || {
            bridge.request_with_cancel(command, worker_canceled)
        });
        tokio::select! {
            result = &mut worker => result
                .map_err(|error| internal_error(format!("editor bridge task failed: {error}")))?
                .map_err(bridge_error),
            () = context.ct.cancelled() => {
                canceled.store(true, Ordering::Release);
                worker.await
                    .map_err(|error| internal_error(format!("editor bridge task failed: {error}")))?
                    .map_err(bridge_error)
            }
        }
    }

    async fn snapshot(
        &self,
        context: &RequestContext<RoleServer>,
    ) -> Result<LiveSnapshot, McpError> {
        let value = self.request(BridgeCommand::Snapshot, context).await?;
        serde_json::from_value(value).map_err(|error| {
            internal_error(format!("editor returned an invalid snapshot: {error}"))
        })
    }

    async fn edit(
        &self,
        request: EditRequest,
        context: &RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        let value = self.request(BridgeCommand::Apply(request), context).await?;
        serde_json::from_value(value).map(Json).map_err(|error| {
            internal_error(format!("editor returned an invalid edit result: {error}"))
        })
    }
}

#[tool_router]
impl ShrimplyServer {
    #[tool(
        description = "Connect this MCP session to the open Shrimply project at an absolute path. Calling it again switches projects"
    )]
    async fn connect_project(
        &self,
        Parameters(request): Parameters<ConnectProjectRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<ConnectProjectResponse>, McpError> {
        let project_path = PathBuf::from(request.project_path);
        if !project_path.is_absolute() {
            return Err(mcp_error("project_path must be an absolute path"));
        }

        let canceled = Arc::new(AtomicBool::new(false));
        let worker_canceled = canceled.clone();
        let mut worker = tokio::task::spawn_blocking(move || {
            Bridge::connect_with_cancel(&project_path, worker_canceled)
        });
        let bridge = tokio::select! {
            result = &mut worker => result
                .map_err(|error| internal_error(format!("editor bridge task failed: {error}")))?
                .map_err(bridge_error)?,
            () = context.ct.cancelled() => {
                canceled.store(true, Ordering::Release);
                worker.await
                    .map_err(|error| internal_error(format!("editor bridge task failed: {error}")))?
                    .map_err(bridge_error)?
            }
        };
        let project_path = bridge
            .project_path()
            .to_str()
            .expect("project path was validated when the bridge connected")
            .to_string();
        *self
            .bridge
            .write()
            .expect("Shrimply MCP project connection lock was poisoned") = Some(bridge);
        Ok(Json(ConnectProjectResponse { project_path }))
    }

    #[tool(
        description = "Return live project, playhead, selection, active scope, and track state",
        annotations(read_only_hint = true)
    )]
    async fn get_editor_state(
        &self,
        Parameters(_): Parameters<GetEditorStateRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditorState>, McpError> {
        query::editor_state(&self.snapshot(&context).await?)
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(
        description = "List root and concrete folded-sequence presentation scopes with tracks",
        annotations(read_only_hint = true)
    )]
    async fn list_scopes(
        &self,
        Parameters(_): Parameters<ListScopesRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<ListScopesResponse>, McpError> {
        query::list_scopes(&self.snapshot(&context).await?)
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(
        description = "Query live clip presentations. The optional half-open frame range is stateless and independent from editor selection",
        annotations(read_only_hint = true)
    )]
    async fn query_clips(
        &self,
        Parameters(request): Parameters<QueryClipsRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<QueryClipsResponse>, McpError> {
        query::query_clips(&self.snapshot(&context).await?, &request)
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(
        description = "Get full live metadata for one concrete clip address or every presentation of an item UUID",
        annotations(read_only_hint = true)
    )]
    async fn get_clip(
        &self,
        Parameters(request): Parameters<GetClipRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<ClipMetadata>, McpError> {
        query::get_clip(
            &self.snapshot(&context).await?,
            request.address.as_ref(),
            request.item_id.as_deref(),
        )
        .map(Json)
        .map_err(mcp_error)
    }

    #[tool(
        description = "Return one fully addressed track and up to 500 of its clips in projected timeline order",
        annotations(read_only_hint = true)
    )]
    async fn get_track(
        &self,
        Parameters(request): Parameters<GetTrackRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<TrackMetadata>, McpError> {
        query::get_track(&self.snapshot(&context).await?, &request)
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(
        description = "Query expression IDs, property paths, enabled state, and source across live clip metadata",
        annotations(read_only_hint = true)
    )]
    async fn query_expressions(
        &self,
        Parameters(request): Parameters<QueryExpressionsRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<QueryExpressionsResponse>, McpError> {
        query::query_expressions(&self.snapshot(&context).await?, &request)
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(description = "Seek the visible editor playhead to a project frame")]
    async fn seek_playhead(
        &self,
        Parameters(request): Parameters<SeekPlayheadRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<FrameTime>, McpError> {
        let value = self
            .request(
                BridgeCommand::Seek {
                    frame: request.frame,
                },
                &context,
            )
            .await?;
        serde_json::from_value(value).map(Json).map_err(|error| {
            internal_error(format!("editor returned an invalid seek result: {error}"))
        })
    }

    #[tool(
        description = "Render a project frame with Shrimply's native compositor and return it as a PNG without changing the playhead",
        annotations(read_only_hint = true)
    )]
    async fn view_frame(
        &self,
        Parameters(request): Parameters<ViewFrameRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let value = self
            .request(
                BridgeCommand::ViewFrame {
                    frame: request.frame,
                },
                &context,
            )
            .await?;
        let response: ViewFrameResponse = serde_json::from_value(value).map_err(|error| {
            internal_error(format!(
                "editor returned an invalid rendered frame: {error}"
            ))
        })?;
        let metadata = serde_json::to_string(&response.frame)
            .map_err(|error| internal_error(format!("could not encode frame metadata: {error}")))?;
        Ok(CallToolResult::success(vec![
            ContentBlock::text(metadata),
            ContentBlock::image(response.png, "image/png"),
        ]))
    }

    #[tool(description = "Create one caption, video, or audio track as an explicit undoable edit")]
    async fn create_track(
        &self,
        Parameters(request): Parameters<CreateTrackRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            EditRequest {
                history_label: "MCP create track".to_string(),
                frame: None,
                scope: request.scope,
                operations: vec![EditOperation::CreateTrack(CreateTrackOperation {
                    kind: request.kind,
                    enabled: request.enabled,
                })],
            },
            &context,
        )
        .await
    }

    #[tool(
        description = "Import one or more native files atomically. Omitted targets choose an existing compatible track with room and never create one unless collision=new_track is explicit. Copying into project media is the preferred default; link=true retains external paths"
    )]
    async fn insert_files(
        &self,
        Parameters(request): Parameters<InsertFilesRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            EditRequest {
                history_label: "MCP insert files".to_string(),
                frame: request.frame,
                scope: request.scope.clone(),
                operations: vec![EditOperation::InsertFiles(request)],
            },
            &context,
        )
        .await
    }

    #[tool(
        description = "Bulk-insert captions into an existing root caption track or create a new one, with exact frame ranges, optional language, collision handling, and source style copying"
    )]
    async fn insert_captions(
        &self,
        Parameters(request): Parameters<InsertCaptionsRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single(
                "MCP insert captions",
                EditOperation::InsertCaptions(request),
            ),
            &context,
        )
        .await
    }

    #[tool(
        description = "Move a fully addressed clip to a projected frame and optional compatible track/scope"
    )]
    async fn move_clip(
        &self,
        Parameters(request): Parameters<MoveClipRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single("MCP move clip", EditOperation::MoveClip(request)),
            &context,
        )
        .await
    }

    #[tool(
        description = "Trim a fully addressed clip using projected frame bounds while preserving source offset"
    )]
    async fn trim_clip(
        &self,
        Parameters(request): Parameters<TrimClipRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single("MCP trim clip", EditOperation::TrimClip(request)),
            &context,
        )
        .await
    }

    #[tool(
        description = "Delete fully addressed clips as one undoable history action",
        annotations(destructive_hint = true)
    )]
    async fn delete_clips(
        &self,
        Parameters(request): Parameters<DeleteClipsRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single("MCP delete clips", EditOperation::DeleteClips(request)),
            &context,
        )
        .await
    }

    #[tool(
        description = "Set typed clip properties: caption text, audio enabled/gain, or video/audio playback speed/repeat strategy"
    )]
    async fn set_clip_properties(
        &self,
        Parameters(request): Parameters<SetClipPropertiesRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single(
                "MCP set clip properties",
                EditOperation::SetClipProperties(request),
            ),
            &context,
        )
        .await
    }

    #[tool(
        description = "Set an expression's source and/or enabled state by stable ID on its owning video or audio clip"
    )]
    async fn set_expression(
        &self,
        Parameters(request): Parameters<SetExpressionRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single("MCP set expression", EditOperation::SetExpression(request)),
            &context,
        )
        .await
    }

    #[tool(description = "Enable or disable a fully addressed caption, visual, or audio track")]
    async fn set_track_enabled(
        &self,
        Parameters(request): Parameters<SetTrackEnabledRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single(
                "MCP set track enabled",
                EditOperation::SetTrackEnabled(request),
            ),
            &context,
        )
        .await
    }

    #[tool(
        description = "Set or clear a caption track's CLDR locale identifier, such as en_US, en_GB, or ja_JP"
    )]
    async fn set_caption_track_language(
        &self,
        Parameters(request): Parameters<SetCaptionTrackLanguageRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single(
                "MCP set caption track language",
                EditOperation::SetCaptionTrackLanguage(request),
            ),
            &context,
        )
        .await
    }

    #[tool(
        description = "Delete a fully addressed caption, visual, or audio track and all of its clips",
        annotations(destructive_hint = true)
    )]
    async fn delete_track(
        &self,
        Parameters(request): Parameters<DeleteTrackRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            single("MCP delete track", EditOperation::DeleteTrack(request)),
            &context,
        )
        .await
    }

    #[tool(
        description = "Run an ordered typed edit program atomically as one MCP edit script history action"
    )]
    async fn run_edit_script(
        &self,
        Parameters(request): Parameters<RunEditScriptRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditResponse>, McpError> {
        self.edit(
            EditRequest {
                history_label: "MCP edit script".to_string(),
                frame: request.frame,
                scope: request.scope,
                operations: request.operations,
            },
            &context,
        )
        .await
    }
}

#[tool_handler]
impl ServerHandler for ShrimplyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "Call connect_project with an absolute project path first. Tools and resources then read and edit that editor's live in-memory project; connect_project can switch the session to another open project".to_string(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("shrimply://editor/state", "editor-state")
                .with_description("Live editor/project/player/selection state")
                .with_mime_type("application/json"),
            Resource::new("shrimply://project/clips", "project-clips")
                .with_description("All current root and nested clip presentations")
                .with_mime_type("application/json"),
            Resource::new("shrimply://edit-api", "edit-api")
                .with_description("Typed direct edit and edit-script usage")
                .with_mime_type("text/plain"),
        ]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            ResourceTemplate::new("shrimply://project/clips/{item_id}", "project-clip")
                .with_description("Full metadata plus every concrete presentation of an item UUID")
                .with_mime_type("application/json"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let value = match request.uri.as_str() {
            "shrimply://editor/state" => serde_json::to_value(
                query::editor_state(&self.snapshot(&context).await?).map_err(mcp_error)?,
            ),
            "shrimply://project/clips" => serde_json::to_value(
                query::all_clips(&self.snapshot(&context).await?).map_err(mcp_error)?,
            ),
            "shrimply://edit-api" => {
                return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                    EDIT_API,
                    request.uri,
                )])
                .into());
            }
            uri if uri.starts_with("shrimply://project/clips/") => {
                let item_id = uri.trim_start_matches("shrimply://project/clips/");
                serde_json::to_value(
                    query::get_clip(&self.snapshot(&context).await?, None, Some(item_id))
                        .map_err(mcp_error)?,
                )
            }
            _ => {
                return Err(McpError::resource_not_found(
                    "Shrimply resource was not found",
                    Some(json!({ "uri": request.uri })),
                ));
            }
        }
        .map_err(|error| internal_error(format!("could not encode resource: {error}")))?;
        let text = serde_json::to_string_pretty(&value)
            .map_err(|error| internal_error(format!("could not encode resource: {error}")))?;
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, request.uri).with_mime_type("application/json"),
        ])
        .into())
    }
}

fn single(label: &str, operation: EditOperation) -> EditRequest {
    EditRequest {
        history_label: label.to_string(),
        frame: None,
        scope: None,
        operations: vec![operation],
    }
}

fn mcp_error(error: impl ToString) -> McpError {
    McpError::invalid_params(error.to_string(), None)
}

fn internal_error(error: impl ToString) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

fn bridge_error(error: BridgeError) -> McpError {
    match error {
        BridgeError::Rejected(error) => mcp_error(error),
        BridgeError::Transport(error) => internal_error(error),
    }
}
