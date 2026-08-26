use std::{any::Any, rc::Rc};

use crate::{
    player_state::ProjectChange,
    timeline_value::{TimelineBase, TimelineKeyframe, TimelineValue, TimelineValueType},
};
use shrimply_project::project::Time;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct KeyframeClipboard {
    values: Rc<dyn Any>,
    len: usize,
}

impl KeyframeClipboard {
    pub(crate) fn len(&self) -> usize {
        self.len
    }
}

pub(crate) fn copy_keyframes<T: TimelineValueType>(
    value: &TimelineValue<T>,
    selected: &[Time],
) -> Option<KeyframeClipboard> {
    let TimelineBase::Keyframes(keyframes) = &value.base else {
        return None;
    };
    let copied: Vec<T::Keyframe> = keyframes
        .iter()
        .filter(|keyframe| selected.contains(&keyframe.time()))
        .cloned()
        .collect();
    (!copied.is_empty()).then(|| KeyframeClipboard {
        len: copied.len(),
        values: Rc::new(copied),
    })
}

pub(crate) fn paste_keyframes<T: TimelineValueType>(
    value: &mut TimelineValue<T>,
    clipboard: &KeyframeClipboard,
    time: Time,
    frame_step: Time,
) -> Option<Vec<Time>> {
    let source = clipboard.values.downcast_ref::<Vec<T::Keyframe>>()?;
    let TimelineBase::Keyframes(keyframes) = &mut value.base else {
        return None;
    };
    let source_start = source.first()?.time();
    let paste_start = snap_to_frame(time, frame_step);
    let mut pasted = source.clone();
    for keyframe in &mut pasted {
        *keyframe.id_mut() = Uuid::new_v4();
        keyframe.time_mut().seconds =
            paste_start.seconds + keyframe.time().seconds - source_start.seconds;
    }
    let paste_end = pasted.last()?.time();
    if paste_start == paste_end {
        keyframes.retain(|keyframe| !same_frame(keyframe.time(), paste_start, frame_step));
    } else {
        keyframes.retain(|keyframe| keyframe.time() < paste_start || keyframe.time() > paste_end);
    }
    let pasted_times = pasted.iter().map(TimelineKeyframe::time).collect();
    keyframes.extend(pasted);
    keyframes.sort_by_key(TimelineKeyframe::time);
    Some(pasted_times)
}

pub(crate) fn live_refresh(mut refresh: ProjectChange) -> ProjectChange {
    refresh.inspector = false;
    refresh.live_preview = true;
    refresh
}

pub(crate) fn key_at(times: &[Time], playhead: Time, frame_step: Time) -> Option<Time> {
    times
        .iter()
        .copied()
        .find(|time| same_frame(*time, playhead, frame_step))
}

pub(crate) fn previous_key(times: &[Time], playhead: Time, frame_step: Time) -> Option<Time> {
    let step = frame_step.as_nonnegative_nanos();
    if step == 0 {
        return times.iter().copied().rev().find(|time| *time < playhead);
    }
    let playhead_frame = rounded_frame(playhead, step);
    times
        .iter()
        .copied()
        .rev()
        .find(|time| rounded_frame(*time, step) < playhead_frame)
}

pub(crate) fn next_key(times: &[Time], playhead: Time, frame_step: Time) -> Option<Time> {
    let step = frame_step.as_nonnegative_nanos();
    if step == 0 {
        return times.iter().copied().find(|time| *time > playhead);
    }
    let playhead_frame = rounded_frame(playhead, step);
    times
        .iter()
        .copied()
        .find(|time| rounded_frame(*time, step) > playhead_frame)
}

pub(crate) fn same_frame(left: Time, right: Time, frame_step: Time) -> bool {
    let step = frame_step.as_nonnegative_nanos();
    if step == 0 {
        return left.approx_eq(right);
    }
    rounded_frame(left, step) == rounded_frame(right, step)
}

pub(crate) fn snap_to_frame(time: Time, frame_step: Time) -> Time {
    let step = frame_step.as_nonnegative_nanos();
    if step == 0 {
        return time;
    }
    Time::from_nanos_i128(rounded_frame(time, step) * step as i128)
}

fn rounded_frame(time: Time, step: u64) -> i128 {
    let step = step as i128;
    let nanos = time.as_nanos_i128();
    if nanos >= 0 {
        (nanos + step / 2) / step
    } else {
        (nanos - step / 2) / step
    }
}
