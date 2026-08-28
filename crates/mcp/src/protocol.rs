use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shrimply_math_core::Time;
use shrimply_project::project::Project;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExactFraction {
    pub numerator: i64,
    pub denominator: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FrameTime {
    pub frame: u64,
    pub seconds: ExactFraction,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FrameRange {
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeRef {
    /// Concrete folded-sequence presenter item IDs. Empty means the root scope.
    #[serde(default)]
    pub sequence_path: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Caption,
    Video,
    Audio,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TrackAddress {
    pub kind: ClipKind,
    #[serde(default)]
    pub sequence_path: Vec<String>,
    pub track_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClipAddress {
    pub kind: ClipKind,
    #[serde(default)]
    pub sequence_path: Vec<String>,
    pub track_id: String,
    pub item_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub position: Time,
    pub duration: Time,
    pub playing: bool,
    pub revision: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActiveScopeSnapshot {
    pub instance_path: Vec<String>,
    pub video_paths: Vec<ScopeRef>,
    pub audio_paths: Vec<ScopeRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveSnapshot {
    pub project_path: String,
    pub project: Project,
    pub player: PlayerSnapshot,
    pub active_scope: ActiveScopeSnapshot,
    pub focused_item: Option<ClipAddress>,
    pub selected_items: Vec<ClipAddress>,
    pub focused_track: Option<TrackAddress>,
    pub selected_tracks: Vec<TrackAddress>,
    pub asset_revisions: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub project_path: String,
    pub command: BridgeCommand,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "command", content = "payload")]
pub enum BridgeCommand {
    Handshake,
    Snapshot,
    Seek { frame: u64 },
    ViewFrame { frame: u64 },
    Apply(EditRequest),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub project_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RangeMatch {
    #[default]
    Overlaps,
    Contained,
    StartsIn,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConnectProjectRequest {
    /// Absolute path to an open Shrimply project file.
    pub project_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConnectProjectResponse {
    pub project_path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct GetEditorStateRequest {}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListScopesRequest {}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct QueryClipsRequest {
    /// Defaults to the editor's active scope.
    pub scope: Option<ScopeRef>,
    #[serde(default)]
    pub recursive: bool,
    pub kind: Option<ClipKind>,
    pub source_kind: Option<String>,
    pub track_id: Option<String>,
    pub item_id: Option<String>,
    pub enabled: Option<bool>,
    pub caption_text: Option<String>,
    pub source_filename: Option<String>,
    /// Independent, stateless half-open frame range selector.
    pub range: Option<FrameRange>,
    #[serde(default)]
    pub range_match: RangeMatch,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetClipRequest {
    pub address: Option<ClipAddress>,
    pub item_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct QueryExpressionsRequest {
    pub address: Option<ClipAddress>,
    pub source_contains: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SeekPlayheadRequest {
    pub frame: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ViewFrameRequest {
    pub frame: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewFrameResponse {
    pub frame: FrameTime,
    pub png: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CollisionBehavior {
    #[default]
    Reject,
    NewTrack,
    Overwrite,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct InitialClipProperties {
    pub text: Option<String>,
    pub enabled: Option<bool>,
    pub gain_db: Option<f32>,
    pub playback_speed: Option<ExactFraction>,
    pub repeat_strategy: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImportEntry {
    pub source: String,
    #[serde(default)]
    pub offset_frames: i64,
    /// Optional explicit compatible tracks. When omitted, an existing track with room is chosen.
    #[serde(default)]
    pub targets: Vec<TrackAddress>,
    #[serde(default)]
    pub properties: InitialClipProperties,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsertFilesRequest {
    pub files: Vec<ImportEntry>,
    pub frame: Option<u64>,
    pub scope: Option<ScopeRef>,
    #[serde(default)]
    pub link: bool,
    pub copy_root: Option<String>,
    /// new_track reuses a compatible track with room before creating one.
    #[serde(default)]
    pub collision: CollisionBehavior,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CaptionCueInput {
    pub start_frame: u64,
    pub end_frame: u64,
    pub text: String,
    /// Optional caption whose styling and layout should be copied.
    pub copy_style_from: Option<ClipAddress>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsertCaptionsRequest {
    /// Existing root caption track. Omit to create a new caption track.
    pub track: Option<TrackAddress>,
    pub captions: Vec<CaptionCueInput>,
    /// CLDR locale identifier such as en_US, zh_CN, or ja_JP.
    pub language: Option<String>,
    /// Sets the resolved track state. New tracks default to enabled.
    pub enabled: Option<bool>,
    #[serde(default)]
    pub collision: CollisionBehavior,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MoveClipRequest {
    pub address: ClipAddress,
    /// Absolute projected frame. Provide exactly one of this and offset_frames.
    pub start_frame: Option<u64>,
    /// Signed offset from the edit-script anchor frame.
    pub offset_frames: Option<i64>,
    pub destination: Option<TrackAddress>,
    #[serde(default)]
    pub collision: CollisionBehavior,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TrimClipRequest {
    pub address: ClipAddress,
    pub start_frame: Option<u64>,
    pub end_frame: Option<u64>,
    /// Signed replacement start relative to the edit-script anchor.
    pub start_offset_frames: Option<i64>,
    /// Signed replacement end relative to the edit-script anchor.
    pub end_offset_frames: Option<i64>,
    #[serde(default)]
    pub collision: CollisionBehavior,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteClipsRequest {
    pub addresses: Vec<ClipAddress>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetClipPropertiesRequest {
    pub address: ClipAddress,
    pub text: Option<String>,
    pub enabled: Option<bool>,
    pub gain_db: Option<f32>,
    pub playback_speed: Option<ExactFraction>,
    pub repeat_strategy: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetExpressionRequest {
    pub address: ClipAddress,
    pub expression_id: String,
    pub source: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetTrackEnabledRequest {
    pub address: TrackAddress,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetCaptionTrackLanguageRequest {
    pub address: TrackAddress,
    /// CLDR locale identifier such as en_US, en_GB, or ja_JP. Null clears it.
    pub language: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteTrackRequest {
    pub address: TrackAddress,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateTrackRequest {
    pub kind: ClipKind,
    pub scope: Option<ScopeRef>,
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateTrackOperation {
    pub kind: ClipKind,
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "args")]
pub enum EditOperation {
    InsertFiles(InsertFilesRequest),
    InsertCaptions(InsertCaptionsRequest),
    CreateTrack(CreateTrackOperation),
    MoveClip(MoveClipRequest),
    TrimClip(TrimClipRequest),
    DeleteClips(DeleteClipsRequest),
    SetClipProperties(SetClipPropertiesRequest),
    SetExpression(SetExpressionRequest),
    SetTrackEnabled(SetTrackEnabledRequest),
    SetCaptionTrackLanguage(SetCaptionTrackLanguageRequest),
    DeleteTrack(DeleteTrackRequest),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunEditScriptRequest {
    pub frame: Option<u64>,
    pub scope: Option<ScopeRef>,
    pub operations: Vec<EditOperation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditRequest {
    pub history_label: String,
    pub frame: Option<u64>,
    pub scope: Option<ScopeRef>,
    pub operations: Vec<EditOperation>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TimeSpan {
    pub start: FrameTime,
    pub end: FrameTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TrackSummary {
    pub address: TrackAddress,
    pub enabled: bool,
    /// Set only for caption tracks.
    pub language: Option<String>,
    pub clip_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClipSummary {
    pub address: ClipAddress,
    pub label: String,
    pub source_kind: String,
    pub asset_path: Option<String>,
    pub enabled: bool,
    pub local: TimeSpan,
    pub projected: TimeSpan,
    pub state: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetMetadata {
    pub path: String,
    pub canonical_path: Option<String>,
    pub exists: bool,
    pub size: Option<u64>,
    pub modified_unix_seconds: Option<u64>,
    pub asset_revision: Option<u64>,
    pub inside_project_media: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClipMetadata {
    pub metadata: Value,
    pub owning_track: TrackSummary,
    pub asset: Option<AssetMetadata>,
    pub presentations: Vec<ClipSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CanvasSummary {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditorState {
    pub project_path: String,
    pub project_name: String,
    pub fps: ExactFraction,
    pub canvas: CanvasSummary,
    pub duration: FrameTime,
    pub playhead: FrameTime,
    pub playing: bool,
    pub revision: u64,
    pub active_scope: ActiveScopeSummary,
    pub focused_item: Option<ClipAddress>,
    pub selected_items: Vec<ClipAddress>,
    pub focused_track: Option<TrackAddress>,
    pub selected_tracks: Vec<TrackAddress>,
    pub tracks: Vec<TrackSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ActiveScopeSummary {
    pub instance_path: Vec<String>,
    pub video_presentations: Vec<ScopeRef>,
    pub audio_presentations: Vec<ScopeRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScopeSummary {
    pub scope: ScopeRef,
    pub tracks: Vec<TrackSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListScopesResponse {
    pub scopes: Vec<ScopeSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryClipsResponse {
    pub clips: Vec<ClipSummary>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExpressionSummary {
    pub address: ClipAddress,
    pub expression_id: String,
    /// JSON Pointer into the clip metadata returned by get_clip.
    pub property_path: String,
    pub enabled: bool,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryExpressionsResponse {
    pub expressions: Vec<ExpressionSummary>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectClipsResource {
    pub clips: Vec<ClipSummary>,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditOperationResult {
    pub index: usize,
    pub operation: String,
    pub changed_addresses: Vec<ClipAddress>,
    pub deleted_addresses: Vec<ClipAddress>,
    pub changed_tracks: Vec<TrackAddress>,
    pub presentations: Vec<ClipSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditResponse {
    pub operations: Vec<EditOperationResult>,
    pub duration: FrameTime,
    pub revision: u64,
}
use std::collections::BTreeMap;
