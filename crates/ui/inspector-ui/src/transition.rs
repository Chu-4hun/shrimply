use crate::player_state::{self, ProjectChange};
use crate::ui::NumberPicker;
use shrimply_project::project::{
    AudioClipTransition, AudioClipTransitionCurve, AudioTransition, DrawingFillMode, Interpolation,
    ItemAddress, MAX_VISUAL_CLIP_TRANSITION_CLOCK_SOFTNESS,
    MAX_VISUAL_CLIP_TRANSITION_DISSOLVE_GRAIN_SIZE, MAX_VISUAL_CLIP_TRANSITION_SOFTNESS,
    MAX_VISUAL_CLIP_TRANSITION_ZOOM_SCALE, MorphUnit, Project, TransitionSide,
    VisualClipTransition, VisualClipTransitionKind, VisualTransition, VisualTransitionKind,
    WriteOrdering,
};
use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::{ColorPicker, switch_row};

use super::{Inspectable, InspectorContext, section::InspectorSection, selector::selector};

pub(super) enum TransitionInspection {
    Visual {
        side: TransitionSide,
        transition: VisualTransition,
        path_animated: bool,
        text: bool,
        drawing: bool,
    },
    Audio {
        side: TransitionSide,
        transition: AudioTransition,
    },
    VisualClip(VisualClipTransition),
    AudioClip(AudioClipTransition),
}

pub(super) fn resolve(
    project: &Project,
    key: &ItemAddress,
    side: TransitionSide,
) -> Option<TransitionInspection> {
    match key {
        ItemAddress::Video { .. } => {
            let item = project.video_item(key)?;
            if side == TransitionSide::Outro
                && let Some(transition) = item.transitions.to_next.as_ref()
            {
                return Some(TransitionInspection::VisualClip(*transition));
            }
            let transition = match side {
                TransitionSide::Intro => item.transitions.intro.as_ref(),
                TransitionSide::Outro => item.transitions.outro.as_ref(),
            }?
            .clone();
            Some(TransitionInspection::Visual {
                side,
                transition,
                path_animated: item.supports_vector_transitions(),
                text: matches!(
                    item.content,
                    shrimply_project::project::VideoItemContent::Text(_)
                ),
                drawing: matches!(
                    item.content,
                    shrimply_project::project::VideoItemContent::Paint(_)
                ),
            })
        }
        ItemAddress::Audio { .. } => {
            let item = project.audio_item(key)?;
            if side == TransitionSide::Outro
                && let Some(transition) = item.transitions.to_next.as_ref()
            {
                return Some(TransitionInspection::AudioClip((**transition).clone()));
            }
            let transition = match side {
                TransitionSide::Intro => item.transitions.intro.as_ref(),
                TransitionSide::Outro => item.transitions.outro.as_ref(),
            }?
            .clone();
            Some(TransitionInspection::Audio { side, transition })
        }
        ItemAddress::Caption { .. } => None,
    }
}

impl Inspectable for TransitionInspection {
    fn title(&self) -> &'static str {
        match self {
            Self::VisualClip(_) | Self::AudioClip(_) => "Transition",
            Self::Visual { side, .. } | Self::Audio { side, .. } => match side {
                TransitionSide::Intro => "Intro",
                TransitionSide::Outro => "Outro",
            },
        }
    }

    fn add_rows(&self, section: &InspectorSection, context: &InspectorContext) {
        match self {
            Self::VisualClip(transition) => visual_clip_rows(section, context, transition),
            Self::AudioClip(transition) => audio_clip_rows(section, context, transition),
            Self::Visual {
                side,
                transition,
                path_animated,
                text,
                drawing,
                ..
            } => visual_rows(
                section,
                context,
                *side,
                transition,
                *path_animated,
                *text,
                *drawing,
            ),
            Self::Audio {
                side, transition, ..
            } => audio_rows(section, context, *side, transition),
        }
    }
}

fn visual_clip_rows(
    section: &InspectorSection,
    context: &InspectorContext,
    transition: &VisualClipTransition,
) {
    let change_context = context.detached();
    let kind = selector(
        "Kind",
        transition.kind,
        [
            (VisualClipTransitionKind::CrossFade, "Cross Fade"),
            (
                VisualClipTransitionKind::FadeThroughColor,
                "Fade Through Color",
            ),
            (VisualClipTransitionKind::Wipe, "Wipe"),
            (VisualClipTransitionKind::Morph, "Morph"),
            (VisualClipTransitionKind::Iris, "Iris"),
            (VisualClipTransitionKind::ClockWipe, "Clock Wipe"),
            (VisualClipTransitionKind::Dissolve, "Dissolve"),
            (VisualClipTransitionKind::Slide, "Slide"),
            (VisualClipTransitionKind::Push, "Push"),
            (VisualClipTransitionKind::Zoom, "Zoom"),
        ],
        move |kind| {
            update_visual_clip(&change_context, true, |transition| {
                transition.set_kind(kind)
            })
        },
    );
    section.add_wide_control(&kind);

    let curve_context = context.detached();
    let curve = selector(
        "Curve",
        transition.interpolation,
        Interpolation::CONTINUOUS.map(|value| (value, value.label())),
        move |interpolation| {
            update_visual_clip(&curve_context, true, |transition| {
                transition.interpolation = interpolation
            })
        },
    );
    section.add_wide_control(&curve);

    match transition.kind {
        VisualClipTransitionKind::FadeThroughColor => {
            let color = transition.fade_color;
            let color_context = context.detached();
            let button = ColorPicker::builder(color.with_alpha(u8::MAX))
                .title(tr!("Fade-through color").as_ref())
                .with_alpha(false)
                .hexpand(true)
                .on_change(move |color| {
                    update_visual_clip(&color_context, true, |transition| {
                        transition.fade_color = color.with_alpha(u8::MAX)
                    })
                })
                .build();
            section.add_control_row("Color", &button);
        }
        VisualClipTransitionKind::Wipe => {
            add_visual_clip_number(
                section,
                context,
                "Direction",
                transition.direction_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.direction_degrees,
            );
            add_visual_clip_number(
                section,
                context,
                "Softness",
                transition.softness,
                0.0,
                f64::from(MAX_VISUAL_CLIP_TRANSITION_SOFTNESS),
                0.01,
                2,
                None,
                |value| &mut value.softness,
            );
        }
        VisualClipTransitionKind::Iris => {
            add_visual_clip_center(section, context, transition);
            let direction_context = context.detached();
            let direction = selector(
                "Direction",
                transition.iris_from_inside,
                [(true, "From inside"), (false, "From outside")],
                move |from_inside| {
                    update_visual_clip(&direction_context, true, |transition| {
                        transition.iris_from_inside = from_inside
                    })
                },
            );
            section.add_wide_control(&direction);
            add_visual_clip_number(
                section,
                context,
                "Softness",
                transition.softness,
                0.0,
                f64::from(MAX_VISUAL_CLIP_TRANSITION_SOFTNESS),
                0.01,
                2,
                None,
                |value| &mut value.softness,
            );
        }
        VisualClipTransitionKind::ClockWipe => {
            add_visual_clip_center(section, context, transition);
            add_visual_clip_number(
                section,
                context,
                "Starting angle",
                transition.direction_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.direction_degrees,
            );
            let direction_context = context.detached();
            let direction = selector(
                "Direction",
                transition.clockwise,
                [(true, "Clockwise"), (false, "Counterclockwise")],
                move |clockwise| {
                    update_visual_clip(&direction_context, true, |transition| {
                        transition.clockwise = clockwise
                    })
                },
            );
            section.add_wide_control(&direction);
            add_visual_clip_number(
                section,
                context,
                "Softness",
                transition.softness,
                0.0,
                f64::from(MAX_VISUAL_CLIP_TRANSITION_CLOCK_SOFTNESS),
                0.01,
                2,
                None,
                |value| &mut value.softness,
            );
        }
        VisualClipTransitionKind::Dissolve => {
            let change_context = context.detached();
            let commit_context = context.detached();
            let grain = NumberPicker::builder(f64::from(transition.dissolve_grain_size))
                .minimum(1.0)
                .maximum(f64::from(MAX_VISUAL_CLIP_TRANSITION_DISSOLVE_GRAIN_SIZE))
                .drag_step(1.0)
                .digits(0)
                .unit_name("px")
                .on_change(move |value| {
                    update_visual_clip(&change_context, false, |transition| {
                        transition.dissolve_grain_size = value.round() as u32
                    })
                })
                .on_commit(move |_| commit(&commit_context, "visual-clip-transition-config"))
                .build();
            section.add_control_row("Grain size", &grain);
        }
        VisualClipTransitionKind::Slide | VisualClipTransitionKind::Push => {
            add_visual_clip_number(
                section,
                context,
                "Direction",
                transition.direction_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.direction_degrees,
            );
            add_visual_clip_fade(section, context, transition.fade_opacity);
        }
        VisualClipTransitionKind::Zoom => {
            add_visual_clip_center(section, context, transition);
            add_visual_clip_number(
                section,
                context,
                "Starting scale",
                transition.zoom_start_scale,
                0.0,
                f64::from(MAX_VISUAL_CLIP_TRANSITION_ZOOM_SCALE),
                0.01,
                2,
                Some("x"),
                |value| &mut value.zoom_start_scale,
            );
            add_visual_clip_fade(section, context, transition.fade_opacity);
        }
        VisualClipTransitionKind::CrossFade | VisualClipTransitionKind::Morph => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn add_visual_clip_number(
    section: &InspectorSection,
    context: &InspectorContext,
    label: &str,
    value: f32,
    minimum: f64,
    maximum: f64,
    step: f64,
    digits: usize,
    unit: Option<&str>,
    field: fn(&mut VisualClipTransition) -> &mut f32,
) {
    let change_context = context.detached();
    let commit_context = context.detached();
    let mut picker = NumberPicker::builder(f64::from(value))
        .minimum(minimum)
        .maximum(maximum)
        .drag_step(step)
        .digits(digits);
    if let Some(unit) = unit {
        picker = picker.unit_name(unit);
    }
    let picker = picker
        .on_change(move |value| {
            update_visual_clip(&change_context, false, |transition| {
                *field(transition) = value as f32
            })
        })
        .on_commit(move |_| commit(&commit_context, "visual-clip-transition-config"))
        .build();
    section.add_control_row(label, &picker);
}

fn add_visual_clip_center(
    section: &InspectorSection,
    context: &InspectorContext,
    transition: &VisualClipTransition,
) {
    add_visual_clip_number(
        section,
        context,
        "Center X",
        transition.center.x,
        0.0,
        1.0,
        0.01,
        2,
        None,
        |value| &mut value.center.x,
    );
    add_visual_clip_number(
        section,
        context,
        "Center Y",
        transition.center.y,
        0.0,
        1.0,
        0.01,
        2,
        None,
        |value| &mut value.center.y,
    );
}

fn add_visual_clip_fade(section: &InspectorSection, context: &InspectorContext, active: bool) {
    let fade_context = context.detached();
    let fade = switch_row("Fade opacity", None, active, move |active| {
        update_visual_clip(&fade_context, true, |transition| {
            transition.fade_opacity = active
        })
    });
    section.add_wide_control(&fade);
}

fn audio_clip_rows(
    section: &InspectorSection,
    context: &InspectorContext,
    transition: &AudioClipTransition,
) {
    let change_context = context.detached();
    let curve = selector(
        "Curve",
        transition.curve,
        [
            (AudioClipTransitionCurve::EqualPower, "Equal Power"),
            (AudioClipTransitionCurve::Linear, "Linear"),
        ],
        move |curve| update_audio_clip(&change_context, |transition| transition.curve = curve),
    );
    section.add_wide_control(&curve);
}

fn visual_rows(
    section: &InspectorSection,
    context: &InspectorContext,
    side: TransitionSide,
    transition: &VisualTransition,
    path_animated: bool,
    text: bool,
    drawing: bool,
) {
    let kind_context = context.detached();
    let mut kinds = vec![
        (VisualTransitionKind::Fade, "Fade"),
        (VisualTransitionKind::Slide, "Slide"),
        (VisualTransitionKind::SlideFade, "Slide + Fade"),
        (VisualTransitionKind::Wipe, "Wipe"),
        (VisualTransitionKind::Iris, "Iris"),
        (VisualTransitionKind::ClockWipe, "Clock Wipe"),
        (VisualTransitionKind::Zoom, "Zoom"),
        (VisualTransitionKind::Spin, "Spin"),
        (VisualTransitionKind::Blur, "Blur"),
        (VisualTransitionKind::Pixelate, "Pixelate"),
        (VisualTransitionKind::Dissolve, "Dissolve"),
        (VisualTransitionKind::TriangularFold, "Triangular Fold"),
        (VisualTransitionKind::Origami, "Origami"),
        (VisualTransitionKind::StreakWipe, "Streak Wipe"),
    ];
    if text {
        kinds.push((VisualTransitionKind::Morph, "Morph"));
    }
    if drawing {
        kinds.push((VisualTransitionKind::Drawing, "Drawing"));
    }
    if path_animated {
        kinds.extend([
            (
                VisualTransitionKind::Write,
                match side {
                    TransitionSide::Intro => "Write",
                    TransitionSide::Outro => "Unwrite",
                },
            ),
            (VisualTransitionKind::Create, "Create"),
            (VisualTransitionKind::FacetAssembly, "Facet Assembly"),
            (VisualTransitionKind::Coalesce, "Coalesce"),
            (VisualTransitionKind::ContourCurrent, "Contour Current"),
            (VisualTransitionKind::SoftRefraction, "Soft Refraction"),
            (
                VisualTransitionKind::MorphologicalResolve,
                "Morphological Resolve",
            ),
            (VisualTransitionKind::LivingFill, "Living Fill"),
        ]);
        kinds.push(match side {
            TransitionSide::Intro => (VisualTransitionKind::ReverseDiffusion, "Reverse Diffusion"),
            TransitionSide::Outro => (VisualTransitionKind::Diffusion, "Diffusion"),
        });
    }
    let kind = selector("Kind", transition.kind, kinds, move |kind| {
        update_visual(&kind_context, side, true, |value| {
            value.set_kind(side, kind);
        })
    });
    section.add_wide_control(&kind);
    section.add_wide_control(&curve_selector(
        context,
        side,
        transition.interpolation,
        true,
    ));

    if text && transition.kind == VisualTransitionKind::Morph {
        let unit_context = context.detached();
        let unit = selector(
            "Morph by",
            transition.morph_unit,
            [(MorphUnit::Letter, "Letter"), (MorphUnit::Word, "Word")],
            move |unit| update_visual(&unit_context, side, true, |value| value.morph_unit = unit),
        );
        section.add_wide_control(&unit);
    }

    if drawing && transition.kind == VisualTransitionKind::Drawing {
        let change_context = context.detached();
        let commit_context = context.detached();
        let overlap = NumberPicker::builder(f64::from(transition.drawing_stroke_overlap * 100.0))
            .minimum(-100.0)
            .maximum(100.0)
            .drag_step(1.0)
            .digits(0)
            .unit_name("%")
            .on_change(move |value| {
                update_visual(&change_context, side, false, |transition| {
                    transition.drawing_stroke_overlap = (value as f32 / 100.0).clamp(-1.0, 1.0)
                })
            })
            .on_commit(move |_| commit(&commit_context, "transition-drawing-overlap"))
            .build();
        section.add_control_row("Stroke overlap", &overlap);

        let change_context = context.detached();
        let commit_context = context.detached();
        let length =
            NumberPicker::builder(f64::from(transition.drawing_stroke_length_weight * 100.0))
                .minimum(0.0)
                .maximum(100.0)
                .drag_step(1.0)
                .digits(0)
                .unit_name("%")
                .on_change(move |value| {
                    update_visual(&change_context, side, false, |transition| {
                        transition.drawing_stroke_length_weight =
                            (value as f32 / 100.0).clamp(0.0, 1.0)
                    })
                })
                .on_commit(move |_| commit(&commit_context, "transition-drawing-length"))
                .build();
        section.add_control_row("Length timing", &length);

        let fill_context = context.detached();
        let fill = selector(
            "Fill",
            transition.drawing_fill_mode,
            [
                (DrawingFillMode::FadeTogether, "Fade together"),
                (DrawingFillMode::FadeSequentially, "Fade one by one"),
                (DrawingFillMode::Direct, "Direct"),
            ],
            move |mode| {
                update_visual(&fill_context, side, true, |transition| {
                    transition.drawing_fill_mode = mode
                })
            },
        );
        section.add_wide_control(&fill);
    }

    if matches!(
        transition.kind,
        VisualTransitionKind::Slide | VisualTransitionKind::SlideFade
    ) {
        let change_context = context.detached();
        let commit_context = context.detached();
        let rotation = NumberPicker::builder(f64::from(transition.slide_rotation_degrees))
            .drag_step(1.0)
            .digits(1)
            .unit_name("°")
            .on_change(move |value| {
                update_visual(&change_context, side, false, |transition| {
                    transition.slide_rotation_degrees = value as f32
                })
            })
            .on_commit(move |_| commit(&commit_context, "transition-slide-rotation"))
            .build();
        section.add_control_row("Rotation", &rotation);

        let change_context = context.detached();
        let commit_context = context.detached();
        let distance = NumberPicker::builder(f64::from(transition.slide_distance))
            .drag_step(1.0)
            .digits(1)
            .minimum(0.0)
            .unit_name("px")
            .on_change(move |value| {
                update_visual(&change_context, side, false, |transition| {
                    transition.slide_distance = value.max(0.0) as f32
                })
            })
            .on_commit(move |_| commit(&commit_context, "transition-slide-distance"))
            .build();
        section.add_control_row("Distance", &distance);
    }

    match transition.kind {
        VisualTransitionKind::Wipe => {
            add_effect_number(
                section,
                context,
                side,
                "Direction",
                transition.effect_angle_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.effect_angle_degrees,
            );
            add_effect_number(
                section,
                context,
                side,
                "Softness",
                transition.effect_detail,
                0.0,
                0.5,
                0.01,
                2,
                None,
                |value| &mut value.effect_detail,
            );
        }
        VisualTransitionKind::Iris => {
            let direction_context = context.detached();
            let direction = selector(
                "Direction",
                match side {
                    TransitionSide::Intro => transition.effect_amount >= 0.5,
                    TransitionSide::Outro => transition.effect_amount < 0.5,
                },
                [(true, "From inside"), (false, "From outside")],
                move |from_inside| {
                    update_visual(&direction_context, side, true, |value| {
                        value.effect_amount = match side {
                            TransitionSide::Intro if from_inside => 1.0,
                            TransitionSide::Outro if !from_inside => 1.0,
                            _ => 0.0,
                        }
                    })
                },
            );
            section.add_wide_control(&direction);
            add_effect_number(
                section,
                context,
                side,
                "Center X",
                transition.iris_center.x,
                0.0,
                1.0,
                0.01,
                2,
                None,
                |value| &mut value.iris_center.x,
            );
            add_effect_number(
                section,
                context,
                side,
                "Center Y",
                transition.iris_center.y,
                0.0,
                1.0,
                0.01,
                2,
                None,
                |value| &mut value.iris_center.y,
            );
            add_effect_number(
                section,
                context,
                side,
                "Softness",
                transition.effect_detail,
                0.0,
                0.5,
                0.01,
                2,
                None,
                |value| &mut value.effect_detail,
            );
        }
        VisualTransitionKind::ClockWipe => {
            add_effect_number(
                section,
                context,
                side,
                "Starting angle",
                transition.effect_angle_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.effect_angle_degrees,
            );
            add_effect_number(
                section,
                context,
                side,
                "Softness",
                transition.effect_detail,
                0.0,
                0.25,
                0.01,
                2,
                None,
                |value| &mut value.effect_detail,
            );
            let direction_context = context.detached();
            let direction = selector(
                "Direction",
                transition.effect_amount >= 0.5,
                [(true, "Clockwise"), (false, "Counterclockwise")],
                move |clockwise| {
                    update_visual(&direction_context, side, true, |value| {
                        value.effect_amount = if clockwise { 1.0 } else { 0.0 }
                    })
                },
            );
            section.add_wide_control(&direction);
        }
        VisualTransitionKind::Zoom => {
            add_effect_number(
                section,
                context,
                side,
                "Starting scale",
                transition.effect_amount,
                0.0,
                2.0,
                0.01,
                2,
                Some("x"),
                |value| &mut value.effect_amount,
            );
        }
        VisualTransitionKind::Spin => {
            add_effect_number(
                section,
                context,
                side,
                "Starting scale",
                transition.effect_amount,
                0.0,
                2.0,
                0.01,
                2,
                Some("x"),
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Rotation",
                transition.effect_angle_degrees,
                -1440.0,
                1440.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.effect_angle_degrees,
            );
        }
        VisualTransitionKind::Blur | VisualTransitionKind::Pixelate => {
            let (label, maximum, unit) = match transition.kind {
                VisualTransitionKind::Blur => ("Maximum radius", 100.0, "px"),
                VisualTransitionKind::Pixelate => ("Maximum block size", 512.0, "px"),
                _ => unreachable!(),
            };
            add_effect_number(
                section,
                context,
                side,
                label,
                transition.effect_amount,
                1.0,
                maximum,
                1.0,
                0,
                Some(unit),
                |value| &mut value.effect_amount,
            );
            let fade_context = context.detached();
            let fade = switch_row(
                "Fade opacity",
                None,
                transition.effect_fade,
                move |active| {
                    update_visual(&fade_context, side, true, |value| {
                        value.effect_fade = active
                    })
                },
            );
            section.add_wide_control(&fade);
        }
        VisualTransitionKind::Dissolve => {
            add_effect_number(
                section,
                context,
                side,
                "Grain size",
                transition.effect_detail,
                1.0,
                64.0,
                1.0,
                0,
                Some("px"),
                |value| &mut value.effect_detail,
            );
        }
        VisualTransitionKind::TriangularFold => {
            add_effect_number(
                section,
                context,
                side,
                "Fold size",
                transition.effect_detail,
                32.0,
                512.0,
                1.0,
                0,
                Some("px"),
                |value| &mut value.effect_detail,
            );
            add_effect_number(
                section,
                context,
                side,
                "Fold depth",
                transition.effect_amount,
                0.0,
                1.0,
                0.01,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Direction",
                transition.effect_angle_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.effect_angle_degrees,
            );
        }
        VisualTransitionKind::Origami => {
            add_effect_number(
                section,
                context,
                side,
                "Complexity",
                transition.effect_detail,
                2.0,
                6.0,
                1.0,
                0,
                None,
                |value| &mut value.effect_detail,
            );
            add_effect_number(
                section,
                context,
                side,
                "Fold depth",
                transition.effect_amount,
                0.0,
                1.0,
                0.01,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Direction",
                transition.effect_angle_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.effect_angle_degrees,
            );
        }
        VisualTransitionKind::StreakWipe => {
            add_effect_number(
                section,
                context,
                side,
                "Direction",
                transition.effect_angle_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.effect_angle_degrees,
            );
            add_effect_number(
                section,
                context,
                side,
                "Line width",
                transition.effect_detail,
                1.0,
                256.0,
                1.0,
                0,
                Some("px"),
                |value| &mut value.effect_detail,
            );
            add_effect_number(
                section,
                context,
                side,
                "Variation",
                transition.effect_amount,
                0.0,
                1.0,
                0.01,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Edge softness",
                transition.effect_softness,
                0.0,
                128.0,
                1.0,
                1,
                Some("px"),
                |value| &mut value.effect_softness,
            );
        }
        VisualTransitionKind::Coalesce => {
            add_effect_number(
                section,
                context,
                side,
                "Softness",
                transition.effect_amount,
                0.25,
                2.5,
                0.05,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Pools",
                transition.effect_detail,
                2.0,
                5.0,
                1.0,
                0,
                None,
                |value| &mut value.effect_detail,
            );
        }
        VisualTransitionKind::ContourCurrent => {
            add_effect_number(
                section,
                context,
                side,
                "Line width",
                transition.effect_amount,
                0.25,
                4.0,
                0.05,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Trail length",
                transition.effect_detail,
                0.04,
                0.7,
                0.01,
                2,
                None,
                |value| &mut value.effect_detail,
            );
        }
        VisualTransitionKind::SoftRefraction => {
            add_effect_number(
                section,
                context,
                side,
                "Strength",
                transition.effect_amount,
                0.0,
                3.0,
                0.05,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Texture scale",
                transition.effect_detail,
                0.25,
                3.0,
                0.05,
                2,
                None,
                |value| &mut value.effect_detail,
            );
        }
        VisualTransitionKind::MorphologicalResolve => {
            add_effect_number(
                section,
                context,
                side,
                "Amount",
                transition.effect_amount,
                0.0,
                3.0,
                0.05,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Softness",
                transition.effect_detail,
                0.0,
                2.0,
                0.05,
                2,
                None,
                |value| &mut value.effect_detail,
            );
        }
        VisualTransitionKind::LivingFill => {
            add_effect_number(
                section,
                context,
                side,
                "Band width",
                transition.effect_amount,
                0.03,
                0.6,
                0.01,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Softness",
                transition.effect_detail,
                0.05,
                1.0,
                0.01,
                2,
                None,
                |value| &mut value.effect_detail,
            );
            add_effect_number(
                section,
                context,
                side,
                "Direction",
                transition.effect_angle_degrees,
                -180.0,
                180.0,
                1.0,
                1,
                Some("°"),
                |value| &mut value.effect_angle_degrees,
            );
        }
        VisualTransitionKind::Diffusion | VisualTransitionKind::ReverseDiffusion => {
            add_effect_number(
                section,
                context,
                side,
                "Amount",
                transition.effect_amount,
                0.0,
                3.0,
                0.05,
                2,
                None,
                |value| &mut value.effect_amount,
            );
            add_effect_number(
                section,
                context,
                side,
                "Detail",
                transition.effect_detail,
                0.25,
                3.0,
                0.05,
                2,
                None,
                |value| &mut value.effect_detail,
            );
            let fade_context = context.detached();
            let fade = switch_row(
                "Fade opacity",
                None,
                transition.effect_fade,
                move |active| {
                    update_visual(&fade_context, side, true, |value| {
                        value.effect_fade = active
                    })
                },
            );
            section.add_wide_control(&fade);

            let evolve_context = context.detached();
            let evolve = switch_row(
                "Evolve seed",
                None,
                transition.effect_evolve_seed,
                move |active| {
                    update_visual(&evolve_context, side, true, |value| {
                        value.effect_evolve_seed = active
                    })
                },
            );
            section.add_wide_control(&evolve);

            if transition.effect_evolve_seed {
                let change_context = context.detached();
                let commit_context = context.detached();
                let frequency = NumberPicker::builder(f64::from(transition.effect_seed_frequency))
                    .minimum(1.0)
                    .maximum(60.0)
                    .drag_step(1.0)
                    .digits(0)
                    .unit_name("Hz")
                    .on_change(move |value| {
                        update_visual(&change_context, side, false, |transition| {
                            transition.effect_seed_frequency = value.round().max(1.0) as u32
                        })
                    })
                    .on_commit(move |_| commit(&commit_context, "transition-seed-frequency"))
                    .build();
                section.add_control_row("Seed frequency", &frequency);
            }
        }
        _ => {}
    }

    if matches!(
        transition.kind,
        VisualTransitionKind::Write
            | VisualTransitionKind::Create
            | VisualTransitionKind::FacetAssembly
            | VisualTransitionKind::Coalesce
            | VisualTransitionKind::ContourCurrent
            | VisualTransitionKind::SoftRefraction
            | VisualTransitionKind::MorphologicalResolve
            | VisualTransitionKind::LivingFill
            | VisualTransitionKind::Diffusion
            | VisualTransitionKind::ReverseDiffusion
    ) {
        let ordering_context = context.detached();
        let ordering = selector(
            "Ordering",
            transition.write_ordering,
            [
                (WriteOrdering::Sequential, "Sequential"),
                (WriteOrdering::Simultaneous, "Simultaneous"),
            ],
            move |ordering| {
                update_visual(&ordering_context, side, true, |value| {
                    value.write_ordering = ordering
                })
            },
        );
        section.add_wide_control(&ordering);
    }
}

#[allow(clippy::too_many_arguments)]
fn add_effect_number(
    section: &InspectorSection,
    context: &InspectorContext,
    side: TransitionSide,
    label: &str,
    value: f32,
    minimum: f64,
    maximum: f64,
    step: f64,
    digits: usize,
    unit: Option<&str>,
    field: fn(&mut VisualTransition) -> &mut f32,
) {
    let change_context = context.detached();
    let commit_context = context.detached();
    let mut picker = NumberPicker::builder(f64::from(value))
        .minimum(minimum)
        .maximum(maximum)
        .drag_step(step)
        .digits(digits);
    if let Some(unit) = unit {
        picker = picker.unit_name(unit);
    }
    let picker = picker
        .on_change(move |value| {
            update_visual(&change_context, side, false, |transition| {
                *field(transition) = value as f32
            })
        })
        .on_commit(move |_| commit(&commit_context, "transition-effect-config"))
        .build();
    section.add_control_row(label, &picker);
}

fn audio_rows(
    section: &InspectorSection,
    context: &InspectorContext,
    side: TransitionSide,
    transition: &AudioTransition,
) {
    let kind = selector(
        "Kind",
        transition.kind,
        [(shrimply_project::project::AudioTransitionKind::Fade, "Fade")],
        |_| {},
    );
    section.add_wide_control(&kind);
    section.add_wide_control(&curve_selector(
        context,
        side,
        transition.interpolation,
        false,
    ));
}

fn curve_selector(
    context: &InspectorContext,
    side: TransitionSide,
    interpolation: Interpolation,
    visual: bool,
) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Curve",
        interpolation,
        Interpolation::CONTINUOUS.map(|value| (value, value.label())),
        move |interpolation| {
            if visual {
                update_visual(&context, side, true, |value| {
                    value.interpolation = interpolation
                });
            } else {
                update_audio(&context, side, true, |value| {
                    value.interpolation = interpolation
                });
            }
        },
    )
}

fn update_visual(
    context: &InspectorContext,
    side: TransitionSide,
    commit_change: bool,
    update: impl FnOnce(&mut VisualTransition),
) {
    let Some(key) = &context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.video_item_mut(key) else {
        return;
    };
    let transition = match side {
        TransitionSide::Intro => item.transitions.intro.as_mut(),
        TransitionSide::Outro => item.transitions.outro.as_mut(),
    };
    let Some(transition) = transition else {
        return;
    };
    update(transition);
    if commit_change {
        shrimply_project::project::commit_edit(&project, "edit-visual-transition");
    }
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: commit_change,
            ..ProjectChange::default()
        },
    );
}

fn update_audio(
    context: &InspectorContext,
    side: TransitionSide,
    commit_change: bool,
    update: impl FnOnce(&mut AudioTransition),
) {
    let Some(key) = &context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.audio_item_mut(key) else {
        return;
    };
    let transition = match side {
        TransitionSide::Intro => item.transitions.intro.as_mut(),
        TransitionSide::Outro => item.transitions.outro.as_mut(),
    };
    let Some(transition) = transition else {
        return;
    };
    update(transition);
    if commit_change {
        shrimply_project::project::commit_edit(&project, "edit-audio-transition");
    }
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            audio: true,
            inspector: commit_change,
            ..ProjectChange::default()
        },
    );
}

fn update_visual_clip(
    context: &InspectorContext,
    commit_change: bool,
    update: impl FnOnce(&mut VisualClipTransition),
) {
    let Some(key) = &context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(transition) = project
        .video_item_mut(key)
        .and_then(|item| item.transitions.to_next.as_mut())
    else {
        return;
    };
    update(transition);
    if commit_change {
        shrimply_project::project::commit_edit(&project, "edit-visual-clip-transition");
    }
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: commit_change,
            ..ProjectChange::default()
        },
    );
}

fn update_audio_clip(context: &InspectorContext, update: impl FnOnce(&mut AudioClipTransition)) {
    let Some(key) = &context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(transition) = project
        .audio_item_mut(key)
        .and_then(|item| item.transitions.to_next.as_mut())
    else {
        return;
    };
    update(transition);
    shrimply_project::project::commit_edit(&project, "edit-audio-clip-transition");
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            audio: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
}

fn commit(context: &InspectorContext, name: &str) {
    shrimply_project::project::commit_edit(&context.project.borrow(), name);
    (context.refresh)();
}
