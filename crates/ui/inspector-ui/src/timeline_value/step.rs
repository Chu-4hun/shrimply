use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use crate::InspectedItem as SelectedItem;
use gtk::prelude::Cast;
use shrimply_core::timeline_value::{
    TimelineBase, TimelineExpression, TimelineStep, TimelineStepKeyframe, TimelineValue,
};
use shrimply_project::project::{Project, Time};
use uuid::Uuid;

use crate::{
    InspectorContext,
    keyframe_editor::{self, KeyframeEditorActions, KeyframeGraph, KeyframePoint},
    keyframe_model,
    player_state::{self, ProjectChange, SharedPlayerState},
    selector::{step_button_editor, step_editor},
};

type Get<T> = dyn for<'a> Fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<T>>;
type GetMut<T> = dyn for<'a> Fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<T>>;

pub(crate) struct StepTarget<T: TimelineStep> {
    get: Rc<Get<T>>,
    get_mut: Rc<GetMut<T>>,
    commit_name: &'static str,
    refresh: ProjectChange,
    refresh_inspector_on_value_change: bool,
    mutated: Option<fn(&mut Project, SelectedItem)>,
}

impl<T: TimelineStep> Clone for StepTarget<T> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            get_mut: self.get_mut.clone(),
            commit_name: self.commit_name,
            refresh: self.refresh,
            refresh_inspector_on_value_change: self.refresh_inspector_on_value_change,
            mutated: self.mutated,
        }
    }
}

impl<T: TimelineStep> StepTarget<T> {
    pub(crate) fn new(
        get: impl for<'a> Fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<T>> + 'static,
        get_mut: impl for<'a> Fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<T>>
        + 'static,
        commit_name: &'static str,
        refresh: ProjectChange,
    ) -> Self {
        Self {
            get: Rc::new(get),
            get_mut: Rc::new(get_mut),
            commit_name,
            refresh,
            refresh_inspector_on_value_change: false,
            mutated: None,
        }
    }

    pub(crate) fn refresh_inspector_on_value_change(mut self) -> Self {
        self.refresh_inspector_on_value_change = true;
        self
    }

    pub(crate) fn mark_mutated(mut self, mutated: fn(&mut Project, SelectedItem)) -> Self {
        self.mutated = Some(mutated);
        self
    }

    fn did_mutate(&self, project: &mut Project, key: SelectedItem) {
        if let Some(mutated) = self.mutated {
            mutated(project, key);
        }
    }
}

pub(crate) fn step_control<T: TimelineStep>(
    label: &str,
    value: &TimelineValue<T>,
    context: &InspectorContext,
    target: StepTarget<T>,
) -> gtk::Widget {
    step_control_with_buttons(label, value, context, target, false)
}

pub(crate) fn step_button_control<T: TimelineStep>(
    label: &str,
    value: &TimelineValue<T>,
    context: &InspectorContext,
    target: StepTarget<T>,
) -> gtk::Widget {
    step_control_with_buttons(label, value, context, target, true)
}

fn step_control_with_buttons<T: TimelineStep>(
    label: &str,
    value: &TimelineValue<T>,
    context: &InspectorContext,
    target: StepTarget<T>,
    buttons: bool,
) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        return super::layered::control(
            label,
            value,
            if buttons {
                step_button_editor(value.fallback(), |_| {})
            } else {
                step_editor(value.fallback(), |_| {})
            },
            Vec::new(),
            |_| {},
            |_| {},
        );
    };
    let position = player_state::snapshot(&context.player_state).position;
    let time = crate::video::visual_local_time(&context.project.borrow(), key.clone(), position)
        .unwrap_or(Time::ZERO);
    let project = context.project.clone();
    let player = context.player_state.clone();
    let update_target = target.clone();
    let editor_key = key.clone();
    let update = Rc::new(move |next| {
        update_value(&project, &player, editor_key.clone(), &update_target, next)
    });
    let editor = if buttons {
        let update = update.clone();
        step_button_editor(value.value_at(time), move |next| update(next))
    } else {
        step_editor(value.value_at(time), move |next| update(next))
    };

    let mut body = Vec::new();
    if let TimelineBase::Keyframes(_) = &value.base {
        let built = keyframe_editor::build(
            context,
            gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
            step_graph(value),
            visible_area(&context.project.borrow(), key.clone())
                .unwrap_or((Time::ZERO, Time::ZERO)),
            format!("step:{}:{}", target.commit_name, value.id),
            actions(context, key.clone(), target.clone()),
        );
        let project = context.project.clone();
        let graph_target = target.clone();
        let graph_key = key.clone();
        keyframe_editor::connect_graph_refresh(
            context,
            "inspector step keyframe graph refresh",
            built.update_graph.clone(),
            move || (graph_target.get)(&project.borrow(), graph_key.clone()).map(step_graph),
        );
        body.push(built.widget);
    }
    if value
        .expression
        .as_ref()
        .is_some_and(|expression| expression.enabled)
    {
        let project = context.project.clone();
        let player = context.player_state.clone();
        let expression_target = target.clone();
        let expression_key = key.clone();
        body.push(crate::rhai_editor::editor(
            value.expression_source().map(str::to_string),
            crate::rhai_editor::ExpressionValue::Step,
            move |source| {
                update_expression(
                    &project,
                    &player,
                    expression_key.clone(),
                    &expression_target,
                    source,
                )
            },
        ));
    }

    let project = context.project.clone();
    let player = context.player_state.clone();
    let refresh = context.refresh.clone();
    let expression_project = context.project.clone();
    let expression_player = context.player_state.clone();
    let expression_refresh = context.refresh.clone();
    let expression_target = target.clone();
    let keyframe_key = key.clone();
    let expression_key = key;
    super::layered::control(
        label,
        value,
        editor,
        body,
        move |enabled| {
            if toggle_keyframes(&project, &player, keyframe_key.clone(), &target, enabled) {
                refresh();
            }
        },
        move |enabled| {
            if toggle_expression(
                &expression_project,
                &expression_player,
                expression_key.clone(),
                &expression_target,
                enabled,
            ) {
                expression_refresh();
            }
        },
    )
}

fn step_graph<T: TimelineStep>(value: &TimelineValue<T>) -> KeyframeGraph {
    let points = match &value.base {
        TimelineBase::Const(_) => Vec::new(),
        TimelineBase::Keyframes(keyframes) => keyframes
            .iter()
            .map(|keyframe| KeyframePoint {
                time: keyframe.time,
                value: 0.5,
            })
            .collect(),
    };
    KeyframeGraph::Step { points }
}

fn actions<T: TimelineStep>(
    context: &InspectorContext,
    key: SelectedItem,
    target: StepTarget<T>,
) -> KeyframeEditorActions {
    let project = context.project.clone();
    let player = context.player_state.clone();
    KeyframeEditorActions {
        playhead: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move || {
                crate::video::visual_local_time(
                    &project.borrow(),
                    key.clone(),
                    player_state::snapshot(&player).position,
                )
                .unwrap_or(Time::ZERO)
            })
        },
        select_time: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |time| {
                if let Some(global) =
                    crate::video::visual_global_time(&project.borrow(), key.clone(), time)
                {
                    player_state::set_position(&player, global);
                }
            })
        },
        add_at_time: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            let target = target.clone();
            Rc::new(move |time| add_key(&project, &player, key.clone(), &target, time))
        },
        delete_at_time: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            let target = target.clone();
            Rc::new(move |time| delete_key(&project, &player, key.clone(), &target, time))
        },
        update_point: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            let target = target.clone();
            Rc::new(move |old, time, value| {
                move_key(&project, &player, key.clone(), &target, old, time, value)
            })
        },
        copy_keyframes: {
            let key = key.clone();
            let project = project.clone();
            let target = target.clone();
            Rc::new(move |times| {
                (target.get)(&project.borrow(), key.clone())
                    .and_then(|value| keyframe_model::copy_keyframes(value, times))
            })
        },
        paste_keyframes: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            let target = target.clone();
            Rc::new(move |clipboard, time| {
                let mut project = project.borrow_mut();
                let frame_step = keyframe_editor::project_frame_step(&project);
                let value = (target.get_mut)(&mut project, key.clone())?;
                let times = keyframe_model::paste_keyframes(value, clipboard, time, frame_step)?;
                target.did_mutate(&mut project, key.clone());
                commit_and_refresh(project, &player, &target, true);
                Some(times)
            })
        },
        set_interpolation: None,
        text_interpolation: None,
        toggle_playback: Rc::new(move || player_state::toggle_playing(&player)),
    }
}

fn update_value<T: TimelineStep>(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &StepTarget<T>,
    next: T,
) {
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(time) = crate::video::visual_local_time(&project, key.clone(), position) else {
        return;
    };
    let step = keyframe_editor::project_frame_step(&project);
    let time = keyframe_model::snap_to_frame(time, step);
    let Some(value) = (target.get_mut)(&mut project, key.clone()) else {
        return;
    };
    match &mut value.base {
        TimelineBase::Const(value) => *value = next,
        TimelineBase::Keyframes(keyframes) => set_key(keyframes, time, step, next),
    }
    target.did_mutate(&mut project, key);
    commit_and_refresh(
        project,
        player,
        target,
        target.refresh_inspector_on_value_change,
    );
}

fn toggle_keyframes<T: TimelineStep>(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &StepTarget<T>,
    enabled: bool,
) -> bool {
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(time) = crate::video::visual_local_time(&project, key.clone(), position) else {
        return false;
    };
    let Some(value) = (target.get_mut)(&mut project, key.clone()) else {
        return false;
    };
    let current = value.value_at(time);
    match (&mut value.base, enabled) {
        (TimelineBase::Const(_), false) | (TimelineBase::Keyframes(_), true) => return false,
        (base @ TimelineBase::Const(_), true) => {
            *base = TimelineBase::Keyframes(vec![T::keyframe(time, current)]);
        }
        (base @ TimelineBase::Keyframes(_), false) => *base = TimelineBase::Const(current),
    }
    target.did_mutate(&mut project, key);
    commit_and_refresh(project, player, target, true);
    true
}

fn toggle_expression<T: TimelineStep>(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &StepTarget<T>,
    enabled: bool,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = (target.get_mut)(&mut project, key.clone()) else {
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
                id: Uuid::new_v4(),
                enabled: true,
                source: "value".to_string(),
            });
            true
        }
        None => false,
    };
    if changed {
        target.did_mutate(&mut project, key);
        commit_and_refresh(project, player, target, true);
    }
    changed
}

fn update_expression<T: TimelineStep>(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &StepTarget<T>,
    source: String,
) {
    let mut project = project.borrow_mut();
    let Some(expression) =
        (target.get_mut)(&mut project, key.clone()).and_then(|value| value.expression.as_mut())
    else {
        return;
    };
    if expression.source == source {
        return;
    }
    expression.source = source;
    target.did_mutate(&mut project, key);
    commit_and_refresh(project, player, target, false);
}

fn add_key<T: TimelineStep>(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &StepTarget<T>,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let frame_step = keyframe_editor::project_frame_step(&project);
    let Some(value) = (target.get_mut)(&mut project, key.clone()) else {
        return;
    };
    let current = value.value_at(time);
    if let TimelineBase::Keyframes(keyframes) = &mut value.base {
        set_key(keyframes, time, frame_step, current);
        target.did_mutate(&mut project, key);
        commit_and_refresh(project, player, target, true);
    }
}

fn delete_key<T: TimelineStep>(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &StepTarget<T>,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let frame_step = keyframe_editor::project_frame_step(&project);
    let Some(value) = (target.get_mut)(&mut project, key.clone()) else {
        return;
    };
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
    target.did_mutate(&mut project, key);
    commit_and_refresh(project, player, target, true);
}

fn move_key<T: TimelineStep>(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &StepTarget<T>,
    old: Time,
    time: Time,
    _graph_value: f64,
) {
    let mut project = project.borrow_mut();
    let Some(value) = (target.get_mut)(&mut project, key.clone()) else {
        return;
    };
    if let TimelineBase::Keyframes(keyframes) = &mut value.base
        && let Some(keyframe) = keyframes
            .iter_mut()
            .find(|keyframe| keyframe.time.approx_eq(old))
    {
        keyframe.time = time;
        keyframes.sort_by_key(|keyframe| keyframe.time);
        target.did_mutate(&mut project, key);
        shrimply_project::project::commit_coalesced_edit(&project, target.commit_name);
        drop(project);
        player_state::refresh_project(player, keyframe_model::live_refresh(target.refresh));
    }
}

fn set_key<T: TimelineStep>(
    keyframes: &mut Vec<TimelineStepKeyframe<T>>,
    time: Time,
    frame_step: Time,
    value: T,
) {
    if let Some(keyframe) = keyframes
        .iter_mut()
        .find(|keyframe| keyframe_model::same_frame(keyframe.time, time, frame_step))
    {
        keyframe.time = time;
        keyframe.value = value;
    } else {
        keyframes.push(T::keyframe(time, value));
        keyframes.sort_by_key(|keyframe| keyframe.time);
    }
}

fn visible_area(project: &Project, key: SelectedItem) -> Option<(Time, Time)> {
    crate::video::visual_visible_area(project, key)
}

fn commit_and_refresh<T: TimelineStep>(
    project: RefMut<'_, Project>,
    player: &SharedPlayerState,
    target: &StepTarget<T>,
    inspector: bool,
) {
    shrimply_project::project::commit_edit(&project, target.commit_name);
    drop(project);
    let mut refresh = target.refresh;
    refresh.inspector = inspector;
    player_state::refresh_project(player, refresh);
}
