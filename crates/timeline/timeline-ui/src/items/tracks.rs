use crate::project::{AudioSource, Project, SequenceReference, TrackAddress, VideoItemContent};
use shrimply_math_color::Color;
use shrimply_timeline::{TrackKey, TrackKind};
use uuid::Uuid;

use super::super::{RULER_HEIGHT, TRACK_HEIGHT};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TrackRow {
    pub(crate) address: TrackAddress,
    pub(crate) root_key: Option<TrackKey>,
    pub(crate) depth: usize,
}

pub(crate) fn rows(project: &Project) -> Vec<TrackRow> {
    let mut rows = project
        .caption_tracks
        .iter()
        .enumerate()
        .map(|(track_index, track)| {
            let root_key = TrackKey {
                kind: TrackKind::Caption,
                track_index,
            };
            TrackRow {
                address: TrackAddress::Caption { track_id: track.id },
                root_key: Some(root_key),
                depth: 0,
            }
        })
        .collect::<Vec<_>>();
    for (track_index, track) in project.video_tracks.iter().enumerate() {
        let root_key = TrackKey {
            kind: TrackKind::Video,
            track_index,
        };
        rows.push(TrackRow {
            address: TrackAddress::Video {
                sequence_path: Vec::new(),
                track_id: track.id,
            },
            root_key: Some(root_key),
            depth: 0,
        });
        for item in &track.items {
            let VideoItemContent::FoldedSequence(reference) = item.content else {
                continue;
            };
            let path = vec![item.id];
            if crate::folded_sequence::expanded(project, &path) {
                append_video_rows(project, reference, &path, 1, &mut rows, &mut Vec::new());
            }
        }
    }
    for (track_index, track) in project.audio_tracks.iter().enumerate() {
        let root_key = TrackKey {
            kind: TrackKind::Audio,
            track_index,
        };
        rows.push(TrackRow {
            address: TrackAddress::Audio {
                sequence_path: Vec::new(),
                track_id: track.id,
            },
            root_key: Some(root_key),
            depth: 0,
        });
        for item in &track.items {
            let AudioSource::FoldedSequence(reference) = item.source else {
                continue;
            };
            let path = vec![item.id];
            if crate::folded_sequence::expanded(project, &path) {
                append_audio_rows(project, reference, &path, 1, &mut rows, &mut Vec::new());
            }
        }
    }
    rows
}

fn append_video_rows(
    project: &Project,
    reference: SequenceReference,
    path: &[Uuid],
    depth: usize,
    rows: &mut Vec<TrackRow>,
    stack: &mut Vec<Uuid>,
) {
    if stack.contains(&reference.sequence_id) {
        return;
    }
    let Some(sequence) = project.folded_sequence(reference.sequence_id) else {
        return;
    };
    stack.push(reference.sequence_id);
    for track in &sequence.video_tracks {
        rows.push(TrackRow {
            address: TrackAddress::Video {
                sequence_path: path.to_vec(),
                track_id: track.id,
            },
            root_key: None,
            depth,
        });
        for item in &track.items {
            let VideoItemContent::FoldedSequence(reference) = item.content else {
                continue;
            };
            let mut nested_path = path.to_vec();
            nested_path.push(item.id);
            if crate::folded_sequence::expanded(project, &nested_path) {
                append_video_rows(project, reference, &nested_path, depth + 1, rows, stack);
            }
        }
    }
    stack.pop();
}

fn append_audio_rows(
    project: &Project,
    reference: SequenceReference,
    path: &[Uuid],
    depth: usize,
    rows: &mut Vec<TrackRow>,
    stack: &mut Vec<Uuid>,
) {
    if stack.contains(&reference.sequence_id) {
        return;
    }
    let Some(sequence) = project.folded_sequence(reference.sequence_id) else {
        return;
    };
    stack.push(reference.sequence_id);
    for track in &sequence.audio_tracks {
        rows.push(TrackRow {
            address: TrackAddress::Audio {
                sequence_path: path.to_vec(),
                track_id: track.id,
            },
            root_key: None,
            depth,
        });
        for item in &track.items {
            let AudioSource::FoldedSequence(reference) = item.source else {
                continue;
            };
            let mut nested_path = path.to_vec();
            nested_path.push(item.id);
            if crate::folded_sequence::expanded(project, &nested_path) {
                append_audio_rows(project, reference, &nested_path, depth + 1, rows, stack);
            }
        }
    }
    stack.pop();
}

pub(crate) fn color(kind: TrackKind) -> Color {
    match kind {
        TrackKind::Video => Color::ACCENT_BLUE,
        TrackKind::Caption => Color::ACCENT_YELLOW,
        TrackKind::Audio => Color::ACCENT_GREEN,
    }
}

pub(crate) fn track_at_y(project: &Project, y: f64) -> Option<(TrackKind, usize, usize)> {
    if y < RULER_HEIGHT {
        return None;
    }

    let row = ((y - RULER_HEIGHT) / TRACK_HEIGHT).floor() as usize;
    let key = rows(project).get(row)?.root_key?;
    Some((key.kind, key.track_index, row))
}

pub(crate) fn target_track_at_y(
    project: &Project,
    kind: TrackKind,
    y: f64,
) -> Option<(usize, Option<usize>)> {
    if y < RULER_HEIGHT {
        return None;
    }

    let row = ((y - RULER_HEIGHT) / TRACK_HEIGHT).floor() as usize;
    let count = track_count(project, kind);
    if let Some(track_index) =
        (0..count).find(|track_index| row_for_track(project, kind, *track_index) == Some(row))
    {
        return Some((track_index, None));
    }
    let start_row = row_for_track(project, kind, 0).unwrap_or_else(|| match kind {
        TrackKind::Caption => 0,
        TrackKind::Video => project.caption_tracks.len(),
        TrackKind::Audio => {
            project.caption_tracks.len()
                + project.video_tracks.len()
                + expanded_rows_before(project, TrackKind::Video, project.video_tracks.len())
        }
    });
    if row.checked_add(1) == Some(start_row) {
        return Some((0, Some(0)));
    }
    let end_row = match kind {
        TrackKind::Caption => project.caption_tracks.len(),
        TrackKind::Video => {
            project.caption_tracks.len()
                + project.video_tracks.len()
                + expanded_rows_before(project, TrackKind::Video, project.video_tracks.len())
        }
        TrackKind::Audio => {
            project.caption_tracks.len()
                + project.video_tracks.len()
                + expanded_rows_before(project, TrackKind::Video, project.video_tracks.len())
                + project.audio_tracks.len()
                + expanded_rows_before(project, TrackKind::Audio, project.audio_tracks.len())
        }
    };
    if row == end_row {
        return Some((count, Some(count)));
    }
    None
}

pub(crate) fn track_count(project: &Project, kind: TrackKind) -> usize {
    match kind {
        TrackKind::Caption => project.caption_tracks.len(),
        TrackKind::Video => project.video_tracks.len(),
        TrackKind::Audio => project.audio_tracks.len(),
    }
}

pub(super) fn active_new_track_at_y(
    project: &Project,
    kind: TrackKind,
    new_tracks: &[(TrackKind, usize)],
    y: f64,
) -> Option<(usize, Option<usize>)> {
    if y < RULER_HEIGHT {
        return None;
    }

    let row = ((y - RULER_HEIGHT) / TRACK_HEIGHT).floor() as usize;
    new_tracks
        .iter()
        .copied()
        .filter(|(track_kind, index)| *track_kind == kind && *index <= track_count(project, kind))
        .find_map(|(_, index)| {
            (row == base_row_for_track(project, kind, index)).then_some((index, Some(index)))
        })
}

fn base_row_for_track(project: &Project, kind: TrackKind, track_index: usize) -> usize {
    row_for_track(project, kind, track_index).unwrap_or(track_index)
}

pub(crate) fn row_for_track(
    project: &Project,
    kind: TrackKind,
    track_index: usize,
) -> Option<usize> {
    rows(project)
        .iter()
        .position(|row| row.root_key == Some(TrackKey { kind, track_index }))
}

pub(crate) fn row_for_address(project: &Project, address: &TrackAddress) -> Option<usize> {
    rows(project).iter().position(|row| &row.address == address)
}

pub(crate) fn expanded_rows_before(project: &Project, kind: TrackKind, end: usize) -> usize {
    crate::folded_sequence::child_tracks_before(project, kind, end)
}
