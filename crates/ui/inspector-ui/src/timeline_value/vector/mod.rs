use crate::keyframe_editor::{KeyframeGraph, SpeedSegment};
use shrimply_core::timeline_value::{
    TimelineBase, TimelineValue, TimelineVector, TimelineVectorKeyframe,
};

pub(crate) mod vec2;
pub(crate) mod vec3;

pub(crate) fn speed_graph<T: TimelineVector>(value: &TimelineValue<T>) -> KeyframeGraph {
    let TimelineBase::Keyframes(keyframes) = &value.base else {
        return KeyframeGraph::Speed {
            segments: Vec::new(),
            keys: Vec::new(),
            static_value: 0.0,
        };
    };
    KeyframeGraph::Speed {
        segments: speed_segments(keyframes),
        keys: keyframes.iter().map(|keyframe| keyframe.time).collect(),
        static_value: 0.0,
    }
}

fn speed_segments<T: TimelineVector>(keyframes: &[TimelineVectorKeyframe<T>]) -> Vec<SpeedSegment> {
    keyframes
        .windows(2)
        .filter_map(|pair| {
            let start = pair[0].time;
            let end = pair[1].time;
            let seconds = end.signed_sub(start).as_secs_f64();
            (seconds > f64::EPSILON).then(|| SpeedSegment {
                owner_id: pair[0].id,
                start,
                end,
                value: T::distance(&pair[0].value, &pair[1].value) / seconds,
                interpolation: pair[0].interpolation_to_next,
            })
        })
        .collect()
}
