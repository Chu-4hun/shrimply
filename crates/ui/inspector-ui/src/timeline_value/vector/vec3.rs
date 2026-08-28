use shrimply_ui_foundation::tr;
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use crate::InspectedItem as SelectedItem;
use glam::Vec3;
use gtk::prelude::*;
use shrimply_core::timeline_value::{
    Interpolation, TimelineBase, TimelineExpression, TimelineValue, TimelineValueType,
    TimelineVectorKeyframe,
};
use shrimply_evaluation::TransformExpressionCache;
use shrimply_project::project::{Project, Time};
use uuid::Uuid;

use crate::{
    InspectorContext,
    keyframe_editor::{self, KeyframeEditorActions},
    keyframe_model,
    player_state::{self, ProjectChange, SharedPlayerState},
    ui::{Number3Picker, NumberPickerHandle},
};

type SceneGet = for<'a> fn(&'a shrimply_scene_3d::ObjScene) -> &'a TimelineValue<Vec3>;
type SceneGetMut = for<'a> fn(&'a mut shrimply_scene_3d::ObjScene) -> &'a mut TimelineValue<Vec3>;
type ItemGet = for<'a> fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<Vec3>>;
type ItemGetMut = for<'a> fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<Vec3>>;

#[derive(Clone, Copy)]
enum Vec3Access {
    Scene3d { get: SceneGet, get_mut: SceneGetMut },
    Item { get: ItemGet, get_mut: ItemGetMut },
    Modifier { id: Uuid, value_id: Uuid },
}

#[derive(Clone, Copy)]
pub(crate) struct Vec3Target {
    access: Vec3Access,
    minimum: Option<f64>,
    degrees: bool,
    lock: bool,
}

pub(crate) struct Vec3TargetBuilder(Vec3Target);

impl Vec3Target {
    pub(crate) fn builder(get: SceneGet, get_mut: SceneGetMut) -> Vec3TargetBuilder {
        Vec3TargetBuilder(Self {
            access: Vec3Access::Scene3d { get, get_mut },
            minimum: None,
            degrees: false,
            lock: false,
        })
    }

    pub(crate) fn item_builder(get: ItemGet, get_mut: ItemGetMut) -> Vec3TargetBuilder {
        Vec3TargetBuilder(Self {
            access: Vec3Access::Item { get, get_mut },
            minimum: None,
            degrees: false,
            lock: false,
        })
    }

    pub(crate) fn modifier_builder(id: Uuid, value_id: Uuid) -> Vec3TargetBuilder {
        Vec3TargetBuilder(Self {
            access: Vec3Access::Modifier { id, value_id },
            minimum: None,
            degrees: false,
            lock: false,
        })
    }

    fn value(self, project: &Project, key: SelectedItem) -> Option<&TimelineValue<Vec3>> {
        match self.access {
            Vec3Access::Scene3d { get, .. } => selected_scene(project, key.clone()).map(get),
            Vec3Access::Item { get, .. } => get(project, key.clone()),
            Vec3Access::Modifier { id, value_id } => crate::modifiers::number3(
                &project
                    .video_item(&key)?
                    .modifiers
                    .iter()
                    .find(|modifier| modifier.id == id)?
                    .effect,
                value_id,
            ),
        }
    }

    fn value_mut(
        self,
        project: &mut Project,
        key: SelectedItem,
    ) -> Option<&mut TimelineValue<Vec3>> {
        match self.access {
            Vec3Access::Scene3d { get_mut, .. } => {
                selected_scene_mut(project, key.clone()).map(get_mut)
            }
            Vec3Access::Item { get_mut, .. } => get_mut(project, key.clone()),
            Vec3Access::Modifier { id, value_id } => crate::modifiers::number3_mut(
                &mut project
                    .video_item_mut(&key)?
                    .modifiers
                    .iter_mut()
                    .find(|modifier| modifier.id == id)?
                    .effect,
                value_id,
            ),
        }
    }
}

impl Vec3TargetBuilder {
    pub(crate) fn degrees(mut self) -> Self {
        self.0.degrees = true;
        self
    }

    pub(crate) fn minimum(mut self, minimum: f64) -> Self {
        self.0.minimum = Some(minimum);
        self
    }

    pub(crate) fn lock(mut self) -> Self {
        self.0.lock = true;
        self
    }

    pub(crate) fn build(self) -> Vec3Target {
        self.0
    }
}

pub(crate) fn control(
    label: &str,
    value: &TimelineValue<Vec3>,
    context: &InspectorContext,
    target: Vec3Target,
) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        return crate::timeline_value::layered::control(
            label,
            value,
            picker(value.fallback(), context, target),
            Vec::new(),
            |_| {},
            |_| {},
        );
    };
    let time = local_time(context, key.clone()).unwrap_or(Time::ZERO);
    let current = value.value_at(time);
    let mut body = Vec::new();
    if matches!(value.base, TimelineBase::Keyframes(_)) {
        let built = keyframe_editor::build(
            context,
            gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
            super::speed_graph(value),
            visible_area(&context.project.borrow(), key.clone())
                .unwrap_or((Time::ZERO, Time::ZERO)),
            format!("vec3:{}:{label}", value.id),
            actions(context, key.clone(), target),
        );
        let project = context.project.clone();
        let graph_key = key.clone();
        keyframe_editor::connect_graph_refresh(
            context,
            "inspector vec3 graph refresh",
            built.update_graph.clone(),
            move || {
                target
                    .value(&project.borrow(), graph_key.clone())
                    .map(super::speed_graph)
            },
        );
        body.push(built.widget);
    }
    if value.expression.as_ref().is_some_and(|value| value.enabled) {
        body.push(expression_editor(context, key.clone(), target));
        body.push(expression_output(context, key.clone(), target));
    }

    let keyframe_project = context.project.clone();
    let keyframe_player = context.player_state.clone();
    let keyframe_refresh = context.refresh.clone();
    let expression_project = context.project.clone();
    let expression_player = context.player_state.clone();
    let expression_refresh = context.refresh.clone();
    let keyframe_key = key.clone();
    let expression_key = key;
    crate::timeline_value::layered::control(
        label,
        value,
        picker(current, context, target),
        body,
        move |enabled| {
            if toggle_keyframes(
                &keyframe_project,
                &keyframe_player,
                keyframe_key.clone(),
                target,
                enabled,
            ) {
                keyframe_refresh();
            }
        },
        move |enabled| {
            if toggle_expression(
                &expression_project,
                &expression_player,
                expression_key.clone(),
                target,
                enabled,
            ) {
                expression_refresh();
            }
        },
    )
}

fn picker(value: Vec3, context: &InspectorContext, target: Vec3Target) -> gtk::Widget {
    let mut picker = Number3Picker::builder(value.x as f64, value.y as f64, value.z as f64)
        .prefixes(["X", "Y", "Z"])
        .drag_step(if target.degrees { 1.0 } else { 0.1 })
        .digits(2)
        .width_chars(5);
    if target.degrees {
        picker = picker.unit_name("°");
    }
    if let Some(minimum) = target.minimum {
        picker = picker.minimum(minimum);
    }
    if target.lock {
        picker = picker.enable_lock();
    }
    let Some(key) = context.selected_item.clone() else {
        return picker.build_with_handles().widget;
    };
    for axis in 0..3 {
        let project = context.project.clone();
        let player = context.player_state.clone();
        let update_key = key.clone();
        picker = picker.on_change(axis, move |next| {
            update_component(&project, &player, update_key.clone(), target, axis, next)
        });
        let project = context.project.clone();
        let player = context.player_state.clone();
        picker = picker.on_commit(axis, move |_| {
            shrimply_project::project::commit_edit(&project.borrow(), "edit-scene-3d-vec3");
            player_state::refresh_project(
                &player,
                ProjectChange {
                    video: true,
                    ..Default::default()
                },
            );
        });
    }
    let parts = picker.build_with_handles();
    connect_display(context, key.clone(), target, parts.handles);
    parts.widget
}

fn update_component(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: Vec3Target,
    axis: usize,
    next: f64,
) {
    if !next.is_finite() {
        return;
    }
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(evaluation_time) = crate::video::visual_local_time(&project, key.clone(), position)
    else {
        return;
    };
    let Some(keyframe_time) = project.keyframe_time(&key, position) else {
        return;
    };
    let Some(value) = target.value_mut(&mut project, key.clone()) else {
        return;
    };
    let mut current = value.value_at(evaluation_time);
    current[axis] = next as f32;
    if target.minimum.is_some() {
        current = current.max(Vec3::splat(target.minimum.unwrap_or_default() as f32));
    }
    if set_value(value, keyframe_time, current) {
        drop(project);
        player_state::refresh_project(
            player,
            keyframe_model::live_refresh(ProjectChange {
                video: true,
                ..Default::default()
            }),
        );
    }
}

fn connect_display(
    context: &InspectorContext,
    key: SelectedItem,
    target: Vec3Target,
    handles: [NumberPickerHandle; 3],
) {
    let project = context.project.clone();
    let player = context.player_state.clone();
    let handles = handles.map(|handle| handle.downgrade());
    let alive = Rc::downgrade(&context.listener_scope);
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector vec3 display",
        move || alive.upgrade().is_some(),
        move |event| {
            if !matches!(event, player_state::PlayerEvent::State(_)) {
                return;
            }
            let snapshot = player_state::snapshot(&player);
            let position = snapshot.position;
            let project = project.borrow();
            let Some(time) = crate::video::visual_local_time(&project, key.clone(), position)
            else {
                return;
            };
            let Some(value) = target
                .value(&project, key.clone())
                .map(|value| value.value_at(time))
            else {
                return;
            };
            for (axis, handle) in handles.iter().enumerate() {
                if let Some(handle) = handle.upgrade() {
                    handle.set_f64(value[axis] as f64);
                }
            }
        },
    );
}

fn toggle_keyframes(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: Vec3Target,
    enabled: bool,
) -> bool {
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(evaluation_time) = crate::video::visual_local_time(&project, key.clone(), position)
    else {
        return false;
    };
    let Some(keyframe_time) = project.keyframe_time(&key, position) else {
        return false;
    };
    let Some(value) = target.value_mut(&mut project, key.clone()) else {
        return false;
    };
    let current = value.value_at(evaluation_time);
    match (&mut value.base, enabled) {
        (TimelineBase::Const(_), false) | (TimelineBase::Keyframes(_), true) => return false,
        (base @ TimelineBase::Const(_), true) => {
            *base = TimelineBase::Keyframes(vec![Vec3::keyframe(keyframe_time, current)]);
        }
        (base @ TimelineBase::Keyframes(_), false) => *base = TimelineBase::Const(current),
    }
    commit_refresh(project, player, true);
    true
}

fn toggle_expression(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: Vec3Target,
    enabled: bool,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = target.value_mut(&mut project, key.clone()) else {
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
                source: "[x, y, z]".to_string(),
            });
            true
        }
        None => false,
    };
    if changed {
        commit_refresh(project, player, true);
    }
    changed
}

fn expression_editor(
    context: &InspectorContext,
    key: SelectedItem,
    target: Vec3Target,
) -> gtk::Widget {
    let source = target
        .value(&context.project.borrow(), key.clone())
        .and_then(|value| value.expression_source().map(str::to_string));
    let project = context.project.clone();
    let player = context.player_state.clone();
    crate::rhai_editor::editor(
        source,
        crate::rhai_editor::ExpressionValue::Vec3,
        move |source| {
            let mut project = project.borrow_mut();
            let Some(expression) = target
                .value_mut(&mut project, key.clone())
                .and_then(|value| value.expression.as_mut())
            else {
                return;
            };
            if expression.source == source {
                return;
            }
            expression.source = source;
            shrimply_project::project::commit_coalesced_edit(
                &project,
                "edit-scene-3d-vec3-expression",
            );
            drop(project);
            player_state::refresh_project(
                &player,
                ProjectChange {
                    video: true,
                    ..Default::default()
                },
            );
        },
    )
}

fn expression_output(
    context: &InspectorContext,
    key: SelectedItem,
    target: Vec3Target,
) -> gtk::Widget {
    let label = gtk::Label::builder()
        .hexpand(true)
        .xalign(1.0)
        .selectable(true)
        .css_classes(["numeric"])
        .build();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let title = gtk::Label::new(Some(tr!("Output").as_ref()));
    title.add_css_class("dim-label");
    row.append(&title);
    row.append(&label);
    let project = context.project.clone();
    let player = context.player_state.clone();
    let volume = context.volume.clone();
    let cache = Rc::new(RefCell::new(TransformExpressionCache::default()));
    let refresh: Rc<dyn Fn()> = Rc::new({
        let label = label.clone();
        move || {
            let snapshot = player_state::snapshot(&player);
            let position = snapshot.position;
            let project = project.borrow();
            let Some(item) = project.video_item(&key) else {
                return;
            };
            let Some(value) = target.value(&project, key.clone()) else {
                return;
            };
            let Some(expression) = value.expression.as_ref().filter(|value| value.enabled) else {
                return;
            };
            let volume_mixer = volume
                .borrow_mut()
                .sample(&project, position, snapshot.revision);
            let eval = shrimply_evaluation::TransformEvaluation::for_item_with_audio(
                &project,
                item,
                position,
                &volume_mixer,
            );
            let time = crate::video::visual_local_time(&project, key.clone(), position)
                .unwrap_or(Time::ZERO);
            let base = value.value_at(time);
            match cache.borrow_mut().eval_timeline_value_result(
                &eval,
                value.id,
                &expression.source,
                &base,
            ) {
                Ok(value) => label.set_label(&format!(
                    "X {:.2}  Y {:.2}  Z {:.2}",
                    value.x, value.y, value.z
                )),
                Err(error) => label.set_label(&format!("Invalid expression: {error}")),
            }
        }
    });
    refresh();
    let alive = Rc::downgrade(&context.listener_scope);
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector vec3 expression output",
        move || alive.upgrade().is_some(),
        move |event| {
            if matches!(event, player_state::PlayerEvent::State(_)) {
                refresh();
            }
        },
    );
    row.upcast()
}

fn actions(
    context: &InspectorContext,
    key: SelectedItem,
    target: Vec3Target,
) -> KeyframeEditorActions {
    let project = context.project.clone();
    let player = context.player_state.clone();
    KeyframeEditorActions {
        add_at_time: mutation(&project, &player, key.clone(), target, add_key),
        delete_at_time: mutation(&project, &player, key.clone(), target, delete_key),
        update_point: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |old, time, _| {
                update_point(&project, &player, key.clone(), target, old, time)
            })
        },
        copy_keyframes: {
            let key = key.clone();
            let project = project.clone();
            Rc::new(move |times| {
                target
                    .value(&project.borrow(), key.clone())
                    .and_then(|value| keyframe_model::copy_keyframes(value, times))
            })
        },
        paste_keyframes: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |clipboard, time| {
                let mut project = project.borrow_mut();
                let value = target.value_mut(&mut project, key.clone())?;
                let times = keyframe_model::paste_keyframes(value, clipboard, time)?;
                commit_refresh(project, &player, true);
                Some(times)
            })
        },
        set_interpolation: Some({
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |owner_id, interpolation| {
                set_interpolation(
                    &project,
                    &player,
                    key.clone(),
                    target,
                    owner_id,
                    interpolation,
                )
            })
        }),
        text_interpolation: None,
        toggle_playback: Rc::new(move || player_state::toggle_playing(&player)),
    }
}

fn mutation(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: Vec3Target,
    mutate: fn(&mut TimelineValue<Vec3>, Time, Time),
) -> Rc<dyn Fn(Time)> {
    let project = project.clone();
    let player = player.clone();
    Rc::new(move |time| {
        let mut project = project.borrow_mut();
        let frame_step = keyframe_editor::project_frame_step(&project);
        let Some(value) = target.value_mut(&mut project, key.clone()) else {
            return;
        };
        mutate(value, time, frame_step);
        commit_refresh(project, &player, true);
    })
}

fn add_key(value: &mut TimelineValue<Vec3>, time: Time, _: Time) {
    let current = value.value_at(time);
    if let TimelineBase::Keyframes(keyframes) = &mut value.base
        && !keyframes
            .iter()
            .any(|keyframe| keyframe.time.approx_eq(time))
    {
        insert_keyframe(keyframes, Vec3::keyframe(time, current));
    }
}

fn delete_key(value: &mut TimelineValue<Vec3>, time: Time, frame_step: Time) {
    let constant = if let TimelineBase::Keyframes(keyframes) = &mut value.base {
        keyframes
            .iter()
            .position(|keyframe| keyframe_model::same_frame(keyframe.time, time, frame_step))
            .map(|index| keyframes.remove(index).value)
            .filter(|_| keyframes.is_empty())
    } else {
        None
    };
    if let Some(constant) = constant {
        value.base = TimelineBase::Const(constant);
    }
}

fn update_point(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: Vec3Target,
    old: Time,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let Some(keyframes) = target
        .value_mut(&mut project, key.clone())
        .and_then(keyframes_mut)
    else {
        return;
    };
    let Some(index) = keyframes.iter().position(|value| value.time.approx_eq(old)) else {
        return;
    };
    keyframes[index].time = time;
    keyframes.sort_by_key(|value| value.time);
    shrimply_project::project::commit_coalesced_edit(&project, "edit-scene-3d-vec3");
    drop(project);
    player_state::refresh_project(
        player,
        keyframe_model::live_refresh(ProjectChange {
            video: true,
            ..Default::default()
        }),
    );
}

fn set_interpolation(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: Vec3Target,
    owner_id: uuid::Uuid,
    interpolation: Interpolation,
) {
    let mut project = project.borrow_mut();
    let Some(keyframes) = target
        .value_mut(&mut project, key.clone())
        .and_then(keyframes_mut)
    else {
        return;
    };
    let Some(keyframe) = keyframes.iter_mut().find(|value| value.id == owner_id) else {
        return;
    };
    if keyframe.interpolation_to_next == interpolation {
        return;
    }
    keyframe.interpolation_to_next = interpolation;
    shrimply_project::project::commit_edit(&project, "edit-scene-3d-vec3");
    drop(project);
    player_state::refresh_project(
        player,
        keyframe_model::live_refresh(ProjectChange {
            video: true,
            ..Default::default()
        }),
    );
}

fn keyframes_mut(
    value: &mut TimelineValue<Vec3>,
) -> Option<&mut Vec<TimelineVectorKeyframe<Vec3>>> {
    match &mut value.base {
        TimelineBase::Keyframes(keyframes) => Some(keyframes),
        TimelineBase::Const(_) => None,
    }
}

fn set_value(value: &mut TimelineValue<Vec3>, time: Time, next: Vec3) -> bool {
    match &mut value.base {
        TimelineBase::Const(current) if current.abs_diff_eq(next, 0.000_001) => false,
        TimelineBase::Const(current) => {
            *current = next;
            true
        }
        TimelineBase::Keyframes(keyframes) => {
            if let Some(keyframe) = keyframes
                .iter_mut()
                .find(|value| value.time.approx_eq(time))
            {
                keyframe.time = time;
                keyframe.value = next;
                keyframes.sort_by_key(|keyframe| keyframe.time);
            } else {
                insert_keyframe(keyframes, Vec3::keyframe(time, next));
            }
            true
        }
    }
}

fn insert_keyframe(
    keyframes: &mut Vec<TimelineVectorKeyframe<Vec3>>,
    mut next: TimelineVectorKeyframe<Vec3>,
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

fn local_time(context: &InspectorContext, key: SelectedItem) -> Option<Time> {
    crate::video::visual_local_time(
        &context.project.borrow(),
        key.clone(),
        player_state::snapshot(&context.player_state).position,
    )
}

fn visible_area(project: &Project, key: SelectedItem) -> Option<(Time, Time)> {
    crate::video::visual_visible_area(project, key)
}

fn selected_scene(project: &Project, key: SelectedItem) -> Option<&shrimply_scene_3d::ObjScene> {
    let item = project.video_item(&key)?;
    let shrimply_project::project::VideoItemContent::Obj(scene) = &item.content else {
        return None;
    };
    Some(scene)
}

fn selected_scene_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut shrimply_scene_3d::ObjScene> {
    let item = project.video_item_mut(&key)?;
    let shrimply_project::project::VideoItemContent::Obj(scene) = &mut item.content else {
        return None;
    };
    Some(scene)
}

fn commit_refresh(project: RefMut<'_, Project>, player: &SharedPlayerState, inspector: bool) {
    shrimply_project::project::commit_edit(&project, "edit-scene-3d-vec3");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector,
            ..Default::default()
        },
    );
}
