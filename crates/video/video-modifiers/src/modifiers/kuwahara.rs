use super::{KeyframeSpan, ModifierModel, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KuwaharaVersion {
    #[default]
    Classic,
    Generalized,
}

shrimply_core::timeline_value::timeline_step_type!(
    KuwaharaVersion,
    KuwaharaVersion::Classic,
    &[
        TimelineStepVariant {
            value: KuwaharaVersion::Classic,
            key: "classic",
            label: "Classic",
            icon: None
        },
        TimelineStepVariant {
            value: KuwaharaVersion::Generalized,
            key: "generalized",
            label: "Generalized",
            icon: None
        },
    ]
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KuwaharaModifier {
    #[serde(deserialize_with = "deserialize_timeline_value")]
    pub version: TimelineValue<KuwaharaVersion>,
    pub radius: TimelineValue<f32>,
}

impl Default for KuwaharaModifier {
    fn default() -> Self {
        Self {
            version: TimelineValue::new_const(KuwaharaVersion::Classic),
            radius: TimelineValue::<f32>::new_const(4.0),
        }
    }
}

impl ModifierModel for KuwaharaModifier {
    fn display_name(&self) -> &'static str {
        "Kuwahara"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["painterly", "oil paint", "painting", "stylize"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.version, seen);
        ensure_timeline_value_ids(&mut self.radius, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        super::combine([
            timeline_value_span(&self.version),
            timeline_value_span(&self.radius),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        (self.radius.id == id).then_some(&self.radius)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        (self.radius.id == id).then_some(&mut self.radius)
    }
}
