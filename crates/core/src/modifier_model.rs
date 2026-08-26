use crate::timeline_value::*;
use crate::{Color, Time, VideoSampleMethod};
use glam::{Vec2, Vec3};
use hashbrown::HashSet;
use uuid::Uuid;

pub type KeyframeSpan = Option<(Time, Time)>;

pub trait ModifierModel {
    fn display_name(&self) -> &'static str;
    fn keywords(&self) -> &'static [&'static str] {
        &[]
    }
    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>);
    fn keyframe_span(&self) -> KeyframeSpan;
    fn number(&self, _id: Uuid) -> Option<&TimelineValue<f32>> {
        None
    }
    fn number_mut(&mut self, _id: Uuid) -> Option<&mut TimelineValue<f32>> {
        None
    }
    fn number2(&self, _id: Uuid) -> Option<&TimelineValue<glam::Vec2>> {
        None
    }
    fn number2_mut(&mut self, _id: Uuid) -> Option<&mut TimelineValue<glam::Vec2>> {
        None
    }
    fn number3(&self, _id: Uuid) -> Option<&TimelineValue<glam::Vec3>> {
        None
    }
    fn number3_mut(&mut self, _id: Uuid) -> Option<&mut TimelineValue<glam::Vec3>> {
        None
    }
    fn color_mut(&mut self, _id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        None
    }
    fn text(&self, _id: Uuid) -> Option<&TimelineValue<String>> {
        None
    }
    fn text_mut(&mut self, _id: Uuid) -> Option<&mut TimelineValue<String>> {
        None
    }
    fn integer(&self, _id: Uuid) -> Option<&TimelineValue<u32>> {
        None
    }
    fn integer_mut(&mut self, _id: Uuid) -> Option<&mut TimelineValue<u32>> {
        None
    }
    fn sample_method(&self, _id: Uuid) -> Option<&TimelineValue<VideoSampleMethod>> {
        None
    }
    fn sample_method_mut(&mut self, _id: Uuid) -> Option<&mut TimelineValue<VideoSampleMethod>> {
        None
    }
}

impl<T: ModifierModel + ?Sized> ModifierModel for Box<T> {
    fn display_name(&self) -> &'static str {
        (**self).display_name()
    }

    fn keywords(&self) -> &'static [&'static str] {
        (**self).keywords()
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        (**self).ensure_ids(seen)
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        (**self).keyframe_span()
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (**self).number(id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (**self).number_mut(id)
    }

    fn number2(&self, id: Uuid) -> Option<&TimelineValue<Vec2>> {
        (**self).number2(id)
    }

    fn number2_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Vec2>> {
        (**self).number2_mut(id)
    }

    fn number3(&self, id: Uuid) -> Option<&TimelineValue<Vec3>> {
        (**self).number3(id)
    }

    fn number3_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Vec3>> {
        (**self).number3_mut(id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        (**self).color_mut(id)
    }

    fn text(&self, id: Uuid) -> Option<&TimelineValue<String>> {
        (**self).text(id)
    }

    fn text_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<String>> {
        (**self).text_mut(id)
    }

    fn integer(&self, id: Uuid) -> Option<&TimelineValue<u32>> {
        (**self).integer(id)
    }

    fn integer_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<u32>> {
        (**self).integer_mut(id)
    }

    fn sample_method(&self, id: Uuid) -> Option<&TimelineValue<VideoSampleMethod>> {
        (**self).sample_method(id)
    }

    fn sample_method_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<VideoSampleMethod>> {
        (**self).sample_method_mut(id)
    }
}

pub trait TimelineValueModel<T: TimelineValueType> {
    fn timeline_value(&self, id: Uuid) -> Option<&TimelineValue<T>>;
    fn timeline_value_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<T>>;
}

macro_rules! model_access {
    ($ty:ty, $get:ident, $get_mut:ident) => {
        impl<M: ModifierModel> TimelineValueModel<$ty> for M {
            fn timeline_value(&self, id: Uuid) -> Option<&TimelineValue<$ty>> {
                self.$get(id)
            }

            fn timeline_value_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<$ty>> {
                self.$get_mut(id)
            }
        }
    };
}

model_access!(f32, number, number_mut);
model_access!(Vec2, number2, number2_mut);
model_access!(Vec3, number3, number3_mut);
model_access!(u32, integer, integer_mut);
model_access!(VideoSampleMethod, sample_method, sample_method_mut);
model_access!(String, text, text_mut);

impl<M: ModifierModel> TimelineValueModel<Color<u8>> for M {
    fn timeline_value(&self, _id: Uuid) -> Option<&TimelineValue<Color<u8>>> {
        None
    }

    fn timeline_value_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        self.color_mut(id)
    }
}

pub fn ensure_unique_id(id: &mut Uuid, seen: &mut HashSet<Uuid>) {
    while id.is_nil() || !seen.insert(*id) {
        *id = Uuid::new_v4();
    }
}
pub fn ensure_timeline_value_ids<T: TimelineValueType>(
    value: &mut TimelineValue<T>,
    seen: &mut HashSet<Uuid>,
) {
    ensure_unique_id(&mut value.id, seen);
    if let Some(value) = &mut value.expression {
        ensure_unique_id(&mut value.id, seen);
    }
    if let TimelineBase::Keyframes(values) = &mut value.base {
        for value in values {
            ensure_unique_id(value.id_mut(), seen);
        }
    }
}

pub fn timeline_value_span<T: TimelineValueType>(value: &TimelineValue<T>) -> KeyframeSpan {
    match &value.base {
        TimelineBase::Const(_) => None,
        TimelineBase::Keyframes(values) => endpoint_span(values.iter().map(TimelineKeyframe::time)),
    }
}
pub fn combine(spans: impl IntoIterator<Item = KeyframeSpan>) -> KeyframeSpan {
    endpoint_span(
        spans
            .into_iter()
            .flatten()
            .flat_map(|(start, end)| [start, end]),
    )
}
fn endpoint_span(times: impl IntoIterator<Item = Time>) -> KeyframeSpan {
    let mut times = times.into_iter();
    let first = times.next()?;
    Some(times.fold((first, first), |(start, end), time| {
        (start.min(time), end.max(time))
    }))
}
