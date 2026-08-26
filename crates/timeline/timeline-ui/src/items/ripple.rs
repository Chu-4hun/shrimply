use super::*;
use crate::project::{ItemAddress, ItemMut, TrackAddress, TrackMut};
use crate::timeline_operation::TimelineOperationContext;

pub(crate) struct AddressRippleResult {
    pub(crate) selection: Vec<ItemAddress>,
    pub(crate) shifted_position: Time,
    pub(crate) captions: bool,
    pub(crate) video: bool,
    pub(crate) audio: bool,
}

pub(crate) fn ripple_delete_item_addresses(
    context: &impl TimelineOperationContext,
    project: &mut Project,
    selected: &[ItemAddress],
    position: Time,
) -> Option<AddressRippleResult> {
    let selected = expand_grouped_item_addresses(context, project, selected);
    let ranges = selected
        .iter()
        .filter_map(|address| context.timeline_item_times(project, address))
        .collect::<Vec<_>>();
    let intervals = ripple_intervals_from_ranges(&ranges);
    if intervals.is_empty() {
        return None;
    }

    let mut captions = false;
    let mut video = false;
    let mut audio = false;
    for track in context.tracks(project) {
        let local_intervals = local_intervals(project, &track, &intervals)?;
        let selected_ids = selected
            .iter()
            .filter(|item| item.track() == track)
            .map(ItemAddress::item_id)
            .collect::<HashSet<_>>();
        let selected_indices = selected_indices(project, &track, &selected_ids)?;
        let changed = match project.track_mut(&track)? {
            TrackMut::Caption(track) => {
                let shift = ripple_track_shift_limit(
                    &track.items,
                    &selected_indices,
                    &local_intervals,
                    ripple_total_shift_nanos(&local_intervals),
                );
                ripple_track_items(
                    &mut track.items,
                    &selected_indices,
                    &local_intervals,
                    shift,
                    |item, start, end| {
                        item.start = start;
                        item.end = end;
                    },
                )
            }
            TrackMut::Video(track) => {
                let shift = ripple_track_shift_limit(
                    &track.items,
                    &selected_indices,
                    &local_intervals,
                    ripple_total_shift_nanos(&local_intervals),
                );
                ripple_track_items(
                    &mut track.items,
                    &selected_indices,
                    &local_intervals,
                    shift,
                    |item, start, end| {
                        item.start = start;
                        item.end = end;
                    },
                )
            }
            TrackMut::Audio(track) => {
                let shift = ripple_track_shift_limit(
                    &track.items,
                    &selected_indices,
                    &local_intervals,
                    ripple_total_shift_nanos(&local_intervals),
                );
                ripple_track_items(
                    &mut track.items,
                    &selected_indices,
                    &local_intervals,
                    shift,
                    |item, start, end| {
                        item.start = start;
                        item.end = end;
                    },
                )
            }
        };
        match track.kind() {
            ItemKind::Caption => captions |= changed,
            ItemKind::Video => video |= changed,
            ItemKind::Audio => audio |= changed,
        }
    }

    let shift = ripple_position_shift_nanos(&intervals, position);
    Some(AddressRippleResult {
        selection: Vec::new(),
        shifted_position: Time::from_nanos(position.as_nonnegative_nanos().saturating_sub(shift)),
        captions,
        video,
        audio,
    })
}

pub(crate) fn ripple_trim_item_addresses(
    context: &impl TimelineOperationContext,
    project: &mut Project,
    selected: &[ItemAddress],
    cut: Time,
) -> Option<AddressRippleResult> {
    let selected = expand_grouped_item_addresses(context, project, selected)
        .into_iter()
        .filter(|address| {
            context
                .timeline_item_times(project, address)
                .is_some_and(|(start, end)| start < cut && cut < end)
        })
        .collect::<Vec<_>>();
    let ranges = selected
        .iter()
        .filter_map(|address| {
            let (start, _) = context.timeline_item_times(project, address)?;
            Some((start, cut))
        })
        .collect::<Vec<_>>();
    let intervals = ripple_intervals_from_ranges(&ranges);
    if intervals.is_empty() {
        return None;
    }

    let mut captions = false;
    let mut video = false;
    let mut audio = false;
    for address in &selected {
        let local_cut = project.timeline_time_to_sequence(&address.track(), cut)?;
        match project.item_mut(address)? {
            ItemMut::Caption(item) => {
                item.trim_start(local_cut);
                captions = true;
            }
            ItemMut::Video(item) => {
                item.trim_start(local_cut);
                fit_visual_transitions(item);
                video = true;
            }
            ItemMut::Audio(item) => {
                item.trim_start(local_cut);
                fit_audio_transitions(item);
                audio = true;
            }
        }
    }

    for track in context.tracks(project) {
        let local_intervals = local_intervals(project, &track, &intervals)?;
        let selected_indices = HashSet::new();
        let shift = ripple_total_shift_nanos(&local_intervals);
        let changed = match project.track_mut(&track)? {
            TrackMut::Caption(track) => ripple_track_items(
                &mut track.items,
                &selected_indices,
                &local_intervals,
                shift,
                |item, start, end| {
                    item.start = start;
                    item.end = end;
                },
            ),
            TrackMut::Video(track) => ripple_track_items(
                &mut track.items,
                &selected_indices,
                &local_intervals,
                shift,
                |item, start, end| {
                    item.start = start;
                    item.end = end;
                },
            ),
            TrackMut::Audio(track) => ripple_track_items(
                &mut track.items,
                &selected_indices,
                &local_intervals,
                shift,
                |item, start, end| {
                    item.start = start;
                    item.end = end;
                },
            ),
        };
        match track.kind() {
            ItemKind::Caption => captions |= changed,
            ItemKind::Video => video |= changed,
            ItemKind::Audio => audio |= changed,
        }
    }

    Some(AddressRippleResult {
        selection: selected,
        shifted_position: Time::from_nanos(
            cut.as_nonnegative_nanos()
                .saturating_sub(ripple_total_shift_nanos(&intervals)),
        ),
        captions,
        video,
        audio,
    })
}

fn local_intervals(
    project: &Project,
    track: &TrackAddress,
    intervals: &[RippleInterval],
) -> Option<Vec<RippleInterval>> {
    let ranges = intervals
        .iter()
        .map(|interval| {
            let first = project.timeline_time_to_sequence(track, interval.start)?;
            let second = project.timeline_time_to_sequence(track, interval.end)?;
            Some((first.min(second), first.max(second)))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ripple_intervals_from_ranges(&ranges))
}

fn selected_indices(
    project: &Project,
    track: &TrackAddress,
    selected_ids: &HashSet<uuid::Uuid>,
) -> Option<HashSet<usize>> {
    Some(match project.track(track)? {
        crate::project::TrackRef::Caption(track) => track
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| selected_ids.contains(&item.id).then_some(index))
            .collect(),
        crate::project::TrackRef::Video(track) => track
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| selected_ids.contains(&item.id).then_some(index))
            .collect(),
        crate::project::TrackRef::Audio(track) => track
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| selected_ids.contains(&item.id).then_some(index))
            .collect(),
    })
}

pub(crate) fn ripple_remove_time_ranges(
    project: &mut Project,
    ranges: &[(Time, Time)],
) -> Option<(bool, bool, bool)> {
    let intervals = ripple_intervals_from_ranges(ranges);
    if intervals.is_empty() {
        return None;
    }
    let mut next_group = next_group_id(project);
    let mut group_map = HashMap::new();

    Some((
        remove_ranges_from_caption_tracks(project, &intervals, &mut next_group, &mut group_map),
        remove_ranges_from_video_tracks(project, &intervals, &mut next_group, &mut group_map),
        remove_ranges_from_audio_tracks(project, &intervals, &mut next_group, &mut group_map),
    ))
}

pub(crate) fn delete_track_gap(
    project: &mut Project,
    gap: shrimply_timeline::TrackGap,
) -> Option<(bool, bool, bool)> {
    if track_gap_at(project, gap.track, gap.start) != Some(gap) {
        return None;
    }

    let intervals = ripple_intervals_from_ranges(&[(gap.start, gap.end)]);
    let shift = gap
        .end
        .as_nonnegative_nanos()
        .saturating_sub(gap.start.as_nonnegative_nanos());
    let selected = HashSet::new();
    let changed = match gap.track.kind {
        TrackKind::Caption => ripple_track_items(
            &mut project.caption_tracks.get_mut(gap.track.track_index)?.items,
            &selected,
            &intervals,
            shift,
            |item, start, end| {
                item.start = start;
                item.end = end;
            },
        ),
        TrackKind::Video => ripple_track_items(
            &mut project.video_tracks.get_mut(gap.track.track_index)?.items,
            &selected,
            &intervals,
            shift,
            |item, start, end| {
                item.start = start;
                item.end = end;
            },
        ),
        TrackKind::Audio => ripple_track_items(
            &mut project.audio_tracks.get_mut(gap.track.track_index)?.items,
            &selected,
            &intervals,
            shift,
            |item, start, end| {
                item.start = start;
                item.end = end;
            },
        ),
    };
    changed.then_some((
        gap.track.kind == TrackKind::Caption,
        gap.track.kind == TrackKind::Video,
        gap.track.kind == TrackKind::Audio,
    ))
}

#[derive(Clone, Copy)]
pub(crate) struct RippleInterval {
    start: Time,
    end: Time,
}

pub(crate) fn ripple_intervals_from_ranges(ranges: &[(Time, Time)]) -> Vec<RippleInterval> {
    let mut intervals: Vec<_> = ranges
        .iter()
        .filter(|(start, end)| end > start)
        .map(|(start, end)| RippleInterval {
            start: *start,
            end: *end,
        })
        .collect();
    intervals.sort_by_key(|interval| (interval.start, interval.end));

    let mut merged: Vec<RippleInterval> = Vec::new();
    for interval in intervals {
        if let Some(last) = merged.last_mut()
            && interval.start <= last.end
        {
            last.end = last.end.max(interval.end);
            continue;
        }
        merged.push(interval);
    }
    merged
}

pub(crate) fn remove_ranges_from_caption_tracks(
    project: &mut Project,
    intervals: &[RippleInterval],
    next_group_id: &mut u64,
    group_map: &mut HashMap<(u64, usize), u64>,
) -> bool {
    let mut changed = false;
    for track in &mut project.caption_tracks {
        changed |= remove_ranges_from_items(
            &mut track.items,
            intervals,
            next_group_id,
            group_map,
            |item, start, end| {
                item.start = start;
                item.end = end;
            },
        );
    }
    changed
}

pub(crate) fn remove_ranges_from_video_tracks(
    project: &mut Project,
    intervals: &[RippleInterval],
    next_group_id: &mut u64,
    group_map: &mut HashMap<(u64, usize), u64>,
) -> bool {
    let mut changed = false;
    for track in &mut project.video_tracks {
        changed |= remove_ranges_from_items(
            &mut track.items,
            intervals,
            next_group_id,
            group_map,
            |item, start, end| {
                item.start = start;
                item.end = end;
            },
        );
    }
    changed
}

pub(crate) fn remove_ranges_from_audio_tracks(
    project: &mut Project,
    intervals: &[RippleInterval],
    next_group_id: &mut u64,
    group_map: &mut HashMap<(u64, usize), u64>,
) -> bool {
    let mut changed = false;
    for track in &mut project.audio_tracks {
        changed |= remove_ranges_from_items(
            &mut track.items,
            intervals,
            next_group_id,
            group_map,
            |item, start, end| {
                item.start = start;
                item.end = end;
            },
        );
    }
    changed
}

pub(crate) fn remove_ranges_from_items<T: OverwriteItem>(
    items: &mut Vec<T>,
    intervals: &[RippleInterval],
    next_group_id: &mut u64,
    group_map: &mut HashMap<(u64, usize), u64>,
    set_times: impl Fn(&mut T, Time, Time),
) -> bool {
    let original = items.clone();
    for interval in intervals {
        overwrite_items(items, interval.start, interval.end);
    }

    let trimmed = std::mem::take(items);
    for mut item in trimmed {
        if let Some(group_id) = item.group_id() {
            let segment = kept_segment_index(intervals, item.start());
            let group_id = *group_map.entry((group_id, segment)).or_insert_with(|| {
                let group_id = *next_group_id;
                *next_group_id = next_group_id.saturating_add(1);
                group_id
            });
            item.set_group_id(Some(group_id));
        }
        let shift = ripple_position_shift_nanos(intervals, item.start());
        if shift > 0 {
            let start = Time::from_nanos(item.start().as_nonnegative_nanos().saturating_sub(shift));
            let end = Time::from_nanos(item.end().as_nonnegative_nanos().saturating_sub(shift));
            set_times(&mut item, start, end);
        }
        insert_sorted(items, item);
    }

    items.len() != original.len()
        || items.iter().zip(original.iter()).any(|(left, right)| {
            left.start() != right.start()
                || left.end() != right.end()
                || left.group_id() != right.group_id()
        })
}

pub(crate) fn kept_segment_index(intervals: &[RippleInterval], time: Time) -> usize {
    intervals
        .iter()
        .take_while(|interval| interval.end <= time)
        .count()
}

pub(crate) fn ripple_total_shift_nanos(intervals: &[RippleInterval]) -> u64 {
    intervals.iter().fold(0_u64, |shift, interval| {
        shift.saturating_add(
            interval
                .end
                .as_nonnegative_nanos()
                .saturating_sub(interval.start.as_nonnegative_nanos()),
        )
    })
}

pub(crate) fn ripple_track_shift_limit<T: TimeSlice>(
    items: &[T],
    selected_indices: &HashSet<usize>,
    intervals: &[RippleInterval],
    max_shift: u64,
) -> u64 {
    let mut shift = max_shift;
    let mut ranges = ripple_track_ranges(items, selected_indices);
    ranges.sort_by_key(|range| (range.0, range.1));
    let mut available_gap = 0_u64;
    let mut previous_end = None;

    for (start, end) in ranges {
        available_gap = available_gap.saturating_add(previous_end.map_or_else(
            || start.as_nonnegative_nanos(),
            |previous_end: Time| {
                start
                    .as_nonnegative_nanos()
                    .saturating_sub(previous_end.as_nonnegative_nanos())
            },
        ));

        let item_shift = ripple_position_shift_nanos(intervals, start).min(max_shift);
        if item_shift == 0 {
            available_gap = 0;
        } else if item_shift > available_gap {
            shift = shift.min(available_gap);
        }
        previous_end = Some(end);
    }

    shift
}

pub(crate) fn ripple_track_ranges<T: TimeSlice>(
    items: &[T],
    selected_indices: &HashSet<usize>,
) -> Vec<(Time, Time)> {
    let mut ranges = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if selected_indices.contains(&index) {
            continue;
        }
        ranges.push((item.start(), item.end()));
    }
    ranges
}

pub(crate) fn ripple_track_items<T: TimeSlice>(
    items: &mut Vec<T>,
    selected_indices: &HashSet<usize>,
    intervals: &[RippleInterval],
    shift_limit: u64,
    set_times: impl Fn(&mut T, Time, Time),
) -> bool {
    let original = std::mem::take(items);
    let mut changed = !selected_indices.is_empty();
    let remaining: Vec<_> = original
        .iter()
        .enumerate()
        .filter(|(index, _)| !selected_indices.contains(index))
        .map(|(_, item)| {
            (
                item.start(),
                item.end(),
                ripple_position_shift_nanos(intervals, item.start()),
            )
        })
        .collect();
    let mut shifts: Vec<_> = remaining
        .iter()
        .map(|(_, _, shift)| (*shift).min(shift_limit))
        .collect();

    for index in (1..remaining.len()).rev() {
        let gap = remaining[index]
            .0
            .as_nonnegative_nanos()
            .saturating_sub(remaining[index - 1].1.as_nonnegative_nanos());
        let required_shift = shifts[index].saturating_sub(gap);
        if required_shift > shifts[index - 1] {
            assert!(
                remaining[index - 1].2 > 0,
                "ripple shift crossed its anchor"
            );
            shifts[index - 1] = required_shift;
        }
    }

    let mut remaining_index = 0;
    for (index, mut item) in original.into_iter().enumerate() {
        if selected_indices.contains(&index) {
            continue;
        }

        let original_start = item.start();
        let original_end = item.end();
        let duration = item.duration_nanos();
        let shift = shifts[remaining_index];
        remaining_index += 1;
        if shift > 0 {
            let target_start =
                Time::from_nanos(original_start.as_nonnegative_nanos().saturating_sub(shift));
            let target_end =
                Time::from_nanos(target_start.as_nonnegative_nanos().saturating_add(duration));
            if target_start != original_start || target_end != original_end {
                set_times(&mut item, target_start, target_end);
                changed = true;
            }
        }

        insert_sorted(items, item);
    }

    changed
}

pub(crate) fn ripple_position_shift_nanos(intervals: &[RippleInterval], position: Time) -> u64 {
    let mut shift = 0_u64;
    let position = position.as_nonnegative_nanos();

    for interval in intervals {
        let start = interval.start.as_nonnegative_nanos();
        let end = interval.end.as_nonnegative_nanos();
        if position <= start {
            break;
        }
        shift = shift.saturating_add(position.min(end).saturating_sub(start));
        if position < end {
            break;
        }
    }

    shift
}
