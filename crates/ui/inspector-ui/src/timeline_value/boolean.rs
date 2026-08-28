use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use shrimply_core::timeline_value::*;
use shrimply_evaluation::{FrameAudioAnalysis, TransformExpressionCache, VisualEvaluation};
use shrimply_project::project::{LayerVisibility, Project, Time, VideoItemContent};
use uuid::Uuid;

use crate::InspectedItem as SelectedItem;
use crate::keyframe_editor::{self, KeyframeEditorActions, KeyframeGraph, KeyframePoint};
use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::timeline_value::layered;
use crate::{InspectorContext, keyframe_model};

pub(crate) type BoolGet =
    for<'a> fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<TimelineBool>>;
pub(crate) type BoolGetMut =
    for<'a> fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<TimelineBool>>;

#[derive(Clone)]
pub(crate) enum BoolTarget {
    ItemValue {
        get: BoolGet,
        get_mut: BoolGetMut,
        scope: &'static str,
        mutated: fn(&mut Project, SelectedItem),
    },
    ItemVisibility,
    Background {
        value_id: Uuid,
    },
    Layer {
        id: Uuid,
        path: String,
    },
}

impl BoolTarget {
    fn get<'a>(
        &self,
        project: &'a Project,
        key: SelectedItem,
    ) -> Option<&'a TimelineValue<TimelineBool>> {
        let item = project.video_item(&key)?;
        match self {
            Self::ItemValue { get, .. } => get(project, key),
            Self::ItemVisibility => Some(&item.visibility),
            Self::Background { value_id } => {
                let VideoItemContent::Background(background) = &item.content else {
                    return None;
                };
                background.generator.boolean(*value_id)
            }
            Self::Layer { id, path, .. } => {
                let VideoItemContent::LayeredImage(image) = &item.content else {
                    return None;
                };
                image
                    .layers
                    .iter()
                    .find(|layer| layer.id == *id || layer.path == *path)?
                    .visibility
                    .as_ref()
            }
        }
    }

    fn get_mut<'a>(
        &self,
        project: &'a mut Project,
        key: SelectedItem,
        source_value: bool,
    ) -> Option<&'a mut TimelineValue<TimelineBool>> {
        match self {
            Self::ItemValue { get_mut, .. } => get_mut(project, key),
            Self::ItemVisibility => Some(&mut project.video_item_mut(&key)?.visibility),
            Self::Background { value_id } => {
                let item = project.video_item_mut(&key)?;
                let VideoItemContent::Background(background) = &mut item.content else {
                    return None;
                };
                background.generator.boolean_mut(*value_id)
            }
            Self::Layer { id, path } => {
                let item = project.video_item_mut(&key)?;
                let VideoItemContent::LayeredImage(image) = &mut item.content else {
                    return None;
                };
                let index = image
                    .layers
                    .iter()
                    .position(|layer| layer.id == *id || layer.path == *path)
                    .unwrap_or_else(|| {
                        image.layers.push(LayerVisibility {
                            id: *id,
                            path: path.clone(),
                            visibility: None,
                        });
                        image.layers.len() - 1
                    });
                Some(image.layers[index].visibility.get_or_insert_with(|| {
                    TimelineValue::<TimelineBool>::new_const(source_value.into())
                }))
            }
        }
    }

    fn scope(&self) -> String {
        match self {
            Self::ItemValue { scope, .. } => (*scope).to_string(),
            Self::ItemVisibility => "item".to_string(),
            Self::Background { value_id } => format!("background:{value_id}"),
            Self::Layer { path, .. } => format!("layered-image:{path}"),
        }
    }

    fn did_mutate(&self, project: &mut Project, key: SelectedItem) {
        if let Self::ItemValue { mutated, .. } = self {
            mutated(project, key);
        }
    }
}

pub(crate) fn bool_control(
    label: &str,
    value: &TimelineValue<TimelineBool>,
    source_value: bool,
    context: &InspectorContext,
    target: BoolTarget,
) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        return gtk::Switch::builder()
            .active(value.fallback().get())
            .build()
            .upcast();
    };
    let position = player_state::snapshot(&context.player_state).position;
    let local_time =
        crate::video::visual_local_time(&context.project.borrow(), key.clone(), position)
            .unwrap_or(Time::ZERO);
    let current = value.value_at(local_time).get();
    let keyframes_enabled = matches!(value.base, TimelineBase::Keyframes(_));
    let expression_enabled = value
        .expression
        .as_ref()
        .is_some_and(|expression| expression.enabled);
    let toggle = gtk::Switch::builder()
        .active(current)
        .valign(gtk::Align::Center)
        .build();
    let syncing_toggle = Rc::new(Cell::new(false));
    let project = context.project.clone();
    let player = context.player_state.clone();
    let target_for_toggle = target.clone();
    let syncing_toggle_for_signal = syncing_toggle.clone();
    let toggle_key = key.clone();
    toggle.connect_active_notify(move |toggle| {
        if syncing_toggle_for_signal.get() {
            return;
        }
        update_value(
            &project,
            &player,
            toggle_key.clone(),
            &target_for_toggle,
            source_value,
            toggle.is_active(),
        );
    });
    let toggle_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    toggle_row.set_hexpand(true);
    toggle_row.set_halign(gtk::Align::End);
    toggle_row.append(&toggle);

    let mut body = Vec::new();
    let mut graph_widget = None;
    if keyframes_enabled {
        let keys = match &value.base {
            TimelineBase::Keyframes(keys) => keys.as_slice(),
            TimelineBase::Const(_) => &[],
        };
        let graph = bool_graph(keys);
        let visible_area = bool_time_range(&context.project.borrow(), key.clone())
            .unwrap_or((Time::ZERO, Time::ZERO));
        let built = keyframe_editor::build(
            context,
            gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
            graph,
            visible_area,
            format!("bool:{}", target.scope()),
            actions(context, key.clone(), target.clone(), source_value),
        );
        graph_widget = Some(built.update_graph.clone());
        body.push(built.widget);
    }
    if expression_enabled {
        body.push(expression_editor(
            context,
            key.clone(),
            target.clone(),
            source_value,
        ));
    }

    connect_display_refresh(
        context,
        key.clone(),
        target.clone(),
        source_value,
        BoolDisplay {
            toggle: toggle.clone(),
            syncing_toggle,
            graph: graph_widget,
        },
    );

    let project = context.project.clone();
    let player = context.player_state.clone();
    let refresh = context.refresh.clone();
    let expression_project = context.project.clone();
    let expression_player = context.player_state.clone();
    let expression_refresh = context.refresh.clone();
    let expression_target = target.clone();
    let keyframe_key = key.clone();
    let expression_key = key;
    layered::control(
        label,
        value,
        toggle_row.upcast(),
        body,
        move |enabled| {
            if toggle_keyframes(
                &project,
                &player,
                keyframe_key.clone(),
                &target,
                source_value,
                enabled,
            ) {
                refresh();
            }
        },
        move |enabled| {
            if set_expression_enabled(
                &expression_project,
                &expression_player,
                expression_key.clone(),
                &expression_target,
                source_value,
                enabled,
            ) {
                expression_refresh();
            }
        },
    )
}

struct BoolDisplay {
    toggle: gtk::Switch,
    syncing_toggle: Rc<Cell<bool>>,
    graph: Option<Rc<dyn Fn(KeyframeGraph)>>,
}

fn connect_display_refresh(
    context: &InspectorContext,
    key: SelectedItem,
    target: BoolTarget,
    source_value: bool,
    display: BoolDisplay,
) {
    let project = context.project.clone();
    let player = context.player_state.clone();
    let toggle = display.toggle.downgrade();
    let graph = display.graph;
    let syncing_toggle = display.syncing_toggle;
    let alive = Rc::downgrade(&context.listener_scope);
    let alive_for_prune = alive.clone();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector bool display refresh",
        move || alive_for_prune.upgrade().is_some(),
        move |event| {
            if !matches!(
                event,
                player_state::PlayerEvent::State(_) | player_state::PlayerEvent::Project(_)
            ) || alive.upgrade().is_none()
            {
                return;
            }
            let Some(toggle) = toggle.upgrade() else {
                return;
            };
            let position = player_state::snapshot(&player).position;
            let project = project.borrow();
            let Some(local_time) = crate::video::visual_local_time(&project, key.clone(), position)
            else {
                return;
            };
            let current = target
                .get(&project, key.clone())
                .map_or(source_value, |value| {
                    if let Some(update_graph) = &graph
                        && let TimelineBase::Keyframes(keys) = &value.base
                    {
                        update_graph(bool_graph(keys));
                    }
                    value.value_at(local_time).get()
                });
            if toggle.is_active() != current {
                syncing_toggle.set(true);
                toggle.set_active(current);
                syncing_toggle.set(false);
            }
        },
    );
}

fn bool_graph(keys: &[TimelineStepKeyframe<TimelineBool>]) -> KeyframeGraph {
    KeyframeGraph::Step {
        points: keys
            .iter()
            .map(|key| KeyframePoint {
                time: key.time,
                value: 0.5,
            })
            .collect(),
    }
}

fn expression_editor(
    context: &InspectorContext,
    key: SelectedItem,
    target: BoolTarget,
    source_value: bool,
) -> gtk::Widget {
    let source = target
        .get(&context.project.borrow(), key.clone())
        .and_then(|value| value.expression_source().map(str::to_string));
    let project = context.project.clone();
    let player = context.player_state.clone();
    let volume = context.volume.clone();
    let feedback = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .build();
    feedback.add_css_class("caption");
    let feedback_cache = Rc::new(RefCell::new(TransformExpressionCache::default()));
    let feedback_target = target.clone();
    let feedback_project = project.clone();
    let feedback_player = player.clone();
    let feedback_volume = volume.clone();
    let feedback_label = feedback.clone();
    let feedback_key = key.clone();
    let refresh_feedback: Rc<dyn Fn()> = Rc::new(move || {
        let snapshot = player_state::snapshot(&feedback_player);
        let position = snapshot.position;
        let project = feedback_project.borrow();
        let volume_mixer =
            feedback_volume
                .borrow_mut()
                .sample(&project, position, snapshot.revision);
        feedback_label.set_label(&expression_feedback(
            &project,
            feedback_key.clone(),
            &feedback_target,
            source_value,
            position,
            &volume_mixer,
            &mut feedback_cache.borrow_mut(),
        ));
    });
    refresh_feedback();

    let refresh_after_edit = refresh_feedback.clone();
    let editor_key = key;
    let editor = crate::rhai_editor::editor(
        source,
        crate::rhai_editor::ExpressionValue::Bool,
        move |source| {
            update_expression_source(
                &project,
                &player,
                editor_key.clone(),
                &target,
                source_value,
                source,
            );
            refresh_after_edit();
        },
    );
    let alive = Rc::downgrade(&context.listener_scope);
    let alive_for_prune = alive.clone();
    let refresh_during_playback = refresh_feedback.clone();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector bool expression feedback",
        move || alive_for_prune.upgrade().is_some(),
        move |event| {
            if matches!(event, player_state::PlayerEvent::State(_)) && alive.upgrade().is_some() {
                refresh_during_playback();
            }
        },
    );

    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    body.append(&editor);
    body.append(&feedback);
    body.upcast()
}

fn expression_feedback(
    project: &Project,
    key: SelectedItem,
    target: &BoolTarget,
    source_value: bool,
    position: Time,
    audio_analysis: &FrameAudioAnalysis,
    cache: &mut TransformExpressionCache,
) -> String {
    let Some(evaluation_position) = project.timeline_time_to_sequence(&key.track(), position)
    else {
        return "Expression output unavailable".to_string();
    };
    let Some(item) = project.video_item(&key) else {
        return "Expression output unavailable".to_string();
    };
    let Some(local_time) = crate::video::visual_local_time(project, key.clone(), position) else {
        return "Expression output unavailable".to_string();
    };
    let Some(value) = target.get(project, key.clone()) else {
        return format!("Output: {source_value}");
    };
    let base = value.value_at(local_time);
    let Some(expression) = &value.expression else {
        return format!("Output: {}", base.get());
    };
    if let Some(error) = TransformExpressionCache::syntax_diagnostic(&expression.source) {
        return format!("Syntax error: {}", error.message);
    }
    if expression.source.trim().is_empty() {
        return format!("Output: {}", base.get());
    }
    let evaluation =
        VisualEvaluation::for_item_with_audio(project, item, evaluation_position, audio_analysis);
    match cache.eval_timeline_value_result(&evaluation, value.id, &expression.source, &base) {
        Ok(output) => format!("Output: {}", output.get()),
        Err(error) => format!("Evaluation error: {error}"),
    }
}

fn actions(
    context: &InspectorContext,
    key: SelectedItem,
    target: BoolTarget,
    source_value: bool,
) -> KeyframeEditorActions {
    let project = context.project.clone();
    let player = context.player_state.clone();
    let refresh_add = context.refresh.clone();
    let refresh_delete = context.refresh.clone();
    KeyframeEditorActions {
        playhead: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move || {
                bool_local_time(
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
                    player_state::seek_time(&player, global)
                }
            })
        },
        add_at_time: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            let target = target.clone();
            Rc::new(move |time| {
                add_key(&project, &player, key.clone(), &target, source_value, time);
                refresh_add();
            })
        },
        delete_at_time: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            let target = target.clone();
            Rc::new(move |time| {
                delete_key(&project, &player, key.clone(), &target, source_value, time);
                refresh_delete();
            })
        },
        update_point: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            let target = target.clone();
            Rc::new(move |old, time, _| {
                move_key(
                    &project,
                    &player,
                    key.clone(),
                    &target,
                    source_value,
                    old,
                    time,
                )
            })
        },
        copy_keyframes: {
            let key = key.clone();
            let project = project.clone();
            let target = target.clone();
            Rc::new(move |times| {
                target
                    .get(&project.borrow(), key.clone())
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
                let value = target.get_mut(&mut project, key.clone(), source_value)?;
                let times = keyframe_model::paste_keyframes(value, clipboard, time, frame_step)?;
                target.did_mutate(&mut project, key.clone());
                shrimply_project::project::commit_edit(&project, "paste-bool-keyframes");
                drop(project);
                player_state::refresh_project(
                    &player,
                    ProjectChange {
                        video: true,
                        inspector: true,
                        ..Default::default()
                    },
                );
                Some(times)
            })
        },
        set_interpolation: None,
        text_interpolation: None,
        toggle_playback: Rc::new(move || player_state::toggle_playing(&player)),
    }
}

fn update_value(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &BoolTarget,
    source_value: bool,
    next: bool,
) {
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let time = bool_local_time(&project, key.clone(), position).unwrap_or(Time::ZERO);
    let frame_step = keyframe_editor::project_frame_step(&project);
    let time = keyframe_model::snap_to_frame(time, frame_step);
    let Some(value) = target.get_mut(&mut project, key.clone(), source_value) else {
        return;
    };
    match &mut value.base {
        TimelineBase::Const(value) => *value = next.into(),
        TimelineBase::Keyframes(keys) => set_key(keys, time, frame_step, next.into()),
    }
    target.did_mutate(&mut project, key);
    shrimply_project::project::commit_edit(&project, "visual-bool");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn toggle_keyframes(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &BoolTarget,
    source_value: bool,
    enabled: bool,
) -> bool {
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let time = bool_local_time(&project, key.clone(), position).unwrap_or(Time::ZERO);
    let time = keyframe_model::snap_to_frame(time, keyframe_editor::project_frame_step(&project));
    let Some(value) = target.get_mut(&mut project, key.clone(), source_value) else {
        return false;
    };
    let current = value.value_at(time);
    match (&mut value.base, enabled) {
        (TimelineBase::Const(_), false) | (TimelineBase::Keyframes(_), true) => return false,
        (base @ TimelineBase::Const(_), true) => {
            *base = TimelineBase::Keyframes(vec![new_key(time, current)])
        }
        (base @ TimelineBase::Keyframes(_), false) => *base = TimelineBase::Const(current),
    }
    target.did_mutate(&mut project, key);
    shrimply_project::project::commit_edit(&project, "visual-bool-keyframes");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    true
}

fn set_expression_enabled(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &BoolTarget,
    source_value: bool,
    enabled: bool,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = target.get_mut(&mut project, key.clone(), source_value) else {
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
    if !changed {
        return false;
    }
    target.did_mutate(&mut project, key);
    shrimply_project::project::commit_edit(&project, "visual-bool-expression");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    true
}

fn update_expression_source(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &BoolTarget,
    source_value: bool,
    source: String,
) {
    let mut project = project.borrow_mut();
    let Some(value) = target.get_mut(&mut project, key.clone(), source_value) else {
        return;
    };
    let Some(expression) = &mut value.expression else {
        return;
    };
    if expression.source == source {
        return;
    }
    expression.source = source;
    target.did_mutate(&mut project, key);
    shrimply_project::project::commit_coalesced_edit(&project, "visual-bool-expression");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: false,
            ..Default::default()
        },
    );
}

fn add_key(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &BoolTarget,
    source: bool,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let step = keyframe_editor::project_frame_step(&project);
    let Some(value) = target.get_mut(&mut project, key.clone(), source) else {
        return;
    };
    let current = value.value_at(time);
    let TimelineBase::Keyframes(keys) = &mut value.base else {
        return;
    };
    set_key(keys, time, step, current);
    target.did_mutate(&mut project, key);
    shrimply_project::project::commit_edit(&project, "add-bool-keyframe");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn delete_key(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &BoolTarget,
    source: bool,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let step = keyframe_editor::project_frame_step(&project);
    let Some(value) = target.get_mut(&mut project, key.clone(), source) else {
        return;
    };
    let TimelineBase::Keyframes(keys) = &mut value.base else {
        return;
    };
    let Some(index) = keys
        .iter()
        .position(|item| keyframe_model::same_frame(item.time, time, step))
    else {
        return;
    };
    let removed = keys.remove(index).value;
    let constant = keys.is_empty().then_some(removed);
    if let Some(constant) = constant {
        value.base = TimelineBase::Const(constant);
    }
    target.did_mutate(&mut project, key);
    shrimply_project::project::commit_edit(&project, "delete-bool-keyframe");
    drop(project);
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn move_key(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    target: &BoolTarget,
    source: bool,
    old: Time,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let Some(value) = target.get_mut(&mut project, key.clone(), source) else {
        return;
    };
    if let TimelineBase::Keyframes(keys) = &mut value.base
        && let Some(item) = keys.iter_mut().find(|item| item.time.approx_eq(old))
    {
        item.time = time;
        keys.sort_by_key(|item| item.time);
        target.did_mutate(&mut project, key);
    }
    drop(project);
    player_state::refresh_project(
        player,
        keyframe_model::live_refresh(ProjectChange {
            video: true,
            ..Default::default()
        }),
    );
}

fn set_key(
    keys: &mut Vec<TimelineStepKeyframe<TimelineBool>>,
    time: Time,
    step: Time,
    value: TimelineBool,
) {
    if let Some(key) = keys
        .iter_mut()
        .find(|key| keyframe_model::same_frame(key.time, time, step))
    {
        key.time = time;
        key.value = value;
    } else {
        keys.push(new_key(time, value));
        keys.sort_by_key(|key| key.time);
    }
}

fn new_key(time: Time, value: TimelineBool) -> TimelineStepKeyframe<TimelineBool> {
    TimelineStepKeyframe::<TimelineBool> {
        id: Uuid::new_v4(),
        time,
        value,
    }
}

fn bool_local_time(project: &Project, key: SelectedItem, position: Time) -> Option<Time> {
    crate::video::visual_local_time(project, key, position)
}

fn bool_time_range(project: &Project, key: SelectedItem) -> Option<(Time, Time)> {
    crate::video::visual_visible_area(project, key)
}
