use crate::InspectedItem as SelectedItem;
use crate::{
    InspectorContext,
    player_state::ProjectChange,
    timeline_value::step::{StepTarget, step_control},
};
use gtk::prelude::BoxExt;
use shrimply_core::timeline_value::TimelineValue;
use shrimply_project::project::{Project, VideoSampleMethod};
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect, sampling::SamplingModifier};
use uuid::Uuid;

pub fn add_rows(value: &SamplingModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&step_control(
        "Method",
        &value.method,
        context,
        StepTarget::new(
            move |project, key| sampling_method(project, key.clone(), id),
            move |project, key| sampling_method_mut(project, key.clone(), id),
            "edit-raster-sampling",
            ProjectChange {
                video: true,
                inspector: true,
                ..Default::default()
            },
        ),
    ));
}

fn sampling_method(
    project: &Project,
    key: SelectedItem,
    id: Uuid,
) -> Option<&TimelineValue<VideoSampleMethod>> {
    project
        .video_item(&key)?
        .modifiers
        .iter()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &modifier.effect {
            ModifierEffect::Raster(raster) => match &**raster {
                RasterModifierEffect::Sampling(effect) => Some(&effect.method),
                _ => None,
            },
            _ => None,
        })
}

fn sampling_method_mut(
    project: &mut Project,
    key: SelectedItem,
    id: Uuid,
) -> Option<&mut TimelineValue<VideoSampleMethod>> {
    project
        .video_item_mut(&key)?
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Raster(raster) => match &mut **raster {
                RasterModifierEffect::Sampling(effect) => Some(&mut effect.method),
                _ => None,
            },
            _ => None,
        })
}
