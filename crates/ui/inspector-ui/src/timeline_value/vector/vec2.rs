use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;

use glam::Vec2;
use gtk::prelude::*;

use crate::InspectedItem as SelectedItem;
use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::timeline_value::*;
use crate::transform_eval::{self, TransformExpressionCache};
use crate::ui::Number2Picker;
use crate::{InspectorContext, keyframe_model, timeline_value::layered};
use shrimply_project::project::{Project, Time, VisualAlphaMaskTarget};

use crate::keyframe_editor::{self, KeyframeEditorActions};

pub(crate) type VecGet =
    for<'a> fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<glam::Vec2>>;
pub(crate) type VecGetMut =
    for<'a> fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<glam::Vec2>>;

#[derive(Clone, Copy)]
pub(crate) enum VecAccess {
    Item {
        get: VecGet,
        get_mut: VecGetMut,
    },
    ItemWithMutation {
        get: VecGet,
        get_mut: VecGetMut,
        mutated: fn(&mut Project, SelectedItem),
    },
    Modifier {
        id: Uuid,
        value_id: Uuid,
    },
    AlphaMask {
        target: VisualAlphaMaskTarget,
        value_id: Uuid,
    },
    Background {
        value_id: Uuid,
    },
}
impl VecAccess {
    fn get(self, p: &Project, k: SelectedItem) -> Option<&TimelineValue<glam::Vec2>> {
        match self {
            Self::Item { get, .. } | Self::ItemWithMutation { get, .. } => get(p, k.clone()),
            Self::Modifier { id, value_id } => crate::modifiers::number2(
                p.video_item(&k)?.modifiers.iter().find(|m| m.id == id)?,
                value_id,
            ),
            Self::AlphaMask { target, value_id } => {
                p.video_item(&k)?.alpha_mask(target)?.number2(value_id)
            }
            Self::Background { value_id } => {
                let shrimply_project::project::VideoItemContent::Background(background) =
                    &p.video_item(&k)?.content
                else {
                    return None;
                };
                background.generator.number2(value_id)
            }
        }
    }
    fn get_mut(self, p: &mut Project, k: SelectedItem) -> Option<&mut TimelineValue<glam::Vec2>> {
        match self {
            Self::Item { get_mut, .. } | Self::ItemWithMutation { get_mut, .. } => {
                get_mut(p, k.clone())
            }
            Self::Modifier { id, value_id } => crate::modifiers::number2_mut(
                p.video_item_mut(&k)?
                    .modifiers
                    .iter_mut()
                    .find(|m| m.id == id)?,
                value_id,
            ),
            Self::AlphaMask { target, value_id } => p
                .video_item_mut(&k)?
                .alpha_mask_mut(target)?
                .number2_mut(value_id),
            Self::Background { value_id } => {
                let shrimply_project::project::VideoItemContent::Background(background) =
                    &mut p.video_item_mut(&k)?.content
                else {
                    return None;
                };
                background.generator.number2_mut(value_id)
            }
        }
    }

    fn mark_mutated(self, project: &mut Project, key: SelectedItem) {
        if let Self::ItemWithMutation { mutated, .. } = self {
            mutated(project, key);
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VecTarget {
    pub(crate) access: VecAccess,
    pub(crate) scope_id: Option<Uuid>,
    pub(crate) local_time: fn(&Project, SelectedItem, Time) -> Option<Time>,
    pub(crate) global_time: fn(&Project, SelectedItem, Time) -> Option<Time>,
    pub(crate) duration: fn(&Project, SelectedItem) -> Option<Time>,
    pub(crate) refresh: ProjectChange,
    pub(crate) commit_name: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct VecSpec {
    pub(crate) first_prefix: &'static str,
    pub(crate) second_prefix: &'static str,
    pub(crate) drag_step: f64,
    pub(crate) digits: usize,
    pub(crate) width_chars: i32,
    pub(crate) minimum: Option<f64>,
    pub(crate) maximum: Option<f64>,
    pub(crate) unit_name: &'static str,
}

pub(crate) fn vec_control(
    label: &str,
    value: &TimelineValue<glam::Vec2>,
    context: &InspectorContext,
    target: VecTarget,
    spec: VecSpec,
) -> gtk::Widget {
    vec_control_with_lock(label, value, context, target, spec, false)
}

pub(crate) fn vec_control_with_lock(
    label: &str,
    value: &TimelineValue<glam::Vec2>,
    context: &InspectorContext,
    target: VecTarget,
    spec: VecSpec,
    lock: bool,
) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        return layered::control(
            label,
            value,
            picker(value.fallback(), context, target, spec, lock),
            Vec::new(),
            |_| {},
            |_| {},
        );
    };
    let position = player_state::snapshot(&context.player_state).position;
    let local_time = {
        let project = context.project.borrow();
        (target.local_time)(&project, key.clone(), position).unwrap_or(Time::ZERO)
    };
    let current = base_value(value, local_time);
    let keyframes_enabled = matches!(value.base, TimelineBase::Keyframes(_));
    let expression_enabled = value
        .expression
        .as_ref()
        .is_some_and(|expression| expression.enabled);
    let mut body = Vec::new();
    if keyframes_enabled {
        let graph = super::speed_graph(value);
        let duration = {
            let project = context.project.borrow();
            (target.duration)(&project, key.clone()).unwrap_or(Time::ZERO)
        };
        let built = keyframe_editor::build(
            context,
            keyframe_body_placeholder(),
            graph,
            (Time::ZERO, duration),
            format!("vec:{}:{:?}:{label}", target.commit_name, target.scope_id),
            keyframe_actions(context, key.clone(), target),
        );
        let project = context.project.clone();
        let graph_key = key.clone();
        keyframe_editor::connect_graph_refresh(
            context,
            "inspector vec keyframe graph refresh",
            built.update_graph.clone(),
            move || {
                target
                    .access
                    .get(&project.borrow(), graph_key.clone())
                    .map(super::speed_graph)
            },
        );
        body.push(built.widget);
    }
    if expression_enabled {
        body.push(expression_editor(context, key.clone(), target));
        body.push(expression_output(context, key.clone(), target));
    }
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
        picker(current, context, target, spec, lock),
        body,
        move |enabled| {
            if set_keyframes_enabled(
                &keyframe_project,
                &keyframe_player_state,
                keyframe_key.clone(),
                target,
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
                target,
                enabled,
            ) {
                expression_refresh();
            }
        },
    )
}

fn keyframe_body_placeholder() -> gtk::Widget {
    gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast()
}

fn picker(
    value: Vec2,
    context: &InspectorContext,
    target: VecTarget,
    spec: VecSpec,
    lock: bool,
) -> gtk::Widget {
    let mut picker = Number2Picker::builder(value.x as f64, value.y as f64)
        .first_prefix(spec.first_prefix)
        .second_prefix(spec.second_prefix)
        .drag_step(spec.drag_step)
        .digits(spec.digits)
        .width_chars(spec.width_chars)
        .unit_name(spec.unit_name);
    if let Some(minimum) = spec.minimum {
        picker = picker.minimum(minimum);
    }
    if let Some(maximum) = spec.maximum {
        picker = picker.maximum(maximum);
    }
    if lock {
        picker = picker.enable_lock();
    }
    let Some(key) = context.selected_item.clone() else {
        return picker.build_with_handles().widget;
    };
    let first_project = context.project.clone();
    let first_player_state = context.player_state.clone();
    let second_project = context.project.clone();
    let second_player_state = context.player_state.clone();
    let first_commit_project = context.project.clone();
    let second_commit_project = context.project.clone();
    let first_commit_player_state = context.player_state.clone();
    let second_commit_player_state = context.player_state.clone();
    let first_key = key.clone();
    let second_key = key;
    picker
        .on_first_change(move |next| {
            update_component(
                &first_project,
                &first_player_state,
                first_key.clone(),
                target,
                0,
                next,
            );
        })
        .on_second_change(move |next| {
            update_component(
                &second_project,
                &second_player_state,
                second_key.clone(),
                target,
                1,
                next,
            );
        })
        .on_first_commit(move |_| {
            shrimply_project::project::commit_edit(
                &first_commit_project.borrow(),
                target.commit_name,
            );
            player_state::refresh_project(&first_commit_player_state, target.refresh);
        })
        .on_second_commit(move |_| {
            shrimply_project::project::commit_edit(
                &second_commit_project.borrow(),
                target.commit_name,
            );
            player_state::refresh_project(&second_commit_player_state, target.refresh);
        })
        .build_with_handles()
        .widget
}

fn update_component(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    component: usize,
    next: f64,
) {
    if !next.is_finite() {
        return;
    }
    let position = player_state::snapshot(player_state).position;
    let mut project = project.borrow_mut();
    let Some(local_time) = (target.local_time)(&project, key.clone(), position) else {
        return;
    };
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return;
    };
    let mut vec = base_value(value, local_time);
    if component == 0 {
        vec.x = next as f32;
    } else {
        vec.y = next as f32;
    }
    if !set_value(value, local_time, vec) {
        return;
    }
    target.access.mark_mutated(&mut project, key);
    drop(project);
    player_state::refresh_project(player_state, keyframe_model::live_refresh(target.refresh));
}

fn set_keyframes_enabled(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    enabled: bool,
) -> bool {
    let position = player_state::snapshot(player_state).position;
    let mut project = project.borrow_mut();
    let Some(local_time) = (target.local_time)(&project, key.clone(), position) else {
        return false;
    };
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return false;
    };
    let current = base_value(value, local_time);
    match (&mut value.base, enabled) {
        (TimelineBase::Const(_), false) | (TimelineBase::Keyframes(_), true) => return false,
        (base @ TimelineBase::Const(_), true) => {
            *base = TimelineBase::Keyframes(vec![keyframe(local_time, current)]);
        }
        (base @ TimelineBase::Keyframes(_), false) => *base = TimelineBase::Const(current),
    }
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_edit(&project, target.commit_name);
    drop(project);
    player_state::refresh_project(player_state, target.refresh);
    true
}

fn set_expression_enabled(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    enabled: bool,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return false;
    };
    let changed = match &mut value.expression {
        Some(expression) if expression.enabled != enabled => {
            expression.enabled = enabled;
            true
        }
        Some(_) => false,
        None if enabled => {
            value.expression = Some(TimelineExpression {
                id: uuid::Uuid::new_v4(),
                enabled: true,
                source: "[x, y]".to_string(),
            });
            true
        }
        None => false,
    };
    if !changed {
        return false;
    }
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_edit(&project, target.commit_name);
    drop(project);
    player_state::refresh_project(player_state, target.refresh);
    true
}

fn keyframe_actions(
    context: &InspectorContext,
    key: SelectedItem,
    target: VecTarget,
) -> KeyframeEditorActions {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let playhead_project = project.clone();
    let playhead_player = player_state.clone();
    let select_project = project.clone();
    let select_player = player_state.clone();
    let add_project = project.clone();
    let add_player = player_state.clone();
    let delete_project = project.clone();
    let delete_player = player_state.clone();
    let point_project = project.clone();
    let point_player = player_state.clone();
    let copy_project = project.clone();
    let paste_project = project.clone();
    let paste_player = player_state.clone();
    let interpolation_project = project;
    let interpolation_player = player_state.clone();
    let playback_player = player_state;
    let refresh_add = context.refresh.clone();
    let refresh_delete = context.refresh.clone();
    let playhead_key = key.clone();
    let select_key = key.clone();
    let add_key = key.clone();
    let delete_key = key.clone();
    let point_key = key.clone();
    let copy_key = key.clone();
    let paste_key = key.clone();
    let interpolation_key = key;
    KeyframeEditorActions {
        playhead: Rc::new(move || {
            let project = playhead_project.borrow();
            let position = player_state::snapshot(&playhead_player).position;
            (target.local_time)(&project, playhead_key.clone(), position).unwrap_or(Time::ZERO)
        }),
        select_time: Rc::new(move |time| {
            let project = select_project.borrow();
            if let Some(position) = (target.global_time)(&project, select_key.clone(), time) {
                player_state::set_position(&select_player, position);
            }
        }),
        add_at_time: Rc::new(move |time| {
            if add_keyframe_at_time(&add_project, &add_player, add_key.clone(), target, time) {
                refresh_add();
            }
        }),
        delete_at_time: Rc::new(move |time| {
            if delete_keyframe_at_time(
                &delete_project,
                &delete_player,
                delete_key.clone(),
                target,
                time,
            ) {
                refresh_delete();
            }
        }),
        update_point: Rc::new(move |old_time, time, value| {
            update_keyframe_point(
                &point_project,
                &point_player,
                point_key.clone(),
                target,
                old_time,
                time,
                value,
            );
        }),
        copy_keyframes: Rc::new(move |times| {
            target
                .access
                .get(&copy_project.borrow(), copy_key.clone())
                .and_then(|value| keyframe_model::copy_keyframes(value, times))
        }),
        paste_keyframes: Rc::new(move |clipboard, time| {
            let mut project = paste_project.borrow_mut();
            let frame_step = keyframe_editor::project_frame_step(&project);
            let value = target.access.get_mut(&mut project, paste_key.clone())?;
            let times = keyframe_model::paste_keyframes(value, clipboard, time, frame_step)?;
            target.access.mark_mutated(&mut project, paste_key.clone());
            shrimply_project::project::commit_edit(&project, target.commit_name);
            drop(project);
            player_state::refresh_project(&paste_player, target.refresh);
            Some(times)
        }),
        set_interpolation: Some(Rc::new(move |owner_id, interpolation| {
            set_keyframe_interpolation(
                &interpolation_project,
                &interpolation_player,
                interpolation_key.clone(),
                target,
                owner_id,
                interpolation,
            );
        })),
        text_interpolation: None,
        toggle_playback: Rc::new(move || player_state::toggle_playing(&playback_player)),
    }
}

fn add_keyframe_at_time(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    time: Time,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return false;
    };
    let next = base_value(value, time);
    let TimelineBase::Keyframes(keyframes) = &mut value.base else {
        return false;
    };
    if keyframes
        .iter()
        .any(|keyframe| keyframe.time.approx_eq(time))
    {
        return false;
    }
    insert_keyframe(keyframes, keyframe(time, next));
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_edit(&project, target.commit_name);
    drop(project);
    player_state::refresh_project(player_state, target.refresh);
    true
}

fn delete_keyframe_at_time(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    time: Time,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return false;
    };
    let TimelineBase::Keyframes(keyframes) = &mut value.base else {
        return false;
    };
    let Some(index) = keyframes
        .iter()
        .position(|keyframe| keyframe.time.approx_eq(time))
    else {
        return false;
    };
    keyframes.remove(index);
    if keyframes.is_empty() {
        value.base = TimelineBase::Const(Vec2::ZERO);
    }
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_edit(&project, target.commit_name);
    drop(project);
    player_state::refresh_project(player_state, target.refresh);
    true
}

fn update_keyframe_point(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    old_time: Time,
    time: Time,
    _value: f64,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(keyframes) = keyframes_mut(&mut project, key.clone(), target) else {
        return false;
    };
    let Some(index) = keyframes
        .iter()
        .position(|keyframe| keyframe.time.approx_eq(old_time))
    else {
        return false;
    };
    keyframes[index].time = time;
    keyframes.sort_by_key(|keyframe| keyframe.time);
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_coalesced_edit(&project, target.commit_name);
    drop(project);
    let mut refresh = target.refresh;
    refresh.inspector = false;
    player_state::refresh_project(player_state, refresh);
    true
}

fn set_keyframe_interpolation(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    owner_id: Uuid,
    interpolation: Interpolation,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(keyframes) = keyframes_mut(&mut project, key.clone(), target) else {
        return false;
    };
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
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_edit(&project, target.commit_name);
    drop(project);
    player_state::refresh_project(player_state, keyframe_model::live_refresh(target.refresh));
    true
}

fn keyframes_mut(
    project: &mut Project,
    key: SelectedItem,
    target: VecTarget,
) -> Option<&mut Vec<TimelineVectorKeyframe<glam::Vec2>>> {
    let value = target.access.get_mut(project, key.clone())?;
    match &mut value.base {
        TimelineBase::Keyframes(keyframes) => Some(keyframes),
        TimelineBase::Const(_) => None,
    }
}

fn base_value(value: &TimelineValue<glam::Vec2>, time: Time) -> Vec2 {
    match &value.base {
        TimelineBase::Const(value) => *value,
        TimelineBase::Keyframes(keyframes) => transform_eval::vec2_keyframes_value(keyframes, time),
    }
}

fn expression_editor(
    context: &InspectorContext,
    key: SelectedItem,
    target: VecTarget,
) -> gtk::Widget {
    let source = {
        let project = context.project.borrow();
        target
            .access
            .get(&project, key.clone())
            .and_then(|value| value.expression_source().map(str::to_string))
    };
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    crate::rhai_editor::editor(
        source,
        crate::rhai_editor::ExpressionValue::Vec2,
        move |source| {
            update_expression_source(&project, &player_state, key.clone(), target, source);
        },
    )
}

fn update_expression_source(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: VecTarget,
    source: String,
) {
    let mut project = project.borrow_mut();
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return;
    };
    let Some(expression) = &mut value.expression else {
        return;
    };
    if expression.source == source {
        return;
    }
    expression.source = source;
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_coalesced_edit(&project, target.commit_name);
    drop(project);
    let mut refresh = target.refresh;
    refresh.inspector = false;
    player_state::refresh_project(player_state, refresh);
}

fn expression_output(
    context: &InspectorContext,
    key: SelectedItem,
    target: VecTarget,
) -> gtk::Widget {
    let label = gtk::Label::new(None);
    label.set_hexpand(true);
    label.set_xalign(1.0);
    label.set_selectable(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.add_css_class("numeric");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_hexpand(true);
    let title = gtk::Label::new(Some("Output"));
    title.add_css_class("dim-label");
    title.set_xalign(0.0);
    row.append(&title);
    row.append(&label);

    let cache = Rc::new(RefCell::new(TransformExpressionCache::default()));
    update_expression_output(
        &context.project,
        &context.player_state,
        &context.volume,
        key.clone(),
        target,
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
        "inspector vec expression output",
        move || alive.upgrade().is_some(),
        move |event| {
            let refresh = match event {
                player_state::PlayerEvent::State => true,
                player_state::PlayerEvent::Project(change) => change.video || change.audio,
            };
            if !refresh {
                return;
            }
            let Some(label) = label.upgrade() else {
                return;
            };
            update_expression_output(
                &project,
                &player_state,
                &volume,
                key.clone(),
                target,
                &label,
                &cache,
            );
        },
    );
    row.upcast()
}

fn update_expression_output(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    volume: &Rc<RefCell<shrimply_audio::streaming::FrameAudioSampler>>,
    key: SelectedItem,
    target: VecTarget,
    label: &gtk::Label,
    cache: &Rc<RefCell<TransformExpressionCache>>,
) {
    let snapshot = player_state::snapshot(player_state);
    let position = snapshot.position;
    let volume_mixer = volume
        .borrow_mut()
        .sample(&project.borrow(), position, snapshot.revision);
    let project = project.borrow();
    let Some(item) = project.video_item(&key) else {
        return;
    };
    let Some(value) = target.access.get(&project, key.clone()) else {
        return;
    };
    let Some(expression) = value.expression.as_ref() else {
        return;
    };
    if !expression.enabled || expression.source.trim().is_empty() {
        return;
    }
    let eval = transform_eval::TransformEvaluation::for_item_with_audio(
        &project,
        item,
        position,
        &volume_mixer,
    );
    let base = transform_eval::resolve_vec2_base(value, &eval);
    match cache
        .borrow_mut()
        .eval_timeline_value_result(&eval, value.id, &expression.source, &base)
    {
        Ok(value) => {
            label.remove_css_class("error");
            label.set_tooltip_text(None);
            label.set_text(&format!(
                "X {:.0}{}  Y {:.0}{}",
                value.x, "px", value.y, "px"
            ));
        }
        Err(error) => {
            label.add_css_class("error");
            label.set_tooltip_text(Some(&error));
            let message = error.lines().next().unwrap_or_default().trim();
            if message.is_empty() {
                label.set_text("Invalid expression");
            } else {
                label.set_text(&format!("Invalid expression: {message}"));
            }
        }
    }
}

fn set_value(value: &mut TimelineValue<glam::Vec2>, local_time: Time, next: Vec2) -> bool {
    match &mut value.base {
        TimelineBase::Const(current) if (*current - next).length_squared() <= 0.000001 => false,
        TimelineBase::Const(current) => {
            *current = next;
            true
        }
        TimelineBase::Keyframes(keyframes) => {
            if let Some(keyframe) = keyframes
                .iter_mut()
                .find(|keyframe| keyframe.time.approx_eq(local_time))
            {
                keyframe.value = next;
            } else {
                insert_keyframe(keyframes, keyframe(local_time, next));
            }
            true
        }
    }
}

fn keyframe(time: Time, value: Vec2) -> TimelineVectorKeyframe<glam::Vec2> {
    TimelineVectorKeyframe::<glam::Vec2> {
        id: uuid::Uuid::new_v4(),
        time,
        value,
        interpolation_to_next: Default::default(),
    }
}

fn insert_keyframe(
    keyframes: &mut Vec<TimelineVectorKeyframe<glam::Vec2>>,
    mut next: TimelineVectorKeyframe<glam::Vec2>,
) {
    if let Some(previous) = keyframes
        .iter()
        .rev()
        .find(|keyframe| keyframe.time < next.time)
        && keyframes.iter().any(|keyframe| keyframe.time > next.time)
    {
        next.interpolation_to_next = previous.interpolation_to_next;
    }
    keyframes.push(next);
    keyframes.sort_by_key(|keyframe| keyframe.time);
}
