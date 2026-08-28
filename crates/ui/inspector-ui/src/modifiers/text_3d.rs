use gtk::prelude::*;
use shrimply_core::{
    FontFamily, FontVariation, TextDirection, TextFontStyle, TextHorizontalAlign, VerticalAlign,
};
use shrimply_project::project::Project;
use shrimply_scene_3d::{MAX_IOR, MIN_IOR, MIN_ROUGHNESS, NormalMode};
use shrimply_ui_foundation::ui::MultilineTextInput;
use shrimply_video_modifiers::{
    ModifierEffect,
    scene_3d::{Scene3dModifierEffect, Text3dModifier},
};
use uuid::Uuid;

use crate::{
    InspectorContext,
    font_selector::font_selector_list,
    player_state::{self, ProjectChange},
    selector::{enum_selector, selector},
    timeline_value::layered,
};

use super::{ScalarOptions, color_row, integer_scalar_row, scalar_row, vec3_row, vec3_scale_row};

pub fn add_rows(value: &Text3dModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&text_row(value, id, context));
    out.append(&crate::ui::control_row(
        "Fonts",
        &font_families(value, id, context),
    ));
    out.append(&font_style(value.font_style, id, context));
    for control in font_variations(value, id, context) {
        out.append(&control);
    }
    out.append(&number_row(
        "Font weight",
        &value.font_weight,
        id,
        Some(1.0),
        Some(1000.0),
        None,
        context,
    ));
    out.append(&number_row(
        "Font size",
        &value.font_size,
        id,
        Some(0.001),
        None,
        None,
        context,
    ));
    out.append(&horizontal_align(value.h_align, id, context));
    out.append(&vertical_align(value.v_align, id, context));
    out.append(&direction(value.direction, id, context));
    out.append(&number_row(
        "Depth",
        &value.depth,
        id,
        Some(0.001),
        None,
        None,
        context,
    ));
    out.append(&number_row(
        "Roundness",
        &value.roundness,
        id,
        Some(0.0),
        None,
        None,
        context,
    ));
    out.append(&integer_scalar_row(
        "Smoothness",
        &value.smoothness,
        id,
        ScalarOptions {
            minimum: Some(shrimply_text_3d::MIN_SMOOTHNESS.into()),
            maximum: Some(shrimply_text_3d::MAX_SMOOTHNESS.into()),
            unit: None,
            rotating: false,
        },
        context,
    ));

    for (label, timeline, degrees) in [
        ("Position", &value.transform.position, false),
        ("Anchor", &value.transform.anchor, false),
        ("Rotation", &value.transform.rotation_degrees, true),
    ] {
        out.append(&vec3_row(label, timeline, id, degrees, context));
    }
    out.append(&vec3_scale_row(
        "Scale",
        &value.transform.scale,
        id,
        context,
    ));
    out.append(&color_row(
        "Base color",
        &value.material.base_color,
        id,
        context,
    ));
    for (label, timeline, minimum, maximum) in [
        ("Metallic", &value.material.metallic, Some(0.0), Some(1.0)),
        (
            "Roughness",
            &value.material.roughness,
            Some(MIN_ROUGHNESS as f64),
            Some(1.0),
        ),
        (
            "Subsurface",
            &value.material.subsurface,
            Some(0.0),
            Some(1.0),
        ),
        ("Clearcoat", &value.material.clearcoat, Some(0.0), Some(1.0)),
        ("Sheen", &value.material.sheen, Some(0.0), Some(1.0)),
        (
            "Transmission",
            &value.material.transmission,
            Some(0.0),
            Some(1.0),
        ),
        (
            "Index of refraction",
            &value.material.ior,
            Some(MIN_IOR as f64),
            Some(MAX_IOR as f64),
        ),
    ] {
        out.append(&number_row(
            label, timeline, id, minimum, maximum, None, context,
        ));
    }
    out.append(&normal_mode(value.material.normal_mode, id, context));
}

fn text_row(value: &Text3dModifier, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        let input = MultilineTextInput::builder(value.text.fallback())
            .min_content_height(86)
            .build();
        return layered::wide_control(
            "Text",
            &value.text,
            input.widget().clone(),
            Vec::new(),
            |_| {},
            |_| {},
        );
    };
    let position = player_state::snapshot(&context.player_state).position;
    let time = crate::video::visual_local_time(&context.project.borrow(), key, position)
        .unwrap_or_default();
    let edit_context = context.detached();
    let commit_project = context.project.clone();
    let input = MultilineTextInput::builder(value.text.value_at(time))
        .min_content_height(86)
        .on_change(move |next| update_text_value(&edit_context, id, next))
        .on_commit(move || {
            shrimply_project::project::commit_edit(&commit_project.borrow(), "edit-3d-text");
        })
        .build();
    let mut body = Vec::new();
    if value
        .text
        .expression
        .as_ref()
        .is_some_and(|expression| expression.enabled)
    {
        let source = value.text.expression_source().map(str::to_string);
        let expression_context = context.detached();
        body.push(crate::rhai_editor::editor(
            source,
            crate::rhai_editor::ExpressionValue::Text,
            move |source| update_text_expression(&expression_context, id, source),
        ));
    }
    let keyframe_context = context.detached();
    let expression_context = context.detached();
    let refresh = context.refresh.clone();
    let expression_refresh = context.refresh.clone();
    layered::wide_control(
        "Text",
        &value.text,
        input.widget().clone(),
        body,
        move |enabled| {
            if toggle_text_keyframes(&keyframe_context, id, enabled) {
                refresh();
            }
        },
        move |enabled| {
            if toggle_text_expression(&expression_context, id, enabled) {
                expression_refresh();
            }
        },
    )
}

fn font_families(value: &Text3dModifier, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    font_selector_list(&value.font_families, move |families| {
        update_text(&context, id, "edit-3d-text-font", move |text| {
            text.font_families = families
        })
    })
}

fn font_style(value: TextFontStyle, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Style",
        value,
        [
            (TextFontStyle::Normal, "Normal"),
            (TextFontStyle::Italic, "Italic"),
            (TextFontStyle::Oblique, "Oblique"),
        ],
        move |value| {
            update_text(&context, id, "edit-3d-text-style", move |text| {
                text.font_style = value
            })
        },
    )
}

fn font_variations(
    value: &Text3dModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let Some(family) = value.font_families.first() else {
        return Vec::new();
    };
    let capabilities = match family {
        FontFamily::GoogleFonts { name } => {
            crate::font_cache::cached_capabilities(name).unwrap_or_default()
        }
        FontFamily::Local { name } => crate::font_cache::local_capabilities(name),
    };
    capabilities
        .axes
        .into_iter()
        .filter(|axis| !matches!(axis.tag.as_str(), "wght" | "ital"))
        .map(|axis| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(
                &gtk::Label::builder()
                    .label(&axis.tag)
                    .halign(gtk::Align::Start)
                    .hexpand(true)
                    .tooltip_text(shrimply_ui_foundation::i18n::text_args(
                        "%{axis} variation axis",
                        &[("axis", axis.tag.clone())],
                    ))
                    .build(),
            );
            let step = ((axis.maximum - axis.minimum).abs() / 100.0).max(0.01);
            let spin =
                gtk::SpinButton::with_range(axis.minimum.into(), axis.maximum.into(), step.into());
            spin.set_digits(2);
            spin.set_width_chars(8);
            spin.set_value(
                value
                    .font_variations
                    .iter()
                    .find(|value| value.axis == axis.tag)
                    .map_or(axis.default, |value| value.value)
                    .into(),
            );
            let context = context.detached();
            spin.connect_value_changed(move |spin| {
                let value = spin.value() as f32;
                let tag = axis.tag.clone();
                update_text(&context, id, "edit-3d-text-variation", move |text| {
                    if let Some(variation) = text
                        .font_variations
                        .iter_mut()
                        .find(|variation| variation.axis == tag)
                    {
                        variation.value = value;
                    } else {
                        text.font_variations
                            .push(FontVariation { axis: tag, value });
                    }
                });
            });
            row.append(&spin);
            row.upcast()
        })
        .collect()
}

fn horizontal_align(
    value: TextHorizontalAlign,
    id: Uuid,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Horizontal alignment",
        value,
        [
            (TextHorizontalAlign::Left, "Left"),
            (TextHorizontalAlign::Center, "Center"),
            (TextHorizontalAlign::Right, "Right"),
            (TextHorizontalAlign::Fill, "Fill"),
        ],
        move |value| {
            update_text(&context, id, "edit-3d-text-align", move |text| {
                text.h_align = value
            })
        },
    )
}

fn vertical_align(value: VerticalAlign, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Vertical alignment",
        value,
        [
            (VerticalAlign::Top, "Top"),
            (VerticalAlign::Middle, "Middle"),
            (VerticalAlign::Bottom, "Bottom"),
        ],
        move |value| {
            update_text(&context, id, "edit-3d-text-align", move |text| {
                text.v_align = value
            })
        },
    )
}

fn direction(value: TextDirection, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Direction",
        value,
        [
            (TextDirection::Horizontal, "Horizontal"),
            (TextDirection::Vertical, "Vertical"),
        ],
        move |value| {
            update_text(&context, id, "edit-3d-text-direction", move |text| {
                text.direction = value
            })
        },
    )
}

fn normal_mode(value: NormalMode, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    enum_selector("Normals", value, move |value| {
        update_text(&context, id, "edit-3d-text-normals", move |text| {
            text.material.normal_mode = value
        })
    })
}

fn number_row(
    label: &str,
    value: &shrimply_core::timeline_value::TimelineValue<f32>,
    id: Uuid,
    minimum: Option<f64>,
    maximum: Option<f64>,
    unit: Option<&'static str>,
    context: &InspectorContext,
) -> gtk::Widget {
    scalar_row(
        label,
        value,
        id,
        ScalarOptions {
            minimum,
            maximum,
            unit,
            rotating: false,
        },
        context,
    )
}

fn update_text_value(context: &InspectorContext, id: Uuid, next: String) -> bool {
    let Some(key) = context.selected_item.clone() else {
        return false;
    };
    let position = player_state::snapshot(&context.player_state).position;
    let mut project = context.project.borrow_mut();
    let Some(time) = project.keyframe_time(&key, position) else {
        return false;
    };
    let Some(text) = text_mut(&mut project, key, id) else {
        return false;
    };
    let changed = match &mut text.text.base {
        shrimply_core::timeline_value::TimelineBase::Const(value) if *value != next => {
            *value = next;
            true
        }
        shrimply_core::timeline_value::TimelineBase::Keyframes(keyframes) => {
            if let Some(keyframe) = keyframes.iter_mut().find(|keyframe| keyframe.time == time) {
                if keyframe.value == next {
                    false
                } else {
                    keyframe.value = next;
                    true
                }
            } else {
                keyframes.push(shrimply_core::timeline_value::TimelineTextKeyframe {
                    id: Uuid::new_v4(),
                    time,
                    value: next,
                    text_interpolation_to_next: Default::default(),
                    interpolation_to_next: Default::default(),
                });
                keyframes.sort_by_key(|keyframe| keyframe.time);
                true
            }
        }
        shrimply_core::timeline_value::TimelineBase::Const(_) => false,
    };
    drop(project);
    if changed {
        player_state::refresh_project(
            &context.player_state,
            ProjectChange {
                video: true,
                ..Default::default()
            },
        );
    }
    changed
}

fn toggle_text_keyframes(context: &InspectorContext, id: Uuid, enabled: bool) -> bool {
    let Some(key) = context.selected_item.clone() else {
        return false;
    };
    let position = player_state::snapshot(&context.player_state).position;
    let mut project = context.project.borrow_mut();
    let Some(evaluation_time) = crate::video::visual_local_time(&project, key.clone(), position)
    else {
        return false;
    };
    let Some(keyframe_time) = project.keyframe_time(&key, position) else {
        return false;
    };
    let Some(text) = text_mut(&mut project, key, id) else {
        return false;
    };
    let current = text.text.value_at(evaluation_time);
    match (&mut text.text.base, enabled) {
        (shrimply_core::timeline_value::TimelineBase::Const(_), false)
        | (shrimply_core::timeline_value::TimelineBase::Keyframes(_), true) => return false,
        (base @ shrimply_core::timeline_value::TimelineBase::Const(_), true) => {
            *base = shrimply_core::timeline_value::TimelineBase::Keyframes(vec![
                shrimply_core::timeline_value::TimelineTextKeyframe {
                    id: Uuid::new_v4(),
                    time: keyframe_time,
                    value: current,
                    text_interpolation_to_next: Default::default(),
                    interpolation_to_next: Default::default(),
                },
            ]);
        }
        (base @ shrimply_core::timeline_value::TimelineBase::Keyframes(_), false) => {
            *base = shrimply_core::timeline_value::TimelineBase::Const(current);
        }
    }
    shrimply_project::project::commit_edit(&project, "3d-text-keyframes");
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    true
}

fn toggle_text_expression(context: &InspectorContext, id: Uuid, enabled: bool) -> bool {
    let Some(key) = context.selected_item.clone() else {
        return false;
    };
    let mut project = context.project.borrow_mut();
    let Some(text) = text_mut(&mut project, key, id) else {
        return false;
    };
    let changed = match &mut text.text.expression {
        Some(expression) if expression.enabled != enabled => {
            expression.enabled = enabled;
            true
        }
        Some(_) => false,
        None if enabled => {
            text.text.expression = Some(shrimply_core::timeline_value::TimelineExpression {
                id: Uuid::new_v4(),
                enabled: true,
                source: "value".to_string(),
            });
            true
        }
        None => false,
    };
    if !changed {
        return false;
    }
    shrimply_project::project::commit_edit(&project, "3d-text-expression");
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    true
}

fn update_text_expression(context: &InspectorContext, id: Uuid, source: String) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(expression) =
        text_mut(&mut project, key, id).and_then(|text| text.text.expression.as_mut())
    else {
        return;
    };
    if expression.source == source {
        return;
    }
    expression.source = source;
    shrimply_project::project::commit_coalesced_edit(&project, "3d-text-expression");
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            ..Default::default()
        },
    );
}

fn update_text(
    context: &InspectorContext,
    id: Uuid,
    commit: &str,
    update: impl FnOnce(&mut Text3dModifier),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(text) = text_mut(&mut project, key, id) else {
        return;
    };
    update(text);
    shrimply_project::project::commit_edit(&project, commit);
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn text_mut(
    project: &mut Project,
    key: crate::InspectedItem,
    id: Uuid,
) -> Option<&mut Text3dModifier> {
    project
        .video_item_mut(&key)?
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Scene3d(effect) => match &mut **effect {
                Scene3dModifierEffect::Text(text) => Some(&mut **text),
                _ => None,
            },
            _ => None,
        })
}
