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

const DEFAULT_ROTATION_DEGREES: glam::Vec3 = glam::Vec3::new(-35.0, -45.0, 0.0);
const DEFAULT_INTENSITY: f32 = 1.0;
const DEFAULT_ANGULAR_RADIUS_DEGREES: f32 = 0.266;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SunLightModifier {
    pub rotation_degrees: TimelineValue<glam::Vec3>,
    pub color: TimelineValue<Color<u8>>,
    pub intensity: TimelineValue<f32>,
    pub angular_radius_degrees: TimelineValue<f32>,
}

impl Default for SunLightModifier {
    fn default() -> Self {
        Self {
            rotation_degrees: TimelineValue::new_const(DEFAULT_ROTATION_DEGREES),
            color: TimelineValue::new_const(Color::<u8>::WHITE),
            intensity: TimelineValue::new_const(DEFAULT_INTENSITY),
            angular_radius_degrees: TimelineValue::new_const(DEFAULT_ANGULAR_RADIUS_DEGREES),
        }
    }
}

impl ModifierModel for SunLightModifier {
    fn display_name(&self) -> &'static str {
        "Sun"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["directional light", "daylight", "sunlight"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.rotation_degrees, seen);
        ensure_timeline_value_ids(&mut self.color, seen);
        for value in [&mut self.intensity, &mut self.angular_radius_degrees] {
            ensure_timeline_value_ids(value, seen);
        }
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.rotation_degrees),
            timeline_value_span(&self.color),
            timeline_value_span(&self.intensity),
            timeline_value_span(&self.angular_radius_degrees),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.intensity, &self.angular_radius_degrees]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [&mut self.intensity, &mut self.angular_radius_degrees]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number3(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec3>> {
        (self.rotation_degrees.id == id).then_some(&self.rotation_degrees)
    }

    fn number3_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec3>> {
        (self.rotation_degrees.id == id).then_some(&mut self.rotation_degrees)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        (self.color.id == id).then_some(&mut self.color)
    }
}
