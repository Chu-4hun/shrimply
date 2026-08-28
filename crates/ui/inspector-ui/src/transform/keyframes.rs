use super::*;
use crate::keyframe_model;

struct TransformKeyframeEditorInput {
    value_editor: gtk::Widget,
    graph_data: KeyframeGraph,
    refresh_graph: Rc<dyn Fn() -> KeyframeGraph>,
    visible_area: (Time, Time),
    view_state_scope: &'static str,
    add_at_time: Rc<dyn Fn(Time)>,
    delete_at_time: Rc<dyn Fn(Time)>,
    update_point: Rc<dyn Fn(Time, Time, f64)>,
    copy_keyframes: keyframe_editor::CopyKeyframes,
    paste_keyframes: keyframe_editor::PasteKeyframes,
    set_interpolation: Rc<dyn Fn(uuid::Uuid, Interpolation)>,
}

pub(super) fn vec2_base_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
    display: ResolvedTransform,
    keyframes_enabled: bool,
    live_display: bool,
) -> gtk::Widget {
    if keyframes_enabled {
        vec2_keyframe_value_editor(context, key.clone(), field, display)
    } else {
        vec2_const_editor(context, key.clone(), field, display, live_display)
    }
}

pub(super) fn scalar_base_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
    display: ResolvedTransform,
    keyframes_enabled: bool,
    live_display: bool,
) -> gtk::Widget {
    if keyframes_enabled {
        scalar_keyframe_value_editor(context, key.clone(), field, display)
    } else {
        scalar_const_editor(context, key.clone(), field, display, live_display)
    }
}

fn vec2_keyframe_value_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
    display: ResolvedTransform,
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
    let first_commit_player_state = context.player_state.clone();
    let second_commit_project = context.project.clone();
    let second_commit_player_state = context.player_state.clone();
    let first_key = key.clone();
    let second_key = key.clone();
    let parts = picker
        .on_first_change(move |next| {
            update_vec2_keyframe(
                &first_project,
                &first_player_state,
                first_key.clone(),
                field,
                0,
                next,
            );
        })
        .on_second_change(move |next| {
            update_vec2_keyframe(
                &second_project,
                &second_player_state,
                second_key.clone(),
                field,
                1,
                next,
            );
        })
        .on_first_commit(move |_| {
            commit_keyframe_edit(&first_commit_project, &first_commit_player_state);
        })
        .on_second_commit(move |_| {
            commit_keyframe_edit(&second_commit_project, &second_commit_player_state);
        })
        .build_with_handles();
    let widget = parts.widget;
    connect_vec2_base_display(context, key.clone(), field, parts.first, parts.second);
    widget
}

pub(super) fn vec2_keyframe_body_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
) -> gtk::Widget {
    let graph_project = context.project.clone();
    let graph_key = key.clone();
    build_transform_keyframe_editor(
        context,
        TransformKeyframeEditorInput {
            value_editor: keyframe_body_placeholder(),
            graph_data: vec2_speed_graph(&context.project.borrow(), key.clone(), field),
            refresh_graph: Rc::new(move || {
                vec2_speed_graph(&graph_project.borrow(), graph_key.clone(), field)
            }),
            visible_area: selected_video_visible_area(context, key.clone()),
            view_state_scope: transform_graph_view_scope(TransformField::Vec2(field)),
            add_at_time: add_time_callback(context, key.clone(), TransformField::Vec2(field)),
            delete_at_time: delete_time_callback(context, key.clone(), TransformField::Vec2(field)),
            update_point: point_callback(context, key.clone(), TransformField::Vec2(field)),
            copy_keyframes: copy_keyframes_callback(
                context,
                key.clone(),
                TransformField::Vec2(field),
            ),
            paste_keyframes: paste_keyframes_callback(
                context,
                key.clone(),
                TransformField::Vec2(field),
            ),
            set_interpolation: interpolation_callback(
                context,
                key.clone(),
                TransformField::Vec2(field),
            ),
        },
    )
}

fn scalar_keyframe_value_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
    display: ResolvedTransform,
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
    let commit_player_state = context.player_state.clone();
    let update_key = key.clone();
    let parts = picker
        .on_change(move |next| {
            update_scalar_keyframe(
                &project,
                &player_state,
                update_key.clone(),
                field,
                stored_scalar_value(field, next),
            );
        })
        .on_commit(move |_| {
            commit_keyframe_edit(&commit_project, &commit_player_state);
        })
        .build_with_handle();
    let widget = parts.widget;
    connect_scalar_base_display(context, key.clone(), field, parts.handle);
    widget
}

pub(super) fn scalar_keyframe_body_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
    display: ResolvedTransform,
) -> gtk::Widget {
    let graph_project = context.project.clone();
    let graph_key = key.clone();
    build_transform_keyframe_editor(
        context,
        TransformKeyframeEditorInput {
            value_editor: keyframe_body_placeholder(),
            graph_data: scalar_value_graph(&context.project.borrow(), key.clone(), field, display),
            refresh_graph: Rc::new(move || {
                scalar_value_graph(&graph_project.borrow(), graph_key.clone(), field, display)
            }),
            visible_area: selected_video_visible_area(context, key.clone()),
            view_state_scope: transform_graph_view_scope(TransformField::Scalar(field)),
            add_at_time: add_time_callback(context, key.clone(), TransformField::Scalar(field)),
            delete_at_time: delete_time_callback(
                context,
                key.clone(),
                TransformField::Scalar(field),
            ),
            update_point: point_callback(context, key.clone(), TransformField::Scalar(field)),
            copy_keyframes: copy_keyframes_callback(
                context,
                key.clone(),
                TransformField::Scalar(field),
            ),
            paste_keyframes: paste_keyframes_callback(
                context,
                key.clone(),
                TransformField::Scalar(field),
            ),
            set_interpolation: interpolation_callback(
                context,
                key.clone(),
                TransformField::Scalar(field),
            ),
        },
    )
}

fn keyframe_body_placeholder() -> gtk::Widget {
    gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast()
}

fn build_transform_keyframe_editor(
    context: &InspectorContext,
    input: TransformKeyframeEditorInput,
) -> gtk::Widget {
    let refresh_graph = input.refresh_graph.clone();
    let built = keyframe_editor::build(
        context,
        input.value_editor,
        input.graph_data,
        input.visible_area,
        input.view_state_scope,
        keyframe_editor::KeyframeEditorActions {
            add_at_time: input.add_at_time,
            delete_at_time: input.delete_at_time,
            update_point: input.update_point,
            copy_keyframes: input.copy_keyframes,
            paste_keyframes: input.paste_keyframes,
            set_interpolation: Some(input.set_interpolation),
            text_interpolation: None,
            toggle_playback: {
                let player_state = context.player_state.clone();
                Rc::new(move || player_state::toggle_playing(&player_state))
            },
        },
    );
    keyframe_editor::connect_graph_refresh(
        context,
        "inspector keyframe graph refresh",
        built.update_graph.clone(),
        move || Some(refresh_graph()),
    );
    built.widget
}

fn transform_graph_view_scope(field: TransformField) -> &'static str {
    match field {
        TransformField::Vec2(Vec2Field::Position) => "transform:position",
        TransformField::Vec2(Vec2Field::Anchor) => "transform:anchor",
        TransformField::Vec2(Vec2Field::Scale) => "transform:scale",
        TransformField::Vec2(Vec2Field::Shear) => "transform:shear",
        TransformField::Scalar(ScalarField::RotationDegrees) => "transform:rotation",
    }
}

fn update_vec2_keyframe(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: Vec2Field,
    component: usize,
    value: f64,
) {
    let position = player_state::snapshot(player_state).position;
    let Some((local_time, current)) = keyframe_edit_context(project, key.clone(), position) else {
        return;
    };
    let mut next = display_vec2_value(current, field);
    let value = value as f32;
    if component == 0 {
        next.x = value;
    } else {
        next.y = value;
    }

    let mut stored = stored_vec2_value(field, next);
    if matches!(field, Vec2Field::Scale) {
        stored = stored.max(Vec2::ZERO);
    }

    let mut project = project.borrow_mut();
    let Some(keyframes) = selected_transform_mut(&mut project, key.clone())
        .and_then(|transform| vec2_keyframes_mut(transform, field))
    else {
        return;
    };
    if let Some(keyframe) = keyframes
        .iter_mut()
        .find(|keyframe| keyframe.time.approx_eq(local_time))
    {
        keyframe.time = local_time;
        keyframe.value = stored;
        keyframes.sort_by_key(|keyframe| keyframe.time);
    } else {
        insert_vec2_keyframe(keyframes, vec2_keyframe(local_time, stored));
    }
    drop(project);
    refresh_video(player_state);
}

fn update_scalar_keyframe(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: ScalarField,
    value: f64,
) {
    let position = player_state::snapshot(player_state).position;
    let Some((local_time, current)) = keyframe_edit_context(project, key.clone(), position) else {
        return;
    };
    let current = raw_scalar_value(current, field) as f32;
    let next = clamp_scalar_keyframe_value(field, value) as f32;

    let mut project = project.borrow_mut();
    let Some(keyframes) = selected_transform_mut(&mut project, key.clone())
        .and_then(|transform| scalar_keyframes_mut(transform, field))
    else {
        return;
    };
    let value = if next.is_finite() { next } else { current };
    if let Some(keyframe) = keyframes
        .iter_mut()
        .find(|keyframe| keyframe.time.approx_eq(local_time))
    {
        keyframe.time = local_time;
        keyframe.value = value;
        keyframes.sort_by_key(|keyframe| keyframe.time);
    } else {
        insert_scalar_keyframe(keyframes, scalar_keyframe(local_time, value));
    }
    drop(project);
    refresh_video(player_state);
}

fn commit_keyframe_edit(project: &Rc<RefCell<Project>>, player_state: &SharedPlayerState) {
    shrimply_project::project::commit_edit(&project.borrow(), "video-transform-keyframe");
    player_state::refresh_project(
        player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
}

fn delete_keyframe_at_time(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: TransformField,
    time: Time,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return false;
    };
    let deleted = match field {
        TransformField::Vec2(field) => {
            let Some(keyframes) = vec2_keyframes_mut(transform, field) else {
                return false;
            };
            delete_keyframe(keyframes, time)
        }
        TransformField::Scalar(field) => {
            let Some(keyframes) = scalar_keyframes_mut(transform, field) else {
                return false;
            };
            delete_keyframe(keyframes, time)
        }
    };
    if !deleted {
        return false;
    }

    shrimply_project::project::commit_edit(&project, "delete-transform-keyframe");
    drop(project);
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

fn add_keyframe_at_time(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: TransformField,
    time: Time,
) -> bool {
    let current = {
        let project = project.borrow();
        let Some(item) = project.video_item(&key) else {
            return false;
        };
        let position = sequence_position_for_local_time(item, time);
        transform_eval::resolve_item_base_transform(&project, item, position)
    };

    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return false;
    };
    match field {
        TransformField::Vec2(field) => {
            let Some(keyframes) = vec2_keyframes_mut(transform, field) else {
                return false;
            };
            if has_keyframe(keyframes, time) {
                return false;
            }
            let mut value = raw_vec2_value(current, field);
            if matches!(field, Vec2Field::Scale) {
                value = value.max(Vec2::ZERO);
            }
            insert_vec2_keyframe(keyframes, vec2_keyframe(time, value));
        }
        TransformField::Scalar(field) => {
            let Some(keyframes) = scalar_keyframes_mut(transform, field) else {
                return false;
            };
            if has_keyframe(keyframes, time) {
                return false;
            }
            insert_scalar_keyframe(
                keyframes,
                scalar_keyframe(
                    time,
                    clamp_scalar_value(field, raw_scalar_value(current, field)) as f32,
                ),
            );
        }
    }

    shrimply_project::project::commit_edit(&project, "add-transform-keyframe");
    drop(project);
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

fn scalar_value_graph(
    project: &Project,
    key: SelectedItem,
    field: ScalarField,
    display: ResolvedTransform,
) -> KeyframeGraph {
    let static_value = display_scalar_value(display, field);
    let Some(keyframes) = project.video_item(&key).and_then(|item| {
        match &scalar_field(&item.transform, field).base {
            TimelineBase::Keyframes(keyframes) => Some(keyframes.as_slice()),
            TimelineBase::Const(_) => None,
        }
    }) else {
        return KeyframeGraph::RawValue {
            points: Vec::new(),
            segments: Vec::new(),
            static_value,
        };
    };
    KeyframeGraph::RawValue {
        points: keyframes
            .iter()
            .map(|keyframe| KeyframePoint {
                time: keyframe.time,
                value: display_scalar_keyframe_value(field, keyframe.value),
            })
            .collect(),
        segments: keyframes
            .windows(2)
            .map(|pair| RawSegment {
                owner_id: pair[0].id,
                start: pair[0].time,
                end: pair[1].time,
                start_value: display_scalar_keyframe_value(field, pair[0].value),
                end_value: display_scalar_keyframe_value(field, pair[1].value),
                interpolation: pair[0].interpolation_to_next,
            })
            .collect(),
        static_value,
    }
}

fn vec2_speed_graph(project: &Project, key: SelectedItem, field: Vec2Field) -> KeyframeGraph {
    let Some(keyframes) =
        project
            .video_item(&key)
            .and_then(|item| match &vec2_field(&item.transform, field).base {
                TimelineBase::Keyframes(keyframes) => Some(keyframes.as_slice()),
                TimelineBase::Const(_) => None,
            })
    else {
        return KeyframeGraph::Speed {
            segments: Vec::new(),
            keys: Vec::new(),
            static_value: 0.0,
        };
    };
    let segments = vec2_speed_segments(keyframes, field);
    KeyframeGraph::Speed {
        segments,
        keys: keyframes.iter().map(|keyframe| keyframe.time).collect(),
        static_value: 0.0,
    }
}

fn vec2_speed_segments(
    keyframes: &[TimelineVectorKeyframe<glam::Vec2>],
    field: Vec2Field,
) -> Vec<SpeedSegment> {
    keyframes
        .windows(2)
        .filter_map(|pair| {
            let start = pair[0].time;
            let end = pair[1].time;
            let seconds = end.signed_sub(start).as_secs_f64();
            if seconds <= f64::EPSILON {
                return None;
            }
            let distance = (display_vec2_keyframe_value(field, pair[1].value)
                - display_vec2_keyframe_value(field, pair[0].value))
            .length() as f64;
            let speed = distance / seconds;
            Some(SpeedSegment {
                owner_id: pair[0].id,
                start,
                end,
                value: speed,
                interpolation: pair[0].interpolation_to_next,
            })
        })
        .collect()
}

fn selected_video_visible_area(context: &InspectorContext, key: SelectedItem) -> (Time, Time) {
    crate::video::visual_visible_area(&context.project.borrow(), key)
        .unwrap_or((Time::ZERO, Time::ZERO))
}

fn delete_time_callback(
    context: &InspectorContext,
    key: SelectedItem,
    field: TransformField,
) -> Rc<dyn Fn(Time)> {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let refresh = context.refresh.clone();
    Rc::new(move |time| {
        if delete_keyframe_at_time(&project, &player_state, key.clone(), field, time) {
            refresh();
        }
    })
}

fn add_time_callback(
    context: &InspectorContext,
    key: SelectedItem,
    field: TransformField,
) -> Rc<dyn Fn(Time)> {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let refresh = context.refresh.clone();
    Rc::new(move |time| {
        if add_keyframe_at_time(&project, &player_state, key.clone(), field, time) {
            refresh();
        }
    })
}

fn point_callback(
    context: &InspectorContext,
    key: SelectedItem,
    field: TransformField,
) -> Rc<dyn Fn(Time, Time, f64)> {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    Rc::new(move |old_time, time, value| {
        update_keyframe_point(
            &project,
            &player_state,
            key.clone(),
            field,
            old_time,
            time,
            value,
        );
    })
}

fn copy_keyframes_callback(
    context: &InspectorContext,
    key: SelectedItem,
    field: TransformField,
) -> keyframe_editor::CopyKeyframes {
    let project = context.project.clone();
    Rc::new(move |times| {
        let project = project.borrow();
        let transform = &project.video_item(&key)?.transform;
        match field {
            TransformField::Vec2(field) => {
                keyframe_model::copy_keyframes(vec2_field(transform, field), times)
            }
            TransformField::Scalar(field) => {
                keyframe_model::copy_keyframes(scalar_field(transform, field), times)
            }
        }
    })
}

fn paste_keyframes_callback(
    context: &InspectorContext,
    key: SelectedItem,
    field: TransformField,
) -> keyframe_editor::PasteKeyframes {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    Rc::new(move |clipboard, time| {
        let mut project = project.borrow_mut();
        let transform = selected_transform_mut(&mut project, key.clone())?;
        let times = match field {
            TransformField::Vec2(field) => {
                keyframe_model::paste_keyframes(vec2_field_mut(transform, field), clipboard, time)
            }
            TransformField::Scalar(field) => {
                keyframe_model::paste_keyframes(scalar_field_mut(transform, field), clipboard, time)
            }
        }?;
        shrimply_project::project::commit_edit(&project, "paste-transform-keyframes");
        drop(project);
        player_state::refresh_project(
            &player_state,
            ProjectChange {
                video: true,
                inspector: true,
                ..ProjectChange::default()
            },
        );
        Some(times)
    })
}

fn interpolation_callback(
    context: &InspectorContext,
    key: SelectedItem,
    field: TransformField,
) -> Rc<dyn Fn(uuid::Uuid, Interpolation)> {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    Rc::new(move |owner_id, interpolation| {
        set_keyframe_interpolation(
            &project,
            &player_state,
            key.clone(),
            field,
            owner_id,
            interpolation,
        );
    })
}

fn update_keyframe_point(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: TransformField,
    old_time: Time,
    time: Time,
    value: f64,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return false;
    };
    let changed = match field {
        TransformField::Scalar(field) => {
            let Some(keyframes) = scalar_keyframes_mut(transform, field) else {
                return false;
            };
            update_scalar_point(keyframes, field, old_time, time, value)
        }
        TransformField::Vec2(field) => {
            let Some(keyframes) = vec2_keyframes_mut(transform, field) else {
                return false;
            };
            update_vec2_point(keyframes, field, old_time, time, value)
        }
    };
    if !changed {
        return false;
    }
    shrimply_project::project::commit_coalesced_edit(&project, "video-transform-keyframe-point");
    drop(project);
    refresh_video(player_state);
    true
}

fn set_keyframe_interpolation(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: TransformField,
    owner_id: uuid::Uuid,
    interpolation: Interpolation,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return false;
    };
    let changed = match field {
        TransformField::Scalar(field) => {
            let Some(keyframes) = scalar_keyframes_mut(transform, field) else {
                return false;
            };
            set_interpolation(keyframes, owner_id, interpolation)
        }
        TransformField::Vec2(field) => {
            let Some(keyframes) = vec2_keyframes_mut(transform, field) else {
                return false;
            };
            set_interpolation(keyframes, owner_id, interpolation)
        }
    };
    if !changed {
        return false;
    }
    shrimply_project::project::commit_edit(&project, "video-transform-keyframe-interpolation");
    drop(project);
    refresh_video(player_state);
    true
}

fn set_interpolation<T: TimelineValueType>(
    keyframes: &mut [TimelineCurveKeyframe<T>],
    owner_id: uuid::Uuid,
    interpolation: Interpolation,
) -> bool {
    let Some(keyframe) = keyframes
        .iter_mut()
        .find(|keyframe| keyframe.id == owner_id)
    else {
        return false;
    };
    if keyframe.interpolation_to_next == interpolation {
        return false;
    }
    keyframe.interpolation_to_next = interpolation;
    true
}

fn update_scalar_point(
    keyframes: &mut Vec<TimelineScalarKeyframe<f32>>,
    field: ScalarField,
    old_time: Time,
    time: Time,
    value: f64,
) -> bool {
    let Some(index) = keyframes
        .iter()
        .position(|keyframe| keyframe.time.approx_eq(old_time))
    else {
        return false;
    };
    let mut keyframe = keyframes.remove(index);
    keyframe.time = time;
    keyframe.value = clamp_scalar_value(field, stored_scalar_value(field, value)) as f32;
    upsert_scalar_keyframe(keyframes, keyframe);
    true
}

fn update_vec2_point(
    keyframes: &mut Vec<TimelineVectorKeyframe<glam::Vec2>>,
    _field: Vec2Field,
    old_time: Time,
    time: Time,
    _value: f64,
) -> bool {
    let Some(index) = keyframes
        .iter()
        .position(|keyframe| keyframe.time.approx_eq(old_time))
    else {
        return false;
    };
    let mut keyframe = keyframes.remove(index);
    keyframe.time = time;
    upsert_vec2_keyframe(keyframes, keyframe);
    true
}

fn raw_scalar_value(transform: ResolvedTransform, field: ScalarField) -> f64 {
    match field {
        ScalarField::RotationDegrees => transform.rotation_degrees as f64,
    }
}

fn raw_vec2_value(transform: ResolvedTransform, field: Vec2Field) -> Vec2 {
    match field {
        Vec2Field::Position => transform.position,
        Vec2Field::Anchor => transform.anchor,
        Vec2Field::Scale => transform.scale,
        Vec2Field::Shear => transform.shear,
    }
}

fn display_vec2_keyframe_value(field: Vec2Field, value: Vec2) -> Vec2 {
    match field {
        Vec2Field::Position | Vec2Field::Anchor | Vec2Field::Scale | Vec2Field::Shear => value,
    }
}

pub(super) fn stored_vec2_value(field: Vec2Field, value: Vec2) -> Vec2 {
    match field {
        Vec2Field::Position | Vec2Field::Anchor | Vec2Field::Scale | Vec2Field::Shear => value,
    }
}

fn sequence_position_for_local_time(item: &VideoItem, time: Time) -> Time {
    item.start
        .saturating_add(time.signed_sub(item.animation_time_offset))
}

fn keyframe_edit_context(
    project: &Rc<RefCell<Project>>,
    key: SelectedItem,
    position: Time,
) -> Option<(Time, ResolvedTransform)> {
    let project = project.borrow();
    let item = project.video_item(&key)?;
    let sequence_position = project.timeline_time_to_sequence(&key.track(), position)?;
    let local_time = project.keyframe_time(&key, position)?;
    Some((
        local_time,
        transform_eval::resolve_item_base_transform(&project, item, sequence_position),
    ))
}

fn scalar_keyframe(time: Time, value: f32) -> TimelineScalarKeyframe<f32> {
    TimelineScalarKeyframe::<f32> {
        id: uuid::Uuid::new_v4(),
        time,
        value,
        interpolation_to_next: Default::default(),
    }
}

fn vec2_keyframe(time: Time, value: Vec2) -> TimelineVectorKeyframe<glam::Vec2> {
    TimelineVectorKeyframe::<glam::Vec2> {
        id: uuid::Uuid::new_v4(),
        time,
        value,
        interpolation_to_next: Default::default(),
    }
}

fn insert_scalar_keyframe(
    keyframes: &mut Vec<TimelineScalarKeyframe<f32>>,
    mut next: TimelineScalarKeyframe<f32>,
) {
    inherit_split_interpolation(keyframes, &mut next);
    upsert_scalar_keyframe(keyframes, next);
}

fn insert_vec2_keyframe(
    keyframes: &mut Vec<TimelineVectorKeyframe<glam::Vec2>>,
    mut next: TimelineVectorKeyframe<glam::Vec2>,
) {
    inherit_split_interpolation(keyframes, &mut next);
    upsert_vec2_keyframe(keyframes, next);
}

fn inherit_split_interpolation<T: TimelineValueType>(
    keyframes: &[TimelineCurveKeyframe<T>],
    next: &mut TimelineCurveKeyframe<T>,
) {
    if let Some(previous) = keyframes
        .iter()
        .rev()
        .find(|keyframe| keyframe.time < next.time)
        && keyframes.iter().any(|keyframe| keyframe.time > next.time)
    {
        next.interpolation_to_next = previous.interpolation_to_next;
    }
}

fn upsert_scalar_keyframe(
    keyframes: &mut Vec<TimelineScalarKeyframe<f32>>,
    mut next: TimelineScalarKeyframe<f32>,
) {
    if let Some(current) = keyframes
        .iter_mut()
        .find(|current| current.time.approx_eq(next.time))
    {
        next.id = current.id;
        *current = next;
        return;
    }
    keyframes.push(next);
    keyframes.sort_by_key(|keyframe| keyframe.time);
}

fn upsert_vec2_keyframe(
    keyframes: &mut Vec<TimelineVectorKeyframe<glam::Vec2>>,
    mut next: TimelineVectorKeyframe<glam::Vec2>,
) {
    if let Some(current) = keyframes
        .iter_mut()
        .find(|current| current.time.approx_eq(next.time))
    {
        next.id = current.id;
        *current = next;
        return;
    }
    keyframes.push(next);
    keyframes.sort_by_key(|keyframe| keyframe.time);
}

fn delete_keyframe<T: TransformKeyframe>(keyframes: &mut Vec<T>, time: Time) -> bool {
    let Some(index) = keyframes
        .iter()
        .position(|keyframe| keyframe.time().approx_eq(time))
    else {
        return false;
    };
    keyframes.remove(index);
    true
}

fn has_keyframe<T: TransformKeyframe>(keyframes: &[T], time: Time) -> bool {
    keyframes
        .iter()
        .any(|keyframe| keyframe.time().approx_eq(time))
}

trait TransformKeyframe {
    fn time(&self) -> Time;
}

impl TransformKeyframe for TimelineScalarKeyframe<f32> {
    fn time(&self) -> Time {
        self.time
    }
}

impl TransformKeyframe for TimelineVectorKeyframe<glam::Vec2> {
    fn time(&self) -> Time {
        self.time
    }
}
