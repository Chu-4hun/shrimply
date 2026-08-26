use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use shrimply_math_core::Fraction;
use shrimply_project::project::{
    MAX_MOTION_BLUR_SAMPLES, MAX_MOTION_BLUR_SHUTTER_ANGLE_DEGREES,
    MAX_MOTION_BLUR_SHUTTER_PHASE_DEGREES, MIN_MOTION_BLUR_SAMPLES,
    MIN_MOTION_BLUR_SHUTTER_ANGLE_DEGREES, MIN_MOTION_BLUR_SHUTTER_PHASE_DEGREES, Project,
    RepeatStrategy, Time, VideoItem, VisualMotionBlur, default_playback_speed, native_playback_fps,
    playback_speed_or_default,
};

use crate::InspectedItem as SelectedItem;
use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::ui::{NumberPicker, dropdown};

use super::super::{
    Inspectable, InspectorContext,
    item::{DefaultInspectorItem, InspectorListItem},
    section::InspectorSection,
    selector::enum_selector,
};
use super::{apply_video_reset, update_video_item};

pub(super) fn speed_item(item: &VideoItem) -> InspectorListItem {
    DefaultInspectorItem::new(
        "speed",
        "Speed",
        PlaybackSpeed(item.playback_speed),
        Inspectable::controls,
        |context, value: PlaybackSpeed| {
            let Some(key) = context.selected_item.clone() else {
                return;
            };
            update_video_speed(&context.project, &context.player_state, key, value.0);
            shrimply_project::project::commit_edit(&context.project.borrow(), "video-speed");
            (context.refresh)();
        },
    )
    .boxed()
}

pub(super) fn frame_rate_item(item: &VideoItem) -> InspectorListItem {
    DefaultInspectorItem::new(
        "frame-rate",
        "Frame rate",
        PlaybackFrameRate(item.playback_fps),
        playback_frame_rate_controls,
        |context, _: PlaybackFrameRate| {
            apply_video_reset(context, "reset-video-frame-rate", |item| {
                item.playback_fps = native_playback_fps()
            });
        },
    )
    .boxed()
}

pub(super) fn motion_blur_item(item: &VideoItem) -> InspectorListItem {
    DefaultInspectorItem::new(
        "motion-blur",
        "Motion blur",
        item.motion_blur,
        motion_blur_controls,
        |context, value: VisualMotionBlur| {
            apply_video_reset(context, "reset-motion-blur", move |item| {
                item.motion_blur = value
            });
        },
    )
    .boxed()
}

pub(super) fn repeat_item(item: &VideoItem) -> InspectorListItem {
    DefaultInspectorItem::new(
        "repeat",
        "Repeat",
        RepeatValue(item.repeat_strategy),
        Inspectable::controls,
        |context, value: RepeatValue| {
            apply_video_reset(context, "reset-video-repeat", move |item| {
                item.repeat_strategy = value.0
            });
        },
    )
    .boxed()
}

struct PlaybackSpeed(Fraction);

impl Default for PlaybackSpeed {
    fn default() -> Self {
        Self(default_playback_speed())
    }
}

impl Inspectable for PlaybackSpeed {
    fn title(&self) -> &'static str {
        "Speed"
    }

    fn default_action(&self, context: &InspectorContext) -> Option<Box<dyn Fn() + 'static>> {
        let key = context.selected_item.clone()?;
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let refresh = context.refresh.clone();
        Some(Box::new(move || {
            update_video_speed(
                &project,
                &player_state,
                key.clone(),
                default_playback_speed(),
            );
            shrimply_project::project::commit_edit(&project.borrow(), "video-speed");
            refresh();
        }))
    }

    fn add_rows(&self, section: &InspectorSection, context: &InspectorContext) {
        let Some(key) = context.selected_item.clone() else {
            return;
        };
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let commit_project = context.project.clone();
        let refresh = context.refresh.clone();
        let picker = NumberPicker::fraction_builder(playback_speed_or_default(self.0))
            .drag_step(0.05)
            .digits(2)
            .unit_name("x")
            .width_chars(9)
            .on_change_fraction(move |value| {
                update_video_speed(&project, &player_state, key.clone(), value)
            })
            .on_commit(move |_| {
                shrimply_project::project::commit_edit(&commit_project.borrow(), "video-speed");
                refresh();
            })
            .build();
        section.add_control_row("Value", &picker);
    }
}

#[derive(Clone, Copy)]
struct PlaybackFrameRate(Fraction);

impl Default for PlaybackFrameRate {
    fn default() -> Self {
        Self(native_playback_fps())
    }
}

fn playback_frame_rate_controls(
    value: &PlaybackFrameRate,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let Some(key) = context.selected_item.clone() else {
        return Vec::new();
    };
    let mut options = vec![(native_playback_fps(), "Native".to_string())];
    options.extend(
        shrimply_project::project::COMMON_FRAME_RATES
            .iter()
            .map(|rate| (rate.value, rate.label.to_string())),
    );
    if options.iter().all(|(fps, _)| *fps != value.0) {
        options.push((
            value.0,
            shrimply_project::project::fraction_as_label(value.0),
        ));
    }
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let dropdown = dropdown(value.0, options, move |fps| {
        update_video_item(
            &project,
            &player_state,
            key.clone(),
            "video-frame-rate",
            |item| {
                if item.playback_fps == fps {
                    return false;
                }
                item.playback_fps = fps;
                true
            },
        );
    });
    let section = InspectorSection::controls();
    section.add_control_row("FPS", &dropdown);
    vec![section.into_widget()]
}

#[derive(Default)]
struct RepeatValue(RepeatStrategy);

impl Inspectable for RepeatValue {
    fn title(&self) -> &'static str {
        "Repeat"
    }

    fn add_rows(&self, section: &InspectorSection, context: &InspectorContext) {
        let Some(key) = context.selected_item.clone() else {
            return;
        };
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let dropdown = enum_selector("Strategy", self.0, move |strategy| {
            update_video_item(
                &project,
                &player_state,
                key.clone(),
                "video-repeat",
                |item| {
                    if item.repeat_strategy == strategy {
                        return false;
                    }
                    item.repeat_strategy = strategy;
                    true
                },
            );
        });
        section.add_wide_control(&dropdown);
    }
}

fn motion_blur_controls(
    motion_blur: &VisualMotionBlur,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    let Some(key) = context.selected_item.clone() else {
        return vec![section.into_widget()];
    };

    let angle_project = context.project.clone();
    let angle_player = context.player_state.clone();
    let angle_key = key.clone();
    let angle = NumberPicker::integer_builder(motion_blur.shutter_angle_degrees)
        .minimum(f64::from(MIN_MOTION_BLUR_SHUTTER_ANGLE_DEGREES))
        .maximum(f64::from(MAX_MOTION_BLUR_SHUTTER_ANGLE_DEGREES))
        .unit_name("°")
        .on_change_integer(move |value: u32| {
            update_video_item(
                &angle_project,
                &angle_player,
                angle_key.clone(),
                "motion-blur-angle",
                |item| {
                    if item.motion_blur.shutter_angle_degrees == value {
                        return false;
                    }
                    item.motion_blur.shutter_angle_degrees = value;
                    true
                },
            );
        })
        .build();
    angle.set_sensitive(motion_blur.enabled);

    let phase_project = context.project.clone();
    let phase_player = context.player_state.clone();
    let phase_key = key.clone();
    let phase = NumberPicker::integer_builder(motion_blur.shutter_phase_degrees)
        .minimum(f64::from(MIN_MOTION_BLUR_SHUTTER_PHASE_DEGREES))
        .maximum(f64::from(MAX_MOTION_BLUR_SHUTTER_PHASE_DEGREES))
        .unit_name("°")
        .on_change_integer(move |value: i32| {
            update_video_item(
                &phase_project,
                &phase_player,
                phase_key.clone(),
                "motion-blur-phase",
                |item| {
                    if item.motion_blur.shutter_phase_degrees == value {
                        return false;
                    }
                    item.motion_blur.shutter_phase_degrees = value;
                    true
                },
            );
        })
        .build();
    phase.set_sensitive(motion_blur.enabled);

    let samples_project = context.project.clone();
    let samples_player = context.player_state.clone();
    let samples_key = key.clone();
    let samples = NumberPicker::integer_builder(motion_blur.samples)
        .minimum(f64::from(MIN_MOTION_BLUR_SAMPLES))
        .maximum(f64::from(MAX_MOTION_BLUR_SAMPLES))
        .on_change_integer(move |value: u32| {
            update_video_item(
                &samples_project,
                &samples_player,
                samples_key.clone(),
                "motion-blur-samples",
                |item| {
                    if item.motion_blur.samples == value {
                        return false;
                    }
                    item.motion_blur.samples = value;
                    true
                },
            );
        })
        .build();
    samples.set_sensitive(motion_blur.enabled);

    let enabled = gtk::Switch::builder()
        .active(motion_blur.enabled)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    let enabled_project = context.project.clone();
    let enabled_player = context.player_state.clone();
    let enabled_angle = angle.clone();
    let enabled_phase = phase.clone();
    let enabled_samples = samples.clone();
    enabled.connect_active_notify(move |enabled| {
        let active = enabled.is_active();
        enabled_angle.set_sensitive(active);
        enabled_phase.set_sensitive(active);
        enabled_samples.set_sensitive(active);
        update_video_item(
            &enabled_project,
            &enabled_player,
            key.clone(),
            "motion-blur-enabled",
            |item| {
                if item.motion_blur.enabled == active {
                    return false;
                }
                item.motion_blur.enabled = active;
                true
            },
        );
    });

    section.add_control_row("Enabled", &enabled);
    section.add_control_row("Angle", &angle);
    section.add_control_row("Phase", &phase);
    section.add_control_row("Samples", &samples);
    vec![section.into_widget()]
}

fn update_video_speed(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    value: Fraction,
) {
    let mut project = project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    let value = playback_speed_or_default(value);
    if item.playback_speed == value {
        return;
    }

    if shrimply_project::project::playback_speed_is_negative(item.playback_speed)
        != shrimply_project::project::playback_speed_is_negative(value)
    {
        item.time_offset = shrimply_project::project::video_source_time_at(item, item.end)
            .unwrap_or_else(|| {
                if shrimply_project::project::playback_speed_is_negative(value) {
                    item.source_duration
                } else {
                    Time::ZERO
                }
            });
    }
    item.playback_speed = value;
    let duration = project.duration();
    drop(project);
    player_state::refresh_project(
        player_state,
        ProjectChange {
            duration: Some(duration),
            video: true,
            ..ProjectChange::default()
        },
    );
}
