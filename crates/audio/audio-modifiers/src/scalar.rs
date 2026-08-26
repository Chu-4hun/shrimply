use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use uuid::Uuid;

macro_rules! scalar_modifier {
    ($name:ident, $display_name:literal, [$($keyword:literal),* $(,)?] { $($field:ident: $default:expr),+ $(,)? }) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $name { $(pub $field: TimelineValue<f32>),+ }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: TimelineValue::<f32>::new_const($default)),+ }
            }
        }

        impl ModifierModel for $name {
            fn display_name(&self) -> &'static str {
                $display_name
            }

            fn keywords(&self) -> &'static [&'static str] {
                &[$($keyword),*]
            }

            fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
                $(ensure_timeline_value_ids(&mut self.$field, seen);)+
            }

            fn keyframe_span(&self) -> KeyframeSpan {
                combine([$(timeline_value_span(&self.$field)),+])
            }

            fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
                [$(&self.$field),+].into_iter().find(|value| value.id == id)
            }

            fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
                [$(&mut self.$field),+]
                    .into_iter()
                    .find(|value| value.id == id)
            }
        }
    };
}

scalar_modifier!(GainModifier, "Gain", ["volume", "level", "amplify"] { decibels: 0.0 });
scalar_modifier!(PanModifier, "Pan", ["balance", "left right", "stereo position"] { position: 0.0 });
scalar_modifier!(EqualizerModifier, "3-band EQ", ["equalizer", "equalization", "tone"] {
    low_db: 0.0,
    mid_db: 0.0,
    high_db: 0.0,
});
scalar_modifier!(NoiseGateModifier, "Noise gate", ["gate", "silence", "threshold"] {
    threshold_db: -40.0,
    attack_ms: 5.0,
    release_ms: 100.0,
});
scalar_modifier!(StereoWidthModifier, "Stereo width", ["stereo imaging", "widen", "mono"] { width: 1.0 });
scalar_modifier!(TremoloModifier, "Tremolo", ["amplitude modulation", "pulse"] {
    rate_hz: 5.0,
    depth: 0.5,
});
scalar_modifier!(BitcrusherModifier, "Bitcrusher", ["lofi", "lo-fi", "8-bit", "degrade"] {
    resolution_bits: 8.0,
    sample_rate_hz: 12_000.0,
    mix: 1.0,
});
scalar_modifier!(ChorusModifier, "Chorus", ["ensemble", "doubling", "widen"] {
    rate_hz: 0.8,
    depth_ms: 3.0,
    delay_ms: 15.0,
    mix: 0.5,
});
scalar_modifier!(CompressorModifier, "Compressor", ["compression", "dynamics", "leveling"] {
    threshold_db: -18.0,
    ratio: 2.0,
    attack_ms: 20.0,
    release_ms: 250.0,
    makeup_db: 0.0,
    mix: 1.0,
});
scalar_modifier!(LimiterModifier, "Limiter", ["peak", "clipping", "ceiling"] {
    ceiling_db: 0.0,
    release_ms: 50.0,
});
scalar_modifier!(DistortionModifier, "Distortion", ["overdrive", "saturation", "clipping"] {
    drive_db: 12.0,
    tone: 0.5,
    mix: 1.0,
});
