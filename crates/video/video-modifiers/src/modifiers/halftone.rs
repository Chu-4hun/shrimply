use super::{KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::timeline_value::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HalftoneMode {
    #[default]
    Monochrome,
    RgbDots,
    CmykDots,
}

shrimply_core::timeline_value::timeline_step_type!(
    HalftoneMode,
    HalftoneMode::Monochrome,
    &[
        TimelineStepVariant {
            value: HalftoneMode::Monochrome,
            key: "monochrome",
            label: "Monochrome",
            icon: None
        },
        TimelineStepVariant {
            value: HalftoneMode::RgbDots,
            key: "rgb_dots",
            label: "RGB dots",
            icon: None
        },
        TimelineStepVariant {
            value: HalftoneMode::CmykDots,
            key: "cmyk_dots",
            label: "CMYK dots",
            icon: None
        },
    ]
);

fn default_rgb_distance() -> TimelineValue<f32> {
    TimelineValue::<f32>::new_const(4.0)
}

fn default_channel_angle_offset() -> TimelineValue<f32> {
    TimelineValue::<f32>::new_const(30.0)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HalftoneModifier {
    pub size: TimelineValue<f32>,
    pub angle_degrees: TimelineValue<f32>,
    pub contrast: TimelineValue<f32>,
    #[serde(default, deserialize_with = "deserialize_timeline_value")]
    pub mode: TimelineValue<HalftoneMode>,
    #[serde(default = "default_rgb_distance")]
    pub rgb_distance: TimelineValue<f32>,
    #[serde(default = "default_channel_angle_offset")]
    pub channel_angle_offset: TimelineValue<f32>,
}

impl Default for HalftoneModifier {
    fn default() -> Self {
        Self {
            size: TimelineValue::<f32>::new_const(8.0),
            angle_degrees: TimelineValue::<f32>::new_const(45.0),
            contrast: TimelineValue::<f32>::new_const(1.0),
            mode: TimelineValue::new_const(HalftoneMode::Monochrome),
            rgb_distance: default_rgb_distance(),
            channel_angle_offset: default_channel_angle_offset(),
        }
    }
}

impl ModifierModel for HalftoneModifier {
    fn display_name(&self) -> &'static str {
        "Halftone"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["dots", "comic", "print", "newspaper"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        ensure_timeline_value_ids(&mut self.size, seen);
        ensure_timeline_value_ids(&mut self.angle_degrees, seen);
        ensure_timeline_value_ids(&mut self.contrast, seen);
        ensure_timeline_value_ids(&mut self.mode, seen);
        ensure_timeline_value_ids(&mut self.rgb_distance, seen);
        ensure_timeline_value_ids(&mut self.channel_angle_offset, seen);
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine([
            timeline_value_span(&self.size),
            timeline_value_span(&self.angle_degrees),
            timeline_value_span(&self.contrast),
            timeline_value_span(&self.mode),
            timeline_value_span(&self.rgb_distance),
            timeline_value_span(&self.channel_angle_offset),
        ])
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        [
            &self.size,
            &self.angle_degrees,
            &self.contrast,
            &self.rgb_distance,
            &self.channel_angle_offset,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        [
            &mut self.size,
            &mut self.angle_degrees,
            &mut self.contrast,
            &mut self.rgb_distance,
            &mut self.channel_angle_offset,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }
}
