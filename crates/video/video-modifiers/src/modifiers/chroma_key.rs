use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{Color, timeline_value::TimelineValue};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChromaKeyModifier {
    pub key_color: TimelineValue<shrimply_core::Color<u8>>,
    pub similarity: TimelineValue<f32>,
    pub softness: TimelineValue<f32>,
    pub spill_suppression: TimelineValue<f32>,
}

impl Default for ChromaKeyModifier {
    fn default() -> Self {
        Self {
            key_color: TimelineValue::<Color<u8>>::new_const(Color::<u8>::from_rgb(0, 255, 0)),
            similarity: TimelineValue::<f32>::new_const(0.35),
            softness: TimelineValue::<f32>::new_const(0.1),
            spill_suppression: TimelineValue::<f32>::new_const(0.5),
        }
    }
}

impl ModifierModel for ChromaKeyModifier {
    fn display_name(&self) -> &'static str {
        "Chroma key"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["green screen", "blue screen", "keying"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.key_color, seen);
        for value in [
            &mut self.similarity,
            &mut self.softness,
            &mut self.spill_suppression,
        ] {
            ensure_timeline_value_ids(value, seen);
        }
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.key_color),
            timeline_value_span(&self.similarity),
            timeline_value_span(&self.softness),
            timeline_value_span(&self.spill_suppression),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [&self.similarity, &self.softness, &self.spill_suppression]
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.similarity,
            &mut self.softness,
            &mut self.spill_suppression,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        (self.key_color.id == id).then_some(&mut self.key_color)
    }
}
