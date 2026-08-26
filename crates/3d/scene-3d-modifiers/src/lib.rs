mod ground;
mod object;
mod point_light;
mod sun_light;

use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    Color,
    modifier_model::{KeyframeSpan, ModifierModel},
    timeline_value::TimelineValue,
};
use uuid::Uuid;

pub use ground::{GroundKind, GroundModifier};
pub use object::Object3dModifier;
pub use point_light::PointLightModifier;
pub use shrimply_shape_3d::{Shape3dKind, Shape3dModifier, Shape3dRoundingStrategy};
pub use shrimply_text_3d::Text3dModifier;
pub use sun_light::SunLightModifier;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "config", rename_all = "snake_case")]
pub enum Scene3dModifierEffect {
    Object(Box<Object3dModifier>),
    Text(Box<Text3dModifier>),
    Shape(Box<Shape3dModifier>),
    Ground(Box<GroundModifier>),
    PointLight(Box<PointLightModifier>),
    SunLight(Box<SunLightModifier>),
}

impl Scene3dModifierEffect {
    pub const CATALOG: &[fn() -> Self] = &[
        || Self::Shape(Box::default()),
        || Self::Text(Box::default()),
        || Self::Object(Box::default()),
        || Self::Ground(Box::default()),
        || Self::PointLight(Box::default()),
        || Self::SunLight(Box::default()),
    ];

    fn model(&self) -> &dyn ModifierModel {
        match self {
            Self::Object(value) => value,
            Self::Text(value) => value,
            Self::Shape(value) => value,
            Self::Ground(value) => value,
            Self::PointLight(value) => value,
            Self::SunLight(value) => value,
        }
    }

    fn model_mut(&mut self) -> &mut dyn ModifierModel {
        match self {
            Self::Object(value) => value,
            Self::Text(value) => value,
            Self::Shape(value) => value,
            Self::Ground(value) => value,
            Self::PointLight(value) => value,
            Self::SunLight(value) => value,
        }
    }
}

impl ModifierModel for Scene3dModifierEffect {
    fn display_name(&self) -> &'static str {
        self.model().display_name()
    }

    fn keywords(&self) -> &'static [&'static str] {
        self.model().keywords()
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        self.model_mut().ensure_ids(seen)
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        self.model().keyframe_span()
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        self.model().number(id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        self.model_mut().number_mut(id)
    }

    fn number3(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec3>> {
        self.model().number3(id)
    }

    fn number3_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec3>> {
        self.model_mut().number3_mut(id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        self.model_mut().color_mut(id)
    }

    fn text(&self, id: Uuid) -> Option<&TimelineValue<String>> {
        self.model().text(id)
    }

    fn text_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<String>> {
        self.model_mut().text_mut(id)
    }
}
