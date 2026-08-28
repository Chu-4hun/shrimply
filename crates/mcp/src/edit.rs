use std::collections::HashSet;

use shrimply_math_core::{Fraction, Time, fraction_new, time_from_frame};
use shrimply_project::project::{Project, RepeatStrategy, SequenceScopeId, Time as ProjectTime};
use shrimply_timeline::edit::{self, CollisionBehavior as ModelCollision};
use uuid::Uuid;

use crate::protocol::{
    ClipAddress, ClipSummary, CollisionBehavior, EditOperation, ExactFraction,
    SetClipPropertiesRequest, TrackAddress,
};
use crate::query::{model_item_address, model_kind, model_track_address};

#[derive(Default)]
pub struct MutationResult {
    pub changed_item_ids: Vec<Uuid>,
    pub deleted_addresses: Vec<ClipAddress>,
    pub deleted_presentations: Vec<ClipSummary>,
    pub changed_tracks: Vec<TrackAddress>,
}

pub fn apply_non_import(
    project: &mut Project,
    operation: &EditOperation,
    anchor: u64,
    scope: &SequenceScopeId,
) -> Result<MutationResult, String> {
    match operation {
        EditOperation::InsertFiles(_) => {
            Err("insert_files must be handled by the native importer".to_string())
        }
        EditOperation::CreateTrack(request) => {
            let id = edit::create_track(
                project,
                scope,
                model_kind(request.kind),
                request.enabled.unwrap_or(true),
            )?;
            Ok(MutationResult {
                changed_tracks: crate::query::addresses_for_tracks(project, &HashSet::from([id]))?,
                ..Default::default()
            })
        }
        EditOperation::MoveClip(request) => {
            let address = model_item_address(&request.address)?;
            let destination = request
                .destination
                .as_ref()
                .map(model_track_address)
                .transpose()?;
            let projected = frame(
                project,
                resolve_frame(
                    request.start_frame,
                    request.offset_frames,
                    anchor,
                    "move start",
                )?,
            )?;
            let path = destination
                .as_ref()
                .map(|track| track.sequence_path())
                .unwrap_or_else(|| address.sequence_path());
            let local = project
                .timeline_time_to_sequence_path(model_kind(request.address.kind), path, projected)
                .ok_or_else(|| "destination scope does not resolve in the project".to_string())?
                .snapped(project.frame_step());
            let destination = destination.unwrap_or_else(|| address.track());
            let duration = project
                .item(&address)
                .map(|item| {
                    let (start, end) = item.times();
                    end.saturating_sub(start)
                })
                .ok_or_else(|| "clip was not found".to_string())?;
            let deleted = overwritten_presentations(
                project,
                &destination,
                local,
                local.saturating_add(duration),
                address.item_id(),
                request.collision,
            )?;
            let result = edit::move_item(
                project,
                &address,
                Some(&destination),
                local,
                collision(request.collision),
            )?;
            changed_after_insert(project, &result, &destination, deleted)
        }
        EditOperation::TrimClip(request) => {
            let address = model_item_address(&request.address)?;
            let start = optional_frame(
                project,
                request.start_frame,
                request.start_offset_frames,
                anchor,
                "trim start",
            )?
            .map(|time| {
                project
                    .timeline_time_to_sequence_path(
                        model_kind(request.address.kind),
                        address.sequence_path(),
                        time,
                    )
                    .map(|time| time.snapped(project.frame_step()))
                    .ok_or_else(|| "clip scope does not resolve in the project".to_string())
            })
            .transpose()?;
            let end = optional_frame(
                project,
                request.end_frame,
                request.end_offset_frames,
                anchor,
                "trim end",
            )?
            .map(|time| {
                project
                    .timeline_time_to_sequence_path(
                        model_kind(request.address.kind),
                        address.sequence_path(),
                        time,
                    )
                    .map(|time| time.snapped(project.frame_step()))
                    .ok_or_else(|| "clip scope does not resolve in the project".to_string())
            })
            .transpose()?;
            if start.is_none() && end.is_none() {
                return Err("trim_clip requires a start or end frame".to_string());
            }
            let (old_start, old_end) = project
                .item(&address)
                .map(|item| item.times())
                .ok_or_else(|| "clip was not found".to_string())?;
            let deleted = overwritten_presentations(
                project,
                &address.track(),
                start.unwrap_or(old_start),
                end.unwrap_or(old_end),
                address.item_id(),
                request.collision,
            )?;
            let result =
                edit::trim_item(project, &address, start, end, collision(request.collision))?;
            changed_after_insert(project, &result, &address.track(), deleted)
        }
        EditOperation::DeleteClips(request) => {
            if request.addresses.is_empty() {
                return Err("delete_clips requires at least one address".to_string());
            }
            let mut addresses = request
                .addresses
                .iter()
                .map(model_item_address)
                .collect::<Result<Vec<_>, _>>()?;
            for address in &addresses {
                if project.item(address).is_none() {
                    return Err(format!("clip {} was not found", address.item_id()));
                }
            }
            addresses.sort_by_key(|address| std::cmp::Reverse(address.sequence_path().len()));
            let mut logical_items = std::collections::HashSet::new();
            addresses.retain(|address| {
                logical_items.insert((address.kind(), address.track_id(), address.item_id()))
            });
            let item_ids = addresses.iter().map(|address| address.item_id()).collect();
            let deleted_presentations =
                crate::query::presentations_affected_by_items(project, &item_ids)?;
            edit::delete_items(project, &addresses)?;
            Ok(MutationResult {
                deleted_addresses: deleted_presentations
                    .iter()
                    .map(|clip| clip.address.clone())
                    .collect(),
                deleted_presentations,
                ..Default::default()
            })
        }
        EditOperation::SetClipProperties(request) => set_properties(project, request),
        EditOperation::SetTrackEnabled(request) => {
            let address = model_track_address(&request.address)?;
            edit::set_track_enabled(project, &address, request.enabled)?;
            let track_ids = [address.track_id()].into_iter().collect();
            let presentations = crate::query::presentations_for_tracks(project, &track_ids)?;
            Ok(MutationResult {
                changed_item_ids: presentations
                    .iter()
                    .filter_map(|clip| Uuid::parse_str(&clip.address.item_id).ok())
                    .collect(),
                changed_tracks: crate::query::addresses_for_tracks(project, &track_ids)?,
                ..Default::default()
            })
        }
    }
}

fn set_properties(
    project: &mut Project,
    request: &SetClipPropertiesRequest,
) -> Result<MutationResult, String> {
    if request.text.is_none()
        && request.enabled.is_none()
        && request.gain_db.is_none()
        && request.playback_speed.is_none()
        && request.repeat_strategy.is_none()
    {
        return Err("set_clip_properties requires at least one property".to_string());
    }
    let address = model_item_address(&request.address)?;
    let has_playback = request.playback_speed.is_some() || request.repeat_strategy.is_some();
    edit::validate_properties_target(
        project,
        &address,
        request.text.is_some(),
        request.enabled.is_some(),
        request.gain_db.is_some(),
        has_playback,
    )?;
    if let Some(text) = &request.text {
        edit::set_caption_text(project, &address, text.clone())?;
    }
    if let Some(enabled) = request.enabled {
        edit::set_audio_enabled(project, &address, enabled)?;
    }
    if let Some(gain_db) = request.gain_db {
        edit::set_audio_gain(project, &address, gain_db)?;
    }
    let speed = request
        .playback_speed
        .as_ref()
        .map(positive_fraction)
        .transpose()?;
    let repeat = request
        .repeat_strategy
        .as_deref()
        .map(parse_repeat)
        .transpose()?;
    if has_playback {
        edit::set_playback(project, &address, speed, repeat)?;
    }
    Ok(changed(address.item_id()))
}

pub fn positive_fraction(value: &ExactFraction) -> Result<Fraction, String> {
    if value.numerator <= 0 || value.denominator <= 0 {
        return Err("playback_speed must be a positive exact fraction".to_string());
    }
    Ok(fraction_new(value.numerator, value.denominator))
}

fn parse_repeat(value: &str) -> Result<RepeatStrategy, String> {
    match value {
        "repeat" => Ok(RepeatStrategy::Repeat),
        "ping_pong" => Ok(RepeatStrategy::PingPong),
        "hold" => Ok(RepeatStrategy::Hold),
        "empty" => Ok(RepeatStrategy::Empty),
        _ => Err("repeat_strategy must be repeat, ping_pong, hold, or empty".to_string()),
    }
}

fn frame(project: &Project, value: u64) -> Result<Time, String> {
    time_from_frame(value, project.fps)
        .ok_or_else(|| "frame exceeds the supported exact fraction range".to_string())
}

fn optional_frame(
    project: &Project,
    absolute: Option<u64>,
    offset: Option<i64>,
    anchor: u64,
    name: &str,
) -> Result<Option<Time>, String> {
    match (absolute, offset) {
        (None, None) => Ok(None),
        _ => resolve_frame(absolute, offset, anchor, name)
            .and_then(|value| frame(project, value))
            .map(Some),
    }
}

fn resolve_frame(
    absolute: Option<u64>,
    offset: Option<i64>,
    anchor: u64,
    name: &str,
) -> Result<u64, String> {
    match (absolute, offset) {
        (Some(frame), None) => Ok(frame),
        (None, Some(offset)) => {
            frame_with_offset(anchor, offset).map_err(|error| format!("{name}: {error}"))
        }
        (Some(_), Some(_)) => Err(format!(
            "provide exactly one of {name}_frame and {name}_offset_frames"
        )),
        (None, None) => Err(format!("{name} frame is required")),
    }
}

pub fn frame_with_offset(anchor: u64, offset: i64) -> Result<u64, String> {
    if offset >= 0 {
        anchor
            .checked_add(offset as u64)
            .ok_or_else(|| "frame overflow".to_string())
    } else {
        anchor
            .checked_sub(offset.unsigned_abs())
            .ok_or_else(|| "offset places the frame before zero".to_string())
    }
}

fn changed(item_id: Uuid) -> MutationResult {
    MutationResult {
        changed_item_ids: vec![item_id],
        ..Default::default()
    }
}

fn changed_with_deleted(item_id: Uuid, deleted: Vec<ClipSummary>) -> MutationResult {
    MutationResult {
        changed_item_ids: vec![item_id],
        deleted_addresses: deleted.iter().map(|clip| clip.address.clone()).collect(),
        deleted_presentations: deleted,
        ..Default::default()
    }
}

fn changed_after_insert(
    project: &Project,
    result: &shrimply_project::project::ItemAddress,
    requested_track: &shrimply_project::project::TrackAddress,
    deleted: Vec<ClipSummary>,
) -> Result<MutationResult, String> {
    let mut mutation = changed_with_deleted(result.item_id(), deleted);
    if result.track_id() != requested_track.track_id() {
        mutation.changed_tracks =
            crate::query::addresses_for_tracks(project, &HashSet::from([result.track_id()]))?;
    }
    Ok(mutation)
}

fn overwritten_presentations(
    project: &Project,
    track: &shrimply_project::project::TrackAddress,
    start: ProjectTime,
    end: ProjectTime,
    source: Uuid,
    collision: CollisionBehavior,
) -> Result<Vec<ClipSummary>, String> {
    if collision != CollisionBehavior::Overwrite {
        return Ok(Vec::new());
    }
    let item_ids = edit::collision_addresses(project, track, start, end)?
        .into_iter()
        .map(|address| address.item_id())
        .filter(|item_id| *item_id != source)
        .collect();
    crate::query::presentations_affected_by_items(project, &item_ids)
}

fn collision(value: CollisionBehavior) -> ModelCollision {
    match value {
        CollisionBehavior::Reject => ModelCollision::Reject,
        CollisionBehavior::NewTrack => ModelCollision::NewTrack,
        CollisionBehavior::Overwrite => ModelCollision::Overwrite,
    }
}
