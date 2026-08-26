use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverbModifier {
    #[serde(default = "classic_mode")]
    pub mode: ReverbMode,
    pub room_size: TimelineValue<f32>,
    pub decay_seconds: TimelineValue<f32>,
    pub damping: TimelineValue<f32>,
    pub pre_delay_ms: TimelineValue<f32>,
    pub mix: TimelineValue<f32>,
    #[serde(default = "default_distance")]
    pub distance_m: TimelineValue<f32>,
}

impl Default for ReverbModifier {
    fn default() -> Self {
        Self {
            mode: ReverbMode::RoomCapture,
            room_size: TimelineValue::new_const(0.5),
            decay_seconds: TimelineValue::new_const(1.5),
            damping: TimelineValue::new_const(0.5),
            pre_delay_ms: TimelineValue::new_const(20.0),
            mix: TimelineValue::new_const(0.25),
            distance_m: default_distance(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReverbMode {
    #[default]
    Classic,
    RoomCapture,
}

impl ModifierModel for ReverbModifier {
    fn display_name(&self) -> &'static str {
        "Reverb"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["room", "ambience", "space", "recording", "distance"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.room_size, seen);
        ensure_timeline_value_ids(&mut self.decay_seconds, seen);
        ensure_timeline_value_ids(&mut self.damping, seen);
        ensure_timeline_value_ids(&mut self.pre_delay_ms, seen);
        ensure_timeline_value_ids(&mut self.mix, seen);
        ensure_timeline_value_ids(&mut self.distance_m, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.room_size),
            timeline_value_span(&self.decay_seconds),
            timeline_value_span(&self.damping),
            timeline_value_span(&self.pre_delay_ms),
            timeline_value_span(&self.mix),
            timeline_value_span(&self.distance_m),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [
            &self.room_size,
            &self.decay_seconds,
            &self.damping,
            &self.pre_delay_ms,
            &self.mix,
            &self.distance_m,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.room_size,
            &mut self.decay_seconds,
            &mut self.damping,
            &mut self.pre_delay_ms,
            &mut self.mix,
            &mut self.distance_m,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }
}

fn classic_mode() -> ReverbMode {
    ReverbMode::Classic
}

fn default_distance() -> TimelineValue<f32> {
    TimelineValue::new_const(1.5)
}
