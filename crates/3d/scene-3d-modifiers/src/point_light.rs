use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    Color,
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use uuid::Uuid;

const DEFAULT_POSITION: glam::Vec3 = glam::Vec3::splat(2.0);
const DEFAULT_INTENSITY: f32 = 25.0;
const DEFAULT_RANGE: f32 = 10.0;
const DEFAULT_RADIUS: f32 = 0.25;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PointLightModifier {
    pub position: TimelineValue<glam::Vec3>,
    pub color: TimelineValue<Color<u8>>,
    pub intensity: TimelineValue<f32>,
    pub range: TimelineValue<f32>,
    pub radius: TimelineValue<f32>,
}

impl Default for PointLightModifier {
    fn default() -> Self {
        Self {
            position: TimelineValue::new_const(DEFAULT_POSITION),
            color: TimelineValue::new_const(Color::<u8>::WHITE),
            intensity: TimelineValue::new_const(DEFAULT_INTENSITY),
            range: TimelineValue::new_const(DEFAULT_RANGE),
            radius: TimelineValue::new_const(DEFAULT_RADIUS),
        }
    }
}

impl ModifierModel for PointLightModifier {
    fn display_name(&self) -> &'static str {
        "Point Light"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["lamp", "bulb", "omnidirectional"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.position, seen);
        ensure_timeline_value_ids(&mut self.color, seen);
        for value in [&mut self.intensity, &mut self.range, &mut self.radius] {
            ensure_timeline_value_ids(value, seen);
        }
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.position),
            timeline_value_span(&self.color),
            timeline_value_span(&self.intensity),
            timeline_value_span(&self.range),
            timeline_value_span(&self.radius),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.intensity, &self.range, &self.radius]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.intensity, &mut self.range, &mut self.radius]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number3(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec3>> {
        (self.position.id == id).then_some(&self.position)
    }

    fn number3_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec3>> {
        (self.position.id == id).then_some(&mut self.position)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        (self.color.id == id).then_some(&mut self.color)
    }
}
