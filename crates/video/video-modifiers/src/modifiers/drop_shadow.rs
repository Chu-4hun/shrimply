use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DropShadowModifier {
    pub offset: TimelineValue<glam::Vec2>,
    pub blur_radius: TimelineValue<f32>,
    pub color: TimelineValue<shrimply_core::Color<u8>>,
}

impl Default for DropShadowModifier {
    fn default() -> Self {
        Self {
            offset: TimelineValue::<glam::Vec2>::new_const(glam::Vec2::splat(10.0)),
            blur_radius: TimelineValue::<f32>::new_const(10.0),
            color: TimelineValue::<shrimply_core::Color<u8>>::new_const(
                shrimply_core::Color::<u8>::BLACK.with_alpha(128),
            ),
        }
    }
}

impl ModifierModel for DropShadowModifier {
    fn display_name(&self) -> &'static str {
        "Drop shadow"
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.offset, seen);
        ensure_timeline_value_ids(&mut self.blur_radius, seen);
        ensure_timeline_value_ids(&mut self.color, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.offset),
            timeline_value_span(&self.blur_radius),
            timeline_value_span(&self.color),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.blur_radius].into_iter().find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.blur_radius]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number2(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec2>> {
        [&self.offset].into_iter().find(|value| value.id == id)
    }

    fn number2_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec2>> {
        [&mut self.offset].into_iter().find(|value| value.id == id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        [&mut self.color].into_iter().find(|value| value.id == id)
    }
}
