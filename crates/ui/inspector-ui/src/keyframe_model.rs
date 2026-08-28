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
    pub(crate) times: Vec<Time>,
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
        times: copied.iter().map(TimelineKeyframe::time).collect(),
        values: Rc::new(copied),
    })
}

pub(crate) fn paste_keyframes<T: TimelineValueType>(
    value: &mut TimelineValue<T>,
    clipboard: &KeyframeClipboard,
    times: &[Time],
) -> Option<Vec<Time>> {
    let source = clipboard.values.downcast_ref::<Vec<T::Keyframe>>()?;
    if source.len() != times.len() {
        return None;
    }
    let TimelineBase::Keyframes(keyframes) = &mut value.base else {
        return None;
    };
    let mut pasted = source.clone();
    for (keyframe, time) in pasted.iter_mut().zip(times) {
        *keyframe.id_mut() = Uuid::new_v4();
        *keyframe.time_mut() = *time;
    }
    let paste_start = *times.iter().min()?;
    let paste_end = *times.iter().max()?;
    if paste_start == paste_end {
        keyframes.retain(|keyframe| keyframe.time() != paste_start);
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

pub(crate) fn key_at(times: &[Time], playhead: Time, _frame_step: Time) -> Option<Time> {
    times.iter().copied().find(|time| time.approx_eq(playhead))
}

pub(crate) fn previous_key(times: &[Time], playhead: Time, _frame_step: Time) -> Option<Time> {
    times
        .iter()
        .copied()
        .rev()
        .find(|time| *time < playhead && !time.approx_eq(playhead))
}

pub(crate) fn next_key(times: &[Time], playhead: Time, _frame_step: Time) -> Option<Time> {
    times
        .iter()
        .copied()
        .find(|time| *time > playhead && !time.approx_eq(playhead))
}

pub(crate) fn same_frame(left: Time, right: Time, _frame_step: Time) -> bool {
    left.approx_eq(right)
}
