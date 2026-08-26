use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelMixerModifier {
    pub rr: TimelineValue<f32>,
    pub rg: TimelineValue<f32>,
    pub rb: TimelineValue<f32>,
    pub gr: TimelineValue<f32>,
    pub gg: TimelineValue<f32>,
    pub gb: TimelineValue<f32>,
    pub br: TimelineValue<f32>,
    pub bg: TimelineValue<f32>,
    pub bb: TimelineValue<f32>,
}

impl Default for ChannelMixerModifier {
    fn default() -> Self {
        Self {
            rr: TimelineValue::<f32>::new_const(1.0),
            rg: TimelineValue::<f32>::new_const(0.0),
            rb: TimelineValue::<f32>::new_const(0.0),
            gr: TimelineValue::<f32>::new_const(0.0),
            gg: TimelineValue::<f32>::new_const(1.0),
            gb: TimelineValue::<f32>::new_const(0.0),
            br: TimelineValue::<f32>::new_const(0.0),
            bg: TimelineValue::<f32>::new_const(0.0),
            bb: TimelineValue::<f32>::new_const(1.0),
        }
    }
}

impl ModifierModel for ChannelMixerModifier {
    fn display_name(&self) -> &'static str {
        "Channel mixer"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["RGB", "color channels", "colour channels"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.rr, seen);
        ensure_timeline_value_ids(&mut self.rg, seen);
        ensure_timeline_value_ids(&mut self.rb, seen);
        ensure_timeline_value_ids(&mut self.gr, seen);
        ensure_timeline_value_ids(&mut self.gg, seen);
        ensure_timeline_value_ids(&mut self.gb, seen);
        ensure_timeline_value_ids(&mut self.br, seen);
        ensure_timeline_value_ids(&mut self.bg, seen);
        ensure_timeline_value_ids(&mut self.bb, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.rr),
            timeline_value_span(&self.rg),
            timeline_value_span(&self.rb),
            timeline_value_span(&self.gr),
            timeline_value_span(&self.gg),
            timeline_value_span(&self.gb),
            timeline_value_span(&self.br),
            timeline_value_span(&self.bg),
            timeline_value_span(&self.bb),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [
            &self.rr, &self.rg, &self.rb, &self.gr, &self.gg, &self.gb, &self.br, &self.bg,
            &self.bb,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.rr,
            &mut self.rg,
            &mut self.rb,
            &mut self.gr,
            &mut self.gg,
            &mut self.gb,
            &mut self.br,
            &mut self.bg,
            &mut self.bb,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }
}
