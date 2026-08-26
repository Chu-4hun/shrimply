use std::cell::RefCell;
use std::rc::Rc;

use shrimply_core::timeline_value::TimelineValue;
use shrimply_project::project::{AudioGenerator, AudioSource, AudioWaveform, Project};

use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::{
    InspectedItem, InspectorContext,
    item::{DefaultInspectorItem, InspectorListItem},
    modifiers::{ScalarOptions, audio_item_integer_scalar_row, audio_item_scalar_row},
    section::InspectorSection,
    ui::enum_dropdown,
};

const MAX_EXACT_F32_INTEGER: f64 = 16_777_215.0;

pub(super) fn item(generator: &AudioGenerator) -> InspectorListItem {
    DefaultInspectorItem::new(
        "audio-generator",
        "Generator",
        generator.clone(),
        controls,
        |context, _: AudioGenerator| {
            edit(context, "reset-audio-generator", |generator| {
                *generator = AudioGenerator::default();
            });
        },
    )
    .boxed()
}

fn controls(generator: &AudioGenerator, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    section.add_control_row(
        "Waveform",
        &enum_dropdown(generator.waveform, move |waveform| {
            edit_parts(
                &project,
                &player,
                &key,
                "change-audio-generator-waveform",
                |generator| {
                    if generator.waveform == waveform {
                        return false;
                    }
                    generator.waveform = waveform;
                    true
                },
            );
        }),
    );
    if !generator.waveform.is_noise() {
        section.add_wide_control(&audio_item_scalar_row(
            "Frequency",
            &generator.frequency_hz,
            frequency,
            frequency_mut,
            ScalarOptions {
                minimum: Some(1.0),
                maximum: Some(20_000.0),
                unit: Some("Hz"),
                rotating: false,
            },
            context,
        ));
    }
    if generator.waveform == AudioWaveform::SquarePulse {
        section.add_wide_control(&audio_item_scalar_row(
            "Pulse width",
            &generator.pulse_width,
            pulse_width,
            pulse_width_mut,
            ScalarOptions {
                minimum: Some(0.01),
                maximum: Some(0.99),
                unit: Some("%"),
                rotating: false,
            },
            context,
        ));
    }
    if generator.waveform.is_noise() {
        section.add_wide_control(&audio_item_integer_scalar_row(
            "Seed",
            &generator.seed,
            seed,
            seed_mut,
            ScalarOptions {
                minimum: Some(0.0),
                maximum: Some(MAX_EXACT_F32_INTEGER),
                unit: None,
                rotating: false,
            },
            context,
        ));
    }
    vec![section.into_widget()]
}

fn frequency(project: &Project, key: InspectedItem) -> Option<&TimelineValue<f32>> {
    Some(&generator(project, &key)?.frequency_hz)
}

fn frequency_mut(project: &mut Project, key: InspectedItem) -> Option<&mut TimelineValue<f32>> {
    Some(&mut generator_mut(project, &key)?.frequency_hz)
}

fn pulse_width(project: &Project, key: InspectedItem) -> Option<&TimelineValue<f32>> {
    Some(&generator(project, &key)?.pulse_width)
}

fn pulse_width_mut(project: &mut Project, key: InspectedItem) -> Option<&mut TimelineValue<f32>> {
    Some(&mut generator_mut(project, &key)?.pulse_width)
}

fn seed(project: &Project, key: InspectedItem) -> Option<&TimelineValue<f32>> {
    Some(&generator(project, &key)?.seed)
}

fn seed_mut(project: &mut Project, key: InspectedItem) -> Option<&mut TimelineValue<f32>> {
    Some(&mut generator_mut(project, &key)?.seed)
}

fn generator<'a>(project: &'a Project, key: &InspectedItem) -> Option<&'a AudioGenerator> {
    let AudioSource::Generator(generator) = &project.audio_item(key)?.source else {
        return None;
    };
    Some(generator)
}

fn generator_mut<'a>(
    project: &'a mut Project,
    key: &InspectedItem,
) -> Option<&'a mut AudioGenerator> {
    let AudioSource::Generator(generator) = &mut project.audio_item_mut(key)?.source else {
        return None;
    };
    Some(generator)
}

fn edit(context: &InspectorContext, tag: &'static str, change: impl FnOnce(&mut AudioGenerator)) {
    edit_parts(
        &context.project,
        &context.player_state,
        &context.selected_item,
        tag,
        |generator| {
            change(generator);
            true
        },
    );
}

fn edit_parts(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: &Option<InspectedItem>,
    tag: &'static str,
    change: impl FnOnce(&mut AudioGenerator) -> bool,
) {
    let Some(key) = key else { return };
    let mut project = project.borrow_mut();
    let Some(generator) = generator_mut(&mut project, key) else {
        return;
    };
    if !change(generator) {
        return;
    }
    shrimply_project::project::commit_edit(&project, tag);
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            audio: true,
            audio_waveforms: true,
            inspector: true,
            ..Default::default()
        },
    );
}
