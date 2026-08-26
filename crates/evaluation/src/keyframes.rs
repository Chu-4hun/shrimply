use glam::Vec2;
use shrimply_core::timeline_value::{
    TimelineScalarKeyframe, TimelineValueType, TimelineVectorKeyframe,
};
use shrimply_project::project::{Color, Time};

pub fn scalar_keyframes_value(keyframes: &[TimelineScalarKeyframe<f32>], time: Time) -> f32 {
    <f32 as TimelineValueType>::value_at(keyframes, time)
}

pub fn vec2_keyframes_value(keyframes: &[TimelineVectorKeyframe<Vec2>], time: Time) -> Vec2 {
    <Vec2 as TimelineValueType>::value_at(keyframes, time)
}

pub fn color_keyframes_value(
    keyframes: &[TimelineVectorKeyframe<Color<u8>>],
    time: Time,
) -> Color<u8> {
    <Color<u8> as TimelineValueType>::value_at(keyframes, time)
}
