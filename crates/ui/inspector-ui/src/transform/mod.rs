use shrimply_ui_foundation::tr;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use glam::Vec2;
use gtk::prelude::*;

use crate::InspectedItem as SelectedItem;
use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::timeline_value::*;
use crate::transform_eval::{self, FrameAudioAnalysis, TransformExpressionCache};
use crate::ui::{Number2Picker, NumberPicker, NumberPickerHandle};
use shrimply_project::project::{Project, ResolvedTransform, Time, Transform, VideoItem};

use super::{
    Inspectable, InspectorContext,
    keyframe_editor::{self, KeyframeGraph, KeyframePoint, RawSegment, SpeedSegment},
    section::InspectorSection,
    timeline_value::layered,
};

use crate::timeline_value::{
    scalar::{ScalarAccess, ScalarSpec, ScalarTarget, scalar_control},
    vector::vec2::{VecAccess, VecSpec, VecTarget, vec_control, vec_control_with_lock},
};

mod expressions;
mod keyframes;

use expressions::{scalar_expression_editor, set_expression_enabled, vec2_expression_editor};
use keyframes::{
    scalar_base_editor, scalar_keyframe_body_editor, stored_vec2_value, vec2_base_editor,
    vec2_keyframe_body_editor,
};

const SLOW_TRANSFORM_LOG_THRESHOLD: Duration = Duration::from_millis(25);

#[derive(Clone, Copy)]
pub(super) enum TransformTarget {
    Item,
    PaintStroke,
}

pub(super) fn controls(
    transform: &Transform,
    context: &InspectorContext,
    target: TransformTarget,
) -> Vec<gtk::Widget> {
    match target {
        TransformTarget::Item => transform.controls(context),
        TransformTarget::PaintStroke => paint_stroke_controls(transform, context),
    }
}

fn paint_stroke_controls(transform: &Transform, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    section.add_wide_control(&vec_control(
        "Position",
        &transform.position,
        context,
        paint_vec_target(paint_position, paint_position_mut),
        paint_vec_spec("px", 1.0, 0, None),
    ));
    section.add_wide_control(&vec_control(
        "Anchor",
        &transform.anchor,
        context,
        paint_vec_target(paint_anchor, paint_anchor_mut),
        paint_vec_spec("px", 1.0, 0, None),
    ));
    section.add_wide_control(&vec_control_with_lock(
        "Scale",
        &transform.scale,
        context,
        paint_vec_target(paint_scale, paint_scale_mut),
        paint_vec_spec("x", 0.01, 2, Some(0.0)),
        true,
    ));
    section.add_wide_control(&scalar_control(
        "Rotation",
        &transform.rotation_degrees,
        context,
        ScalarTarget {
            access: ScalarAccess::ItemWithMutation {
                get: paint_rotation,
                get_mut: paint_rotation_mut,
                mutated: crate::paint::bump_revision_for_key,
            },
            scope_id: Some(transform.rotation_degrees.id),
            local_time: crate::video::visual_local_time,
            global_time: crate::video::visual_global_time,
            duration: crate::video::visual_duration,
            refresh: paint_refresh(),
            commit_name: "paint-stroke-transform",
        },
        ScalarSpec {
            drag_step: 0.1,
            digits: 1,
            integer: false,
            width_chars: 9,
            minimum: None,
            maximum: None,
            unit_name: Some("°"),
            rotating_icon: Some(("arrow3-up-symbolic", 0.0)),
            display: f64::from,
            store: |value| value as f32,
            clamp: |value| value,
        },
    ));
    vec![section.into_widget()]
}

fn paint_vec_target(
    get: for<'a> fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<Vec2>>,
    get_mut: for<'a> fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<Vec2>>,
) -> VecTarget {
    VecTarget {
        access: VecAccess::ItemWithMutation {
            get,
            get_mut,
            mutated: crate::paint::bump_revision_for_key,
        },
        scope_id: None,
        local_time: crate::video::visual_local_time,
        global_time: crate::video::visual_global_time,
        duration: crate::video::visual_duration,
        refresh: paint_refresh(),
        commit_name: "paint-stroke-transform",
    }
}

fn paint_vec_spec(
    unit_name: &'static str,
    drag_step: f64,
    digits: usize,
    minimum: Option<f64>,
) -> VecSpec {
    VecSpec {
        first_prefix: "X",
        second_prefix: "Y",
        drag_step,
        digits,
        width_chars: 7,
        minimum,
        maximum: None,
        unit_name,
    }
}

fn paint_refresh() -> ProjectChange {
    ProjectChange {
        video: true,
        inspector: true,
        ..ProjectChange::default()
    }
}

fn paint_position(project: &Project, key: SelectedItem) -> Option<&TimelineValue<Vec2>> {
    Some(
        &crate::paint::selected_paint(project, key)?
            .stroke_transform
            .position,
    )
}

fn paint_position_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<Vec2>> {
    Some(
        &mut crate::paint::selected_paint_mut(project, key)?
            .stroke_transform
            .position,
    )
}

fn paint_anchor(project: &Project, key: SelectedItem) -> Option<&TimelineValue<Vec2>> {
    Some(
        &crate::paint::selected_paint(project, key)?
            .stroke_transform
            .anchor,
    )
}

fn paint_anchor_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TimelineValue<Vec2>> {
    Some(
        &mut crate::paint::selected_paint_mut(project, key)?
            .stroke_transform
            .anchor,
    )
}

fn paint_scale(project: &Project, key: SelectedItem) -> Option<&TimelineValue<Vec2>> {
    Some(
        &crate::paint::selected_paint(project, key)?
            .stroke_transform
            .scale,
    )
}

fn paint_scale_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TimelineValue<Vec2>> {
    Some(
        &mut crate::paint::selected_paint_mut(project, key)?
            .stroke_transform
            .scale,
    )
}

fn paint_rotation(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    Some(
        &crate::paint::selected_paint(project, key)?
            .stroke_transform
            .rotation_degrees,
    )
}

fn paint_rotation_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TimelineValue<f32>> {
    Some(
        &mut crate::paint::selected_paint_mut(project, key)?
            .stroke_transform
            .rotation_degrees,
    )
}

impl Inspectable for Transform {
    fn title(&self) -> &'static str {
        "Transform"
    }

    fn default_action(&self, context: &InspectorContext) -> Option<Box<dyn Fn() + 'static>> {
        let key = context.selected_item.clone()?;
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let refresh = context.refresh.clone();
        Some(Box::new(move || {
            let mut project = project.borrow_mut();
            let canvas_size = project.canvas_size;
            let Some(item) = project.video_item_mut(&key) else {
                return;
            };
            item.transform = item.natural_transform(canvas_size);
            shrimply_project::project::commit_edit(&project, "reset-transform");
            drop(project);
            player_state::refresh_project(
                &player_state,
                ProjectChange {
                    video: true,
                    inspector: true,
                    ..ProjectChange::default()
                },
            );
            refresh();
        }))
    }

    fn add_rows(&self, section: &InspectorSection, context: &InspectorContext) {
        let Some(key) = context.selected_item.clone() else {
            return;
        };
        let display = display_transform(context, key.clone()).unwrap_or_else(|| self.fallback());
        add_vec2_meta_control(
            section,
            "Position",
            context,
            key.clone(),
            Vec2Field::Position,
            display,
        );
        add_vec2_meta_control(
            section,
            "Anchor",
            context,
            key.clone(),
            Vec2Field::Anchor,
            display,
        );
        add_vec2_meta_control(
            section,
            "Scale",
            context,
            key.clone(),
            Vec2Field::Scale,
            display,
        );
        add_vec2_meta_control(
            section,
            "Shear",
            context,
            key.clone(),
            Vec2Field::Shear,
            display,
        );
        add_scalar_meta_control(
            section,
            "Rotation",
            context,
            key.clone(),
            ScalarField::RotationDegrees,
            display,
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum Vec2Field {
    Position,
    Anchor,
    Scale,
    Shear,
}

#[derive(Clone, Copy, Debug)]
enum ScalarField {
    RotationDegrees,
}

#[derive(Clone, Copy, Debug)]
enum TransformField {
    Vec2(Vec2Field),
    Scalar(ScalarField),
}

fn add_vec2_meta_control(
    section: &InspectorSection,
    label: &str,
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
    display: ResolvedTransform,
) {
    let current = current_vec2(context, key.clone(), field)
        .unwrap_or_else(|| TimelineValue::<glam::Vec2>::new_const(Vec2::ZERO));
    let keyframes_enabled = vec2_keyframes_enabled(&current);
    let expression_enabled = expression_enabled(&current.expression);
    let base_display = base_display_transform(context, key.clone()).unwrap_or(display);
    let editor = vec2_base_editor(
        context,
        key.clone(),
        field,
        base_display,
        keyframes_enabled,
        keyframes_enabled || expression_enabled,
    );
    let mut body = Vec::new();
    if keyframes_enabled {
        body.push(vec2_keyframe_body_editor(context, key.clone(), field));
    }
    if expression_enabled {
        body.push(vec2_expression_editor(context, key.clone(), field));
        body.push(vec2_expression_output(context, key.clone(), field));
    }
    section.add_wide_control(&layered_control(
        context,
        key.clone(),
        LayeredControlInput {
            label,
            value: &current,
            editor,
            field: TransformField::Vec2(field),
            body,
        },
    ));
}

fn add_scalar_meta_control(
    section: &InspectorSection,
    label: &str,
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
    display: ResolvedTransform,
) {
    let current = current_scalar(context, key.clone(), field)
        .unwrap_or_else(|| TimelineValue::<f32>::new_const(0.0));
    let keyframes_enabled = scalar_keyframes_enabled(&current);
    let expression_enabled = expression_enabled(&current.expression);
    let base_display = base_display_transform(context, key.clone()).unwrap_or(display);
    let editor = scalar_base_editor(
        context,
        key.clone(),
        field,
        base_display,
        keyframes_enabled,
        keyframes_enabled || expression_enabled,
    );
    let mut body = Vec::new();
    if keyframes_enabled {
        body.push(scalar_keyframe_body_editor(
            context,
            key.clone(),
            field,
            base_display,
        ));
    }
    if expression_enabled {
        body.push(scalar_expression_editor(context, key.clone(), field));
        body.push(scalar_expression_output(context, key.clone(), field));
    }
    section.add_wide_control(&layered_control(
        context,
        key.clone(),
        LayeredControlInput {
            label,
            value: &current,
            editor,
            field: TransformField::Scalar(field),
            body,
        },
    ));
}

struct LayeredControlInput<'a, T: TimelineValueType> {
    label: &'a str,
    value: &'a TimelineValue<T>,
    editor: gtk::Widget,
    field: TransformField,
    body: Vec<gtk::Widget>,
}

fn layered_control<T: TimelineValueType>(
    context: &InspectorContext,
    key: SelectedItem,
    input: LayeredControlInput<'_, T>,
) -> gtk::Widget {
    let LayeredControlInput {
        label,
        value,
        editor,
        field,
        body,
    } = input;
    let keyframe_project = context.project.clone();
    let keyframe_player_state = context.player_state.clone();
    let keyframe_refresh = context.refresh.clone();
    let expression_project = context.project.clone();
    let expression_player_state = context.player_state.clone();
    let expression_refresh = context.refresh.clone();
    let keyframe_key = key.clone();
    let expression_key = key;
    layered::control(
        label,
        value,
        editor,
        body,
        move |enabled| {
            if set_keyframes_enabled(
                &keyframe_project,
                &keyframe_player_state,
                keyframe_key.clone(),
                field,
                enabled,
            ) {
                keyframe_refresh();
            }
        },
        move |enabled| {
            if set_expression_enabled(
                &expression_project,
                &expression_player_state,
                expression_key.clone(),
                field,
                enabled,
            ) {
                expression_refresh();
            }
        },
    )
}

fn vec2_const_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
    display: ResolvedTransform,
    live_display: bool,
) -> gtk::Widget {
    let value = display_vec2_value(display, field);
    let mut picker = Number2Picker::builder(value.x as f64, value.y as f64)
        .first_prefix("X")
        .second_prefix("Y")
        .drag_step(if matches!(field, Vec2Field::Scale | Vec2Field::Shear) {
            0.01
        } else {
            1.0
        })
        .digits(if matches!(field, Vec2Field::Scale | Vec2Field::Shear) {
            2
        } else {
            0
        })
        .unit_name(vec2_unit_name(field))
        .width_chars(7);
    if matches!(field, Vec2Field::Scale) {
        picker = picker.minimum(0.0).enable_lock();
    }

    let first_project = context.project.clone();
    let first_player_state = context.player_state.clone();
    let second_project = context.project.clone();
    let second_player_state = context.player_state.clone();
    let first_commit_project = context.project.clone();
    let second_commit_project = context.project.clone();
    let first_key = key.clone();
    let second_key = key.clone();
    let parts = picker
        .on_first_change(move |next| {
            update_vec2_const(
                &first_project,
                &first_player_state,
                first_key.clone(),
                field,
                0,
                next,
            );
        })
        .on_second_change(move |next| {
            update_vec2_const(
                &second_project,
                &second_player_state,
                second_key.clone(),
                field,
                1,
                next,
            );
        })
        .on_first_commit(move |_| {
            shrimply_project::project::commit_edit(
                &first_commit_project.borrow(),
                "video-transform",
            );
        })
        .on_second_commit(move |_| {
            shrimply_project::project::commit_edit(
                &second_commit_project.borrow(),
                "video-transform",
            );
        })
        .build_with_handles();
    let widget = parts.widget;
    if live_display {
        connect_vec2_base_display(context, key.clone(), field, parts.first, parts.second);
    }
    widget
}

fn scalar_const_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
    display: ResolvedTransform,
    live_display: bool,
) -> gtk::Widget {
    let picker = NumberPicker::builder(display_scalar_value(display, field))
        .drag_step(scalar_drag_step(field))
        .digits(scalar_digits(field))
        .width_chars(9)
        .unit_name(scalar_unit_name(field))
        .rotating_prefix_icon_name("arrow3-up-symbolic");

    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let commit_project = context.project.clone();
    let update_key = key.clone();
    let parts = picker
        .on_change(move |next| {
            update_scalar_const(
                &project,
                &player_state,
                update_key.clone(),
                field,
                stored_scalar_value(field, next),
            );
        })
        .on_commit(move |_| {
            shrimply_project::project::commit_edit(&commit_project.borrow(), "video-transform");
        })
        .build_with_handle();
    let widget = parts.widget;
    if live_display {
        connect_scalar_base_display(context, key.clone(), field, parts.handle);
    }
    widget
}

fn update_vec2_const(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: Vec2Field,
    component: usize,
    value: f64,
) {
    let started = Instant::now();
    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return;
    };
    let mut next = vec2_field(transform, field).fallback();
    let display_value = value;
    let value = stored_vec2_component(field, value) as f32;
    if component == 0 {
        next.x = value;
    } else {
        next.y = value;
    }
    if matches!(field, Vec2Field::Scale) {
        next = next.max(Vec2::ZERO);
    }
    vec2_field_mut(transform, field).base = TimelineBase::Const(next);
    drop(project);
    let refresh_started = Instant::now();
    refresh_video(player_state);
    let refresh_elapsed = refresh_started.elapsed();
    let elapsed = started.elapsed();
    if refresh_elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD || elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD {
        tracing::debug!(
            "transform: update_vec2_const field={field:?} component={component} display_value={display_value:.6} stored_value={value:.6} refresh_elapsed_us={} total_elapsed_us={}",
            refresh_elapsed.as_micros(),
            elapsed.as_micros(),
        );
    }
}

fn update_scalar_const(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: ScalarField,
    value: f64,
) {
    let started = Instant::now();
    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return;
    };
    let stored_value = clamp_scalar_value(field, value);
    scalar_field_mut(transform, field).base = TimelineBase::Const(stored_value as f32);
    drop(project);
    let refresh_started = Instant::now();
    refresh_video(player_state);
    let refresh_elapsed = refresh_started.elapsed();
    let elapsed = started.elapsed();
    if refresh_elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD || elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD {
        tracing::debug!(
            "transform: update_scalar_const field={field:?} display_value={value:.6} stored_value={stored_value:.6} refresh_elapsed_us={} total_elapsed_us={}",
            refresh_elapsed.as_micros(),
            elapsed.as_micros(),
        );
    }
}

fn set_keyframes_enabled(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: TransformField,
    enabled: bool,
) -> bool {
    let position = player_state::snapshot(player_state).position;
    let (current, local_time) = {
        let project_ref = project.borrow();
        let Some(position) = project_ref.timeline_time_to_sequence(&key.track(), position) else {
            return false;
        };
        let Some(item) = project_ref.video_item(&key) else {
            return false;
        };
        if field_keyframes_enabled(&item.transform, field) == enabled {
            return false;
        }
        let current = transform_eval::resolve_item_base_transform(&project_ref, item, position);
        let Some(local_time) = shrimply_project::project::generated_item_time(item, position)
        else {
            return false;
        };
        (current, local_time)
    };
    let mut project_ref = project.borrow_mut();
    let Some(item) = project_ref.video_item_mut(&key) else {
        return false;
    };

    match field {
        TransformField::Vec2(field) => {
            let mut current = stored_vec2_value(field, display_vec2_value(current, field));
            if matches!(field, Vec2Field::Scale) {
                current = current.max(Vec2::ZERO);
            }
            vec2_field_mut(&mut item.transform, field).base = if enabled {
                TimelineBase::Keyframes(vec![TimelineVectorKeyframe::<glam::Vec2> {
                    id: uuid::Uuid::new_v4(),
                    time: local_time,
                    value: current,
                    interpolation_to_next: Default::default(),
                }])
            } else {
                TimelineBase::Const(current)
            };
        }
        TransformField::Scalar(field) => {
            let current =
                stored_display_scalar_value(field, display_scalar_value(current, field)) as f32;
            scalar_field_mut(&mut item.transform, field).base = if enabled {
                TimelineBase::Keyframes(vec![TimelineScalarKeyframe::<f32> {
                    id: uuid::Uuid::new_v4(),
                    time: local_time,
                    value: current,
                    interpolation_to_next: Default::default(),
                }])
            } else {
                TimelineBase::Const(current)
            };
        }
    }
    shrimply_project::project::commit_edit(&project_ref, "video-transform-keyframes");
    drop(project_ref);
    player_state::refresh_project(
        player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
    true
}

fn display_transform(context: &InspectorContext, key: SelectedItem) -> Option<ResolvedTransform> {
    let position = player_state::snapshot(&context.player_state).position;
    let audio_analysis = context.audio_analysis_at(position);
    evaluated_transform(
        &context.project.borrow(),
        key.clone(),
        position,
        &audio_analysis,
    )
}

fn base_display_transform(
    context: &InspectorContext,
    key: SelectedItem,
) -> Option<ResolvedTransform> {
    let position = player_state::snapshot(&context.player_state).position;
    base_transform_at(&context.project.borrow(), key.clone(), position)
}

fn evaluated_transform(
    project: &Project,
    key: SelectedItem,
    position: Time,
    audio_analysis: &FrameAudioAnalysis,
) -> Option<ResolvedTransform> {
    let position = project.timeline_time_to_sequence(&key.track(), position)?;
    let item = project.video_item(&key)?;
    let mut cache = TransformExpressionCache::default();
    Some(transform_eval::resolve_item_transform_with_audio(
        project,
        item,
        position,
        audio_analysis,
        &mut cache,
    ))
}

fn base_transform_at(
    project: &Project,
    key: SelectedItem,
    position: Time,
) -> Option<ResolvedTransform> {
    let position = project.timeline_time_to_sequence(&key.track(), position)?;
    let item = project.video_item(&key)?;
    Some(transform_eval::resolve_item_base_transform(
        project, item, position,
    ))
}

fn vec2_expression_result_at(
    project: &Project,
    key: SelectedItem,
    field: Vec2Field,
    position: Time,
    audio_analysis: &FrameAudioAnalysis,
    cache: &Rc<RefCell<TransformExpressionCache>>,
) -> Option<Result<Vec2, String>> {
    let position = project.timeline_time_to_sequence(&key.track(), position)?;
    let item = project.video_item(&key)?;
    let value = vec2_field(&item.transform, field);
    let expression = value.expression.as_ref()?;
    if !expression.enabled || expression.source.trim().is_empty() {
        return None;
    }
    let eval = transform_eval::TransformEvaluation::for_item_with_audio(
        project,
        item,
        position,
        audio_analysis,
    );
    let base = transform_eval::resolve_vec2_base(value, &eval);
    Some(
        cache
            .borrow_mut()
            .eval_timeline_value_result(&eval, value.id, &expression.source, &base),
    )
}

fn scalar_expression_result_at(
    project: &Project,
    key: SelectedItem,
    field: ScalarField,
    position: Time,
    audio_analysis: &FrameAudioAnalysis,
    cache: &Rc<RefCell<TransformExpressionCache>>,
) -> Option<Result<f32, String>> {
    let position = project.timeline_time_to_sequence(&key.track(), position)?;
    let item = project.video_item(&key)?;
    let value = scalar_field(&item.transform, field);
    let expression = value.expression.as_ref()?;
    if !expression.enabled || expression.source.trim().is_empty() {
        return None;
    }
    let eval = transform_eval::TransformEvaluation::for_item_with_audio(
        project,
        item,
        position,
        audio_analysis,
    );
    let base = transform_eval::resolve_scalar_base(value, &eval);
    Some(
        cache
            .borrow_mut()
            .eval_timeline_value_result(&eval, value.id, &expression.source, &base),
    )
}

fn selected_item_label(key: SelectedItem) -> String {
    format!("{:?}#{}:{}", key.kind(), key.track_id(), key.item_id())
}

fn connect_vec2_base_display(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
    first: NumberPickerHandle,
    second: NumberPickerHandle,
) {
    update_vec2_base_display(
        &context.project,
        &context.player_state,
        key.clone(),
        field,
        &first,
        &second,
    );
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let alive = Rc::downgrade(&context.listener_scope);
    let alive_for_prune = alive.clone();
    let first = first.downgrade();
    let second = second.downgrade();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector transform vec2 base display",
        move || alive_for_prune.upgrade().is_some(),
        move |event| {
            if !transform_display_refresh_event(event) {
                return;
            }
            shrimply_support::crash::set_context(format!(
                "inspector transform vec2 base display schedule key={} field={field:?} event={event:?}",
                selected_item_label(key.clone()),
            ));
            if alive.upgrade().is_none() {
                tracing::trace!(
                    "transform: skip vec2 base display key={} field={field:?} reason=stale-scope",
                    selected_item_label(key.clone()),
                );
                return;
            }
            let (Some(first), Some(second)) = (first.upgrade(), second.upgrade()) else {
                tracing::trace!(
                    "transform: skip vec2 base display key={} field={field:?} reason=widget-dropped",
                    selected_item_label(key.clone()),
                );
                return;
            };
            update_vec2_base_display(&project, &player_state, key.clone(), field, &first, &second);
        },
    );
}

fn connect_scalar_base_display(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
    handle: NumberPickerHandle,
) {
    update_scalar_base_display(
        &context.project,
        &context.player_state,
        key.clone(),
        field,
        &handle,
    );
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let alive = Rc::downgrade(&context.listener_scope);
    let alive_for_prune = alive.clone();
    let handle = handle.downgrade();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector transform scalar base display",
        move || alive_for_prune.upgrade().is_some(),
        move |event| {
            if !transform_display_refresh_event(event) {
                return;
            }
            shrimply_support::crash::set_context(format!(
                "inspector transform scalar base display schedule key={} field={field:?} event={event:?}",
                selected_item_label(key.clone()),
            ));
            if alive.upgrade().is_none() {
                tracing::trace!(
                    "transform: skip scalar base display key={} field={field:?} reason=stale-scope",
                    selected_item_label(key.clone()),
                );
                return;
            }
            let Some(handle) = handle.upgrade() else {
                tracing::trace!(
                    "transform: skip scalar base display key={} field={field:?} reason=widget-dropped",
                    selected_item_label(key.clone()),
                );
                return;
            };
            update_scalar_base_display(&project, &player_state, key.clone(), field, &handle);
        },
    );
}

fn update_vec2_base_display(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: Vec2Field,
    first: &NumberPickerHandle,
    second: &NumberPickerHandle,
) {
    let started = Instant::now();
    let position = player_state::snapshot(player_state).position;
    shrimply_support::crash::set_context(format!(
        "inspector transform vec2 base display lookup key={} field={field:?} position={}",
        selected_item_label(key.clone()),
        position.as_label(),
    ));
    let value = {
        let project = project.borrow();
        let Some(position) = project.timeline_time_to_sequence(&key.track(), position) else {
            return;
        };
        let Some(item) = project.video_item(&key) else {
            tracing::trace!(
                "transform: skip vec2 base display key={} field={field:?} position={} reason=missing-item",
                selected_item_label(key.clone()),
                position.as_label(),
            );
            return;
        };
        let eval = transform_eval::TransformEvaluation::for_item(&project, item, position);
        transform_eval::resolve_vec2_base(vec2_field(&item.transform, field), &eval)
    };
    let value = display_vec2_raw_value(field, value);
    if !value.x.is_finite() || !value.y.is_finite() {
        tracing::debug!(
            "transform: skip vec2 base display key={} field={field:?} position={} reason=non-finite x={} y={}",
            selected_item_label(key.clone()),
            position.as_label(),
            value.x,
            value.y,
        );
        return;
    }
    shrimply_support::crash::set_context(format!(
        "inspector transform vec2 base display set first key={} field={field:?} value={}",
        selected_item_label(key.clone()),
        value.x,
    ));
    first.set_f64(value.x as f64);
    shrimply_support::crash::set_context(format!(
        "inspector transform vec2 base display set second key={} field={field:?} value={}",
        selected_item_label(key.clone()),
        value.y,
    ));
    second.set_f64(value.y as f64);
    let elapsed = started.elapsed();
    if elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD {
        tracing::debug!(
            "transform: vec2 base display key={} field={field:?} position={} x={} y={} elapsed_us={}",
            selected_item_label(key.clone()),
            position.as_label(),
            value.x,
            value.y,
            elapsed.as_micros(),
        );
    }
}

fn update_scalar_base_display(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: ScalarField,
    handle: &NumberPickerHandle,
) {
    let started = Instant::now();
    let position = player_state::snapshot(player_state).position;
    shrimply_support::crash::set_context(format!(
        "inspector transform scalar base display lookup key={} field={field:?} position={}",
        selected_item_label(key.clone()),
        position.as_label(),
    ));
    let value = {
        let project = project.borrow();
        let Some(position) = project.timeline_time_to_sequence(&key.track(), position) else {
            return;
        };
        let Some(item) = project.video_item(&key) else {
            tracing::trace!(
                "transform: skip scalar base display key={} field={field:?} position={} reason=missing-item",
                selected_item_label(key.clone()),
                position.as_label(),
            );
            return;
        };
        let eval = transform_eval::TransformEvaluation::for_item(&project, item, position);
        transform_eval::resolve_scalar_base(scalar_field(&item.transform, field), &eval)
    };
    let value = display_scalar_raw_value(field, value);
    if !value.is_finite() {
        tracing::debug!(
            "transform: skip scalar base display key={} field={field:?} position={} reason=non-finite value={}",
            selected_item_label(key.clone()),
            position.as_label(),
            value,
        );
        return;
    }
    shrimply_support::crash::set_context(format!(
        "inspector transform scalar base display set key={} field={field:?} value={}",
        selected_item_label(key.clone()),
        value,
    ));
    handle.set_f64(value);
    let elapsed = started.elapsed();
    if elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD {
        tracing::debug!(
            "transform: scalar base display key={} field={field:?} position={} value={} elapsed_us={}",
            selected_item_label(key.clone()),
            position.as_label(),
            value,
            elapsed.as_micros(),
        );
    }
}

fn vec2_expression_output(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
) -> gtk::Widget {
    let label = expression_output_value_label();
    let widget = expression_output_row(label.clone());
    let cache = Rc::new(RefCell::new(TransformExpressionCache::default()));
    update_vec2_expression_output(
        &context.project,
        &context.player_state,
        &context.volume,
        key.clone(),
        field,
        &label,
        &cache,
    );
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let volume = context.volume.clone();
    let alive = Rc::downgrade(&context.listener_scope);
    let label = label.downgrade();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector transform vec2 expression output",
        move || alive.upgrade().is_some(),
        move |event| {
            if !transform_display_refresh_event(event) {
                return;
            }
            let Some(label) = label.upgrade() else {
                return;
            };
            update_vec2_expression_output(
                &project,
                &player_state,
                &volume,
                key.clone(),
                field,
                &label,
                &cache,
            );
        },
    );
    widget
}

fn scalar_expression_output(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
) -> gtk::Widget {
    let label = expression_output_value_label();
    let widget = expression_output_row(label.clone());
    let cache = Rc::new(RefCell::new(TransformExpressionCache::default()));
    update_scalar_expression_output(
        &context.project,
        &context.player_state,
        &context.volume,
        key.clone(),
        field,
        &label,
        &cache,
    );
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let volume = context.volume.clone();
    let alive = Rc::downgrade(&context.listener_scope);
    let label = label.downgrade();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector transform scalar expression output",
        move || alive.upgrade().is_some(),
        move |event| {
            if !transform_display_refresh_event(event) {
                return;
            }
            let Some(label) = label.upgrade() else {
                return;
            };
            update_scalar_expression_output(
                &project,
                &player_state,
                &volume,
                key.clone(),
                field,
                &label,
                &cache,
            );
        },
    );
    widget
}

fn expression_output_row(value: gtk::Label) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_hexpand(true);
    let label = gtk::Label::new(Some(tr!("Output").as_ref()));
    label.add_css_class("dim-label");
    label.set_xalign(0.0);
    row.append(&label);
    row.append(&value);
    row.upcast()
}

fn expression_output_value_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_hexpand(true);
    label.set_xalign(1.0);
    label.set_selectable(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.add_css_class("numeric");
    label
}

fn update_vec2_expression_output(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    volume: &Rc<RefCell<shrimply_audio::streaming::FrameAudioSampler>>,
    key: SelectedItem,
    field: Vec2Field,
    label: &gtk::Label,
    cache: &Rc<RefCell<TransformExpressionCache>>,
) {
    let started = Instant::now();
    let snapshot = player_state::snapshot(player_state);
    let position = snapshot.position;
    let volume_mixer = volume
        .borrow_mut()
        .sample(&project.borrow(), position, snapshot.revision);
    let outcome = match vec2_expression_result_at(
        &project.borrow(),
        key.clone(),
        field,
        position,
        &volume_mixer,
        cache,
    ) {
        Some(Ok(value)) => {
            set_expression_output_value(
                label,
                &format_vec2_output(field, display_vec2_raw_value(field, value)),
            );
            "ok".to_string()
        }
        Some(Err(error)) => {
            set_expression_output_error(label, &error);
            format!("error: {}", expression_error_summary(&error))
        }
        None => "none".to_string(),
    };
    let elapsed = started.elapsed();
    if (outcome != "ok" && outcome != "none") || elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD {
        tracing::debug!(
            "transform: update_vec2_expression_output field={field:?} outcome={outcome} elapsed_us={}",
            elapsed.as_micros(),
        );
    }
}

fn update_scalar_expression_output(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    volume: &Rc<RefCell<shrimply_audio::streaming::FrameAudioSampler>>,
    key: SelectedItem,
    field: ScalarField,
    label: &gtk::Label,
    cache: &Rc<RefCell<TransformExpressionCache>>,
) {
    let started = Instant::now();
    let snapshot = player_state::snapshot(player_state);
    let position = snapshot.position;
    let volume_mixer = volume
        .borrow_mut()
        .sample(&project.borrow(), position, snapshot.revision);
    let outcome = match scalar_expression_result_at(
        &project.borrow(),
        key.clone(),
        field,
        position,
        &volume_mixer,
        cache,
    ) {
        Some(Ok(value)) => {
            set_expression_output_value(
                label,
                &format_scalar_output(field, display_scalar_raw_value(field, value)),
            );
            "ok".to_string()
        }
        Some(Err(error)) => {
            set_expression_output_error(label, &error);
            format!("error: {}", expression_error_summary(&error))
        }
        None => "none".to_string(),
    };
    let elapsed = started.elapsed();
    if (outcome != "ok" && outcome != "none") || elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD {
        tracing::debug!(
            "transform: update_scalar_expression_output field={field:?} outcome={outcome} elapsed_us={}",
            elapsed.as_micros(),
        );
    }
}

fn set_expression_output_value(label: &gtk::Label, value: &str) {
    label.remove_css_class("error");
    label.set_tooltip_text(None);
    label.set_text(value);
}

fn set_expression_output_error(label: &gtk::Label, error: &str) {
    label.add_css_class("error");
    label.set_tooltip_text(Some(error));
    label.set_text(&expression_error_summary(error));
}

fn expression_error_summary(error: &str) -> String {
    let message = error.lines().next().unwrap_or_default().trim();
    if message.is_empty() {
        "Invalid expression".to_string()
    } else {
        format!("Invalid expression: {message}")
    }
}

fn transform_display_refresh_event(event: player_state::PlayerEvent) -> bool {
    match event {
        player_state::PlayerEvent::State => true,
        player_state::PlayerEvent::Project(change) => change.video || change.audio,
    }
}

fn format_vec2_output(field: Vec2Field, value: Vec2) -> String {
    let unit = vec2_unit_name(field);
    let x = format_display_number(value.x as f64, 0);
    let y = format_display_number(value.y as f64, 0);
    format!("X {x}{unit}  Y {y}{unit}")
}

fn format_scalar_output(field: ScalarField, value: f64) -> String {
    format!(
        "{}{}",
        format_display_number(value, scalar_digits(field)),
        scalar_unit_name(field)
    )
}

fn format_display_number(value: f64, digits: usize) -> String {
    let value = if value.abs() < 10.0_f64.powi(-(digits as i32)) / 2.0 {
        0.0
    } else {
        value
    };
    format!("{value:.digits$}")
}

fn current_vec2(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
) -> Option<TimelineValue<glam::Vec2>> {
    let project = context.project.borrow();
    let transform = &project.video_item(&key)?.transform;
    Some(vec2_field(transform, field).clone())
}

fn current_scalar(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
) -> Option<TimelineValue<f32>> {
    let project = context.project.borrow();
    let transform = &project.video_item(&key)?.transform;
    Some(scalar_field(transform, field).clone())
}

fn selected_transform_mut(project: &mut Project, key: SelectedItem) -> Option<&mut Transform> {
    project.video_item_mut(&key).map(|item| &mut item.transform)
}

fn vec2_field(transform: &Transform, field: Vec2Field) -> &TimelineValue<glam::Vec2> {
    match field {
        Vec2Field::Position => &transform.position,
        Vec2Field::Anchor => &transform.anchor,
        Vec2Field::Scale => &transform.scale,
        Vec2Field::Shear => &transform.shear,
    }
}

fn vec2_field_mut(transform: &mut Transform, field: Vec2Field) -> &mut TimelineValue<glam::Vec2> {
    match field {
        Vec2Field::Position => &mut transform.position,
        Vec2Field::Anchor => &mut transform.anchor,
        Vec2Field::Scale => &mut transform.scale,
        Vec2Field::Shear => &mut transform.shear,
    }
}

fn scalar_field(transform: &Transform, field: ScalarField) -> &TimelineValue<f32> {
    match field {
        ScalarField::RotationDegrees => &transform.rotation_degrees,
    }
}

fn scalar_field_mut(transform: &mut Transform, field: ScalarField) -> &mut TimelineValue<f32> {
    match field {
        ScalarField::RotationDegrees => &mut transform.rotation_degrees,
    }
}

fn vec2_keyframes_enabled(value: &TimelineValue<glam::Vec2>) -> bool {
    matches!(value.base, TimelineBase::Keyframes(_))
}

fn scalar_keyframes_enabled(value: &TimelineValue<f32>) -> bool {
    matches!(value.base, TimelineBase::Keyframes(_))
}

fn field_keyframes_enabled(transform: &Transform, field: TransformField) -> bool {
    match field {
        TransformField::Vec2(field) => vec2_keyframes_enabled(vec2_field(transform, field)),
        TransformField::Scalar(field) => scalar_keyframes_enabled(scalar_field(transform, field)),
    }
}

fn expression_enabled<T>(expression: &Option<T>) -> bool
where
    T: ExpressionState,
{
    expression.as_ref().is_some_and(ExpressionState::enabled)
}

trait ExpressionState {
    fn enabled(&self) -> bool;
}

impl ExpressionState for TimelineExpression {
    fn enabled(&self) -> bool {
        self.enabled
    }
}

fn vec2_keyframes_mut(
    transform: &mut Transform,
    field: Vec2Field,
) -> Option<&mut Vec<TimelineVectorKeyframe<glam::Vec2>>> {
    match &mut vec2_field_mut(transform, field).base {
        TimelineBase::Keyframes(keyframes) => Some(keyframes),
        TimelineBase::Const(_) => None,
    }
}

fn scalar_keyframes_mut(
    transform: &mut Transform,
    field: ScalarField,
) -> Option<&mut Vec<TimelineScalarKeyframe<f32>>> {
    match &mut scalar_field_mut(transform, field).base {
        TimelineBase::Keyframes(keyframes) => Some(keyframes),
        TimelineBase::Const(_) => None,
    }
}

fn display_vec2_value(transform: ResolvedTransform, field: Vec2Field) -> Vec2 {
    let value = match field {
        Vec2Field::Position => transform.position,
        Vec2Field::Anchor => transform.anchor,
        Vec2Field::Scale => transform.scale,
        Vec2Field::Shear => transform.shear,
    };
    display_vec2_raw_value(field, value)
}

fn display_vec2_raw_value(field: Vec2Field, value: Vec2) -> Vec2 {
    match field {
        Vec2Field::Position | Vec2Field::Anchor | Vec2Field::Scale | Vec2Field::Shear => value,
    }
}

fn display_scalar_value(transform: ResolvedTransform, field: ScalarField) -> f64 {
    let value = match field {
        ScalarField::RotationDegrees => transform.rotation_degrees,
    } as f64;
    display_scalar_raw_value(field, value as f32)
}

fn display_scalar_raw_value(field: ScalarField, value: f32) -> f64 {
    display_scalar_keyframe_value(field, value)
}

fn display_scalar_keyframe_value(field: ScalarField, value: f32) -> f64 {
    let value = value as f64;
    match field {
        ScalarField::RotationDegrees => value,
    }
}

fn stored_vec2_component(field: Vec2Field, value: f64) -> f64 {
    match field {
        Vec2Field::Position | Vec2Field::Anchor | Vec2Field::Scale | Vec2Field::Shear => value,
    }
}

fn stored_scalar_value(field: ScalarField, value: f64) -> f64 {
    match field {
        ScalarField::RotationDegrees => value,
    }
}

fn stored_display_scalar_value(field: ScalarField, value: f64) -> f64 {
    clamp_scalar_value(field, stored_scalar_value(field, value))
}

fn clamp_scalar_value(field: ScalarField, value: f64) -> f64 {
    match field {
        ScalarField::RotationDegrees => value,
    }
}

fn vec2_unit_name(field: Vec2Field) -> &'static str {
    match field {
        Vec2Field::Position | Vec2Field::Anchor => "px",
        Vec2Field::Scale => "x",
        Vec2Field::Shear => "",
    }
}

fn scalar_unit_name(field: ScalarField) -> &'static str {
    match field {
        ScalarField::RotationDegrees => "°",
    }
}

fn scalar_drag_step(field: ScalarField) -> f64 {
    match field {
        ScalarField::RotationDegrees => 0.1,
    }
}

fn scalar_digits(field: ScalarField) -> usize {
    match field {
        ScalarField::RotationDegrees => 1,
    }
}

fn clamp_scalar_keyframe_value(field: ScalarField, value: f64) -> f64 {
    match field {
        ScalarField::RotationDegrees => value,
    }
}

fn refresh_video(player_state: &SharedPlayerState) {
    let started = Instant::now();
    player_state::refresh_project(
        player_state,
        ProjectChange {
            video: true,
            live_preview: true,
            ..ProjectChange::default()
        },
    );
    let elapsed = started.elapsed();
    if elapsed >= SLOW_TRANSFORM_LOG_THRESHOLD {
        tracing::debug!(
            "transform: refresh_video elapsed_us={}",
            elapsed.as_micros()
        );
    }
}
