use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use uuid::Uuid;

const DEFAULT_SIZE: f32 = 2.0;
const DEFAULT_SHADOW_STRENGTH: f32 = 1.0;
const DEFAULT_REFLECTION: f32 = 0.0;
const DEFAULT_ROUGHNESS: f32 = 0.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundKind {
    #[default]
    Infinite,
    Square,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroundModifier {
    pub kind: GroundKind,
    pub size: TimelineValue<f32>,
    pub composite_enabled: bool,
    pub intensity: TimelineValue<f32>,
    pub position: TimelineValue<glam::Vec3>,
    pub rotation_degrees: TimelineValue<glam::Vec3>,
    pub opacity: TimelineValue<f32>,
    pub shadow_strength: TimelineValue<f32>,
    pub reflection: TimelineValue<f32>,
    pub roughness: TimelineValue<f32>,
}

impl Default for GroundModifier {
    fn default() -> Self {
        Self {
            kind: GroundKind::Infinite,
            size: TimelineValue::new_const(DEFAULT_SIZE),
            composite_enabled: true,
            intensity: TimelineValue::new_const(
                shrimply_scene_3d::DEFAULT_COMPOSED_PLANE_INTENSITY,
            ),
            position: TimelineValue::new_const(shrimply_scene_3d::DEFAULT_SHADOW_RECEIVER_POSITION),
            rotation_degrees: TimelineValue::new_const(glam::Vec3::ZERO),
            opacity: TimelineValue::new_const(shrimply_scene_3d::DEFAULT_SHADOW_RECEIVER_OPACITY),
            shadow_strength: TimelineValue::new_const(DEFAULT_SHADOW_STRENGTH),
            reflection: TimelineValue::new_const(DEFAULT_REFLECTION),
            roughness: TimelineValue::new_const(DEFAULT_ROUGHNESS),
        }
    }
}

impl ModifierModel for GroundModifier {
    fn display_name(&self) -> &'static str {
        "Ground"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["floor", "plane", "shadow catcher"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        for value in [
            &mut self.size,
            &mut self.intensity,
            &mut self.opacity,
            &mut self.shadow_strength,
            &mut self.reflection,
            &mut self.roughness,
        ] {
            ensure_timeline_value_ids(value, seen);
        }
        ensure_timeline_value_ids(&mut self.position, seen);
        ensure_timeline_value_ids(&mut self.rotation_degrees, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.size),
            timeline_value_span(&self.intensity),
            timeline_value_span(&self.position),
            timeline_value_span(&self.rotation_degrees),
            timeline_value_span(&self.opacity),
            timeline_value_span(&self.shadow_strength),
            timeline_value_span(&self.reflection),
            timeline_value_span(&self.roughness),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [
            &self.size,
            &self.intensity,
            &self.opacity,
            &self.shadow_strength,
            &self.reflection,
            &self.roughness,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.size,
            &mut self.intensity,
            &mut self.opacity,
            &mut self.shadow_strength,
            &mut self.reflection,
            &mut self.roughness,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn number3(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec3>> {
        [&self.position, &self.rotation_degrees]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number3_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec3>> {
        [&mut self.position, &mut self.rotation_degrees]
            .into_iter()
            .find(|value| value.id == id)
    }
}
