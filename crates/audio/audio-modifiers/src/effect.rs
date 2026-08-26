use hashbrown::HashSet;

use serde::{Deserialize, Serialize};
use shrimply_core::{
    modifier_model::{KeyframeSpan, ModifierModel},
    timeline_value::TimelineValue,
};
use uuid::Uuid;

use crate::{
    BitcrusherModifier, CacheModifier, ChorusModifier, CloseUpModifier, CompressorModifier,
    DenoiseModifier, DistortionModifier, EchoModifier, EqualizerModifier, FilterModifier,
    GainModifier, LimiterModifier, NoiseGateModifier, PanModifier, PitchModifier, ReverbModifier,
    StereoWidthModifier, TremoloModifier, VoiceChangeModifier, VoiceColorModifier,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "config", rename_all = "snake_case")]
pub enum AudioModifierEffect {
    Cache(CacheModifier),
    Gain(GainModifier),
    Pan(PanModifier),
    Pitch(Box<PitchModifier>),
    Denoise(Box<DenoiseModifier>),
    Equalizer(Box<EqualizerModifier>),
    Filter(Box<FilterModifier>),
    NoiseGate(Box<NoiseGateModifier>),
    StereoWidth(StereoWidthModifier),
    Tremolo(Box<TremoloModifier>),
    Bitcrusher(Box<BitcrusherModifier>),
    Chorus(Box<ChorusModifier>),
    Compressor(Box<CompressorModifier>),
    Limiter(Box<LimiterModifier>),
    Reverb(Box<ReverbModifier>),
    CloseUp(Box<CloseUpModifier>),
    VoiceColor(Box<VoiceColorModifier>),
    Echo(Box<EchoModifier>),
    Distortion(Box<DistortionModifier>),
    VoiceChange(Box<VoiceChangeModifier>),
}

impl AudioModifierEffect {
    pub const CATALOG: &[fn() -> Self] = &[
        || Self::Cache(Default::default()),
        || Self::Gain(Default::default()),
        || Self::Pan(Default::default()),
        || Self::Pitch(Default::default()),
        || Self::Denoise(Default::default()),
        || Self::Equalizer(Default::default()),
        || Self::Filter(Default::default()),
        || Self::NoiseGate(Default::default()),
        || Self::StereoWidth(Default::default()),
        || Self::Tremolo(Default::default()),
        || Self::Bitcrusher(Default::default()),
        || Self::Chorus(Default::default()),
        || Self::Compressor(Default::default()),
        || Self::Limiter(Default::default()),
        || Self::Reverb(Default::default()),
        || Self::CloseUp(Default::default()),
        || Self::VoiceColor(Default::default()),
        || Self::Echo(Default::default()),
        || Self::Distortion(Default::default()),
        || Self::VoiceChange(Default::default()),
    ];

    fn model(&self) -> &dyn ModifierModel {
        match self {
            Self::Cache(value) => value,
            Self::Gain(value) => value,
            Self::Pan(value) => value,
            Self::Pitch(value) => value.as_ref(),
            Self::Denoise(value) => value.as_ref(),
            Self::Equalizer(value) => value.as_ref(),
            Self::Filter(value) => value.as_ref(),
            Self::NoiseGate(value) => value.as_ref(),
            Self::StereoWidth(value) => value,
            Self::Tremolo(value) => value.as_ref(),
            Self::Bitcrusher(value) => value.as_ref(),
            Self::Chorus(value) => value.as_ref(),
            Self::Compressor(value) => value.as_ref(),
            Self::Limiter(value) => value.as_ref(),
            Self::Reverb(value) => value.as_ref(),
            Self::CloseUp(value) => value.as_ref(),
            Self::VoiceColor(value) => value.as_ref(),
            Self::Echo(value) => value.as_ref(),
            Self::Distortion(value) => value.as_ref(),
            Self::VoiceChange(value) => value.as_ref(),
        }
    }

    fn model_mut(&mut self) -> &mut dyn ModifierModel {
        match self {
            Self::Cache(value) => value,
            Self::Gain(value) => value,
            Self::Pan(value) => value,
            Self::Pitch(value) => value.as_mut(),
            Self::Denoise(value) => value.as_mut(),
            Self::Equalizer(value) => value.as_mut(),
            Self::Filter(value) => value.as_mut(),
            Self::NoiseGate(value) => value.as_mut(),
            Self::StereoWidth(value) => value,
            Self::Tremolo(value) => value.as_mut(),
            Self::Bitcrusher(value) => value.as_mut(),
            Self::Chorus(value) => value.as_mut(),
            Self::Compressor(value) => value.as_mut(),
            Self::Limiter(value) => value.as_mut(),
            Self::Reverb(value) => value.as_mut(),
            Self::CloseUp(value) => value.as_mut(),
            Self::VoiceColor(value) => value.as_mut(),
            Self::Echo(value) => value.as_mut(),
            Self::Distortion(value) => value.as_mut(),
            Self::VoiceChange(value) => value.as_mut(),
        }
    }
}

impl ModifierModel for AudioModifierEffect {
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
}
