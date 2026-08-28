use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use shrimply_evaluation::{FrameAudioAnalysis, TransformExpressionCache, VisualEvaluation};
use shrimply_project::project::{Project, TextItem, Time, VideoItemContent};
use shrimply_ui_foundation::ui::MultilineTextInput;
use uuid::Uuid;

use crate::InspectedItem as SelectedItem;
use crate::keyframe_editor::{
    self, KeyframeEditorActions, KeyframeGraph, SpeedSegment, TextInterpolationActions,
};
use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::{InspectorContext, keyframe_model};

use super::{
    Interpolation, TextInterpolation, TimelineBase, TimelineExpression, TimelineKeyframe,
    TimelineTextKeyframe, TimelineValue, TimelineValueType, layered, text_edit_count,
};

pub(crate) fn text_control(
    label: &str,
    value: &TimelineValue<String>,
    context: &InspectorContext,
) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        let input = MultilineTextInput::builder(value.fallback())
            .min_content_height(86)
            .build();
        return layered::wide_control(
            label,
            value,
            input.widget().clone(),
            Vec::new(),
            |_| {},
            |_| {},
        );
    };
    let position = player_state::snapshot(&context.player_state).position;
    let time = local_time(&context.project.borrow(), key.clone(), position).unwrap_or(Time::ZERO);
    let project = context.project.clone();
    let player = context.player_state.clone();
    let commit_project = context.project.clone();
    let input_key = key.clone();
    let input = MultilineTextInput::builder(value.value_at(time))
        .min_content_height(86)
        .on_change(move |text| update_value(&project, &player, input_key.clone(), text))
        .on_commit(move || {
            shrimply_project::project::commit_edit(&commit_project.borrow(), "edit-text-item");
        })
        .build();

    let mut body = Vec::new();
    if matches!(value.base, TimelineBase::Keyframes(_)) {
        let built = keyframe_editor::build(
            context,
            gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
            text_graph(value),
            visible_area(&context.project.borrow(), key.clone())
                .unwrap_or((Time::ZERO, Time::ZERO)),
            format!("text:{}", value.id),
            actions(context, key.clone()),
        );
        let graph_project = context.project.clone();
        let graph_key = key.clone();
        keyframe_editor::connect_graph_refresh(
            context,
            "inspector text keyframe graph refresh",
            built.update_graph.clone(),
            move || text_value(&graph_project.borrow(), graph_key.clone()).map(text_graph),
        );
        body.push(built.widget);
    }
    if value
        .expression
        .as_ref()
        .is_some_and(|expression| expression.enabled)
    {
        body.push(expression_editor(context, key.clone()));
    }

    connect_text_refresh(context, key.clone(), input.set_text_handler());

    let project = context.project.clone();
    let player = context.player_state.clone();
    let refresh = context.refresh.clone();
    let expression_project = context.project.clone();
    let expression_player = context.player_state.clone();
    let expression_refresh = context.refresh.clone();
    let keyframe_key = key.clone();
    let expression_key = key;
    layered::wide_control(
        label,
        value,
        input.widget().clone(),
        body,
        move |enabled| {
            if toggle_keyframes(&project, &player, keyframe_key.clone(), enabled) {
                refresh();
            }
        },
        move |enabled| {
            if toggle_expression(
                &expression_project,
                &expression_player,
                expression_key.clone(),
                enabled,
            ) {
                expression_refresh();
            }
        },
    )
}

fn connect_text_refresh(context: &InspectorContext, key: SelectedItem, set_text: Rc<dyn Fn(&str)>) {
    let project = context.project.clone();
    let player = context.player_state.clone();
    let alive = Rc::downgrade(&context.listener_scope);
    let alive_for_prune = alive.clone();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector text display refresh",
        move || alive_for_prune.upgrade().is_some(),
        move |event| {
            if !matches!(
                event,
                player_state::PlayerEvent::State(_) | player_state::PlayerEvent::Project(_)
            ) || alive.upgrade().is_none()
            {
                return;
            }
            let position = player_state::snapshot(&player).position;
            let project = project.borrow();
            let Some(time) = local_time(&project, key.clone(), position) else {
                return;
            };
            if let Some(value) = text_value(&project, key.clone()) {
                set_text(&value.value_at(time));
            }
        },
    );
}

fn expression_editor(context: &InspectorContext, key: SelectedItem) -> gtk::Widget {
    let source = text_value(&context.project.borrow(), key.clone())
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
    let cache = Rc::new(RefCell::new(TransformExpressionCache::default()));
    let feedback_project = project.clone();
    let feedback_player = player.clone();
    let feedback_volume = volume.clone();
    let feedback_label = feedback.clone();
    let feedback_key = key.clone();
    let refresh_feedback: Rc<dyn Fn()> = Rc::new(move || {
        let snapshot = player_state::snapshot(&feedback_player);
        let project = feedback_project.borrow();
        let audio =
            feedback_volume
                .borrow_mut()
                .sample(&project, snapshot.position, snapshot.revision);
        feedback_label.set_label(&expression_feedback(
            &project,
            feedback_key.clone(),
            snapshot.position,
            &audio,
            &mut cache.borrow_mut(),
        ));
    });
    refresh_feedback();

    let refresh_after_edit = refresh_feedback.clone();
    let editor_key = key;
    let editor = crate::rhai_editor::editor(
        source,
        crate::rhai_editor::ExpressionValue::Text,
        move |source| {
            update_expression(&project, &player, editor_key.clone(), source);
            refresh_after_edit();
        },
    );
    let alive = Rc::downgrade(&context.listener_scope);
    let alive_for_prune = alive.clone();
    player_state::connect_while_alive_named(
        &context.player_state,
        "inspector text expression feedback",
        move || alive_for_prune.upgrade().is_some(),
        move |event| {
            if matches!(event, player_state::PlayerEvent::State(_)) && alive.upgrade().is_some() {
                refresh_feedback();
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
    position: Time,
    audio: &FrameAudioAnalysis,
    cache: &mut TransformExpressionCache,
) -> String {
    let Some(evaluation_position) = project.timeline_time_to_sequence(&key.track(), position)
    else {
        return tr!("Expression output unavailable").into_owned();
    };
    let Some(item) = project.video_item(&key) else {
        return tr!("Expression output unavailable").into_owned();
    };
    let Some(time) = local_time(project, key.clone(), position) else {
        return tr!("Expression output unavailable").into_owned();
    };
    let Some(value) = text_value(project, key.clone()) else {
        return tr!("Expression output unavailable").into_owned();
    };
    let base = value.value_at(time);
    let Some(expression) = &value.expression else {
        return shrimply_ui_foundation::i18n::text_args("Output: %{output}", &[("output", base)]);
    };
    if let Some(error) = TransformExpressionCache::syntax_diagnostic(&expression.source) {
        return format!("Syntax error: {}", error.message);
    }
    if expression.source.trim().is_empty() {
        return shrimply_ui_foundation::i18n::text_args("Output: %{output}", &[("output", base)]);
    }
    let evaluation =
        VisualEvaluation::for_item_with_audio(project, item, evaluation_position, audio);
    match cache.eval_timeline_value_result(&evaluation, value.id, &expression.source, &base) {
        Ok(output) => {
            shrimply_ui_foundation::i18n::text_args("Output: %{output}", &[("output", output)])
        }
        Err(error) => format!("Evaluation error: {error}"),
    }
}

fn text_graph(value: &TimelineValue<String>) -> KeyframeGraph {
    let TimelineBase::Keyframes(keyframes) = &value.base else {
        return KeyframeGraph::Speed {
            segments: Vec::new(),
            keys: Vec::new(),
            static_value: 0.0,
        };
    };
    KeyframeGraph::Speed {
        segments: keyframes
            .windows(2)
            .filter_map(|pair| {
                let seconds = pair[1].time.signed_sub(pair[0].time).as_secs_f64();
                (seconds > f64::EPSILON).then(|| SpeedSegment {
                    owner_id: pair[0].id,
                    start: pair[0].time,
                    end: pair[1].time,
                    value: text_edit_count(
                        &pair[0].value,
                        &pair[1].value,
                        pair[0].text_interpolation_to_next,
                    ) as f64
                        / seconds,
                    interpolation: pair[0].interpolation_to_next,
                })
            })
            .collect(),
        keys: keyframes.iter().map(|keyframe| keyframe.time).collect(),
        static_value: 0.0,
    }
}

fn actions(context: &InspectorContext, key: SelectedItem) -> KeyframeEditorActions {
    let project = context.project.clone();
    let player = context.player_state.clone();
    KeyframeEditorActions {
        playhead: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move || {
                local_time(
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
                if let Some(position) = global_time(&project.borrow(), key.clone(), time) {
                    player_state::seek_time(&player, position);
                }
            })
        },
        add_at_time: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |time| add_key(&project, &player, key.clone(), time))
        },
        delete_at_time: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |time| delete_key(&project, &player, key.clone(), time))
        },
        update_point: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |old, time, _| move_key(&project, &player, key.clone(), old, time))
        },
        copy_keyframes: {
            let key = key.clone();
            let project = project.clone();
            Rc::new(move |times| {
                text_value(&project.borrow(), key.clone())
                    .and_then(|value| keyframe_model::copy_keyframes(value, times))
            })
        },
        paste_keyframes: {
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |clipboard, time| {
                let mut project = project.borrow_mut();
                let frame_step = keyframe_editor::project_frame_step(&project);
                let value = text_value_mut(&mut project, key.clone())?;
                let times = keyframe_model::paste_keyframes(value, clipboard, time, frame_step)?;
                shrimply_project::project::commit_edit(&project, "paste-text-keyframes");
                drop(project);
                refresh(&player, true);
                Some(times)
            })
        },
        set_interpolation: Some({
            let key = key.clone();
            let project = project.clone();
            let player = player.clone();
            Rc::new(move |id, interpolation| {
                set_interpolation(&project, &player, key.clone(), id, interpolation)
            })
        }),
        text_interpolation: Some(TextInterpolationActions {
            get: {
                let key = key.clone();
                let project = project.clone();
                Rc::new(move |id| {
                    let project = project.borrow();
                    let TimelineBase::Keyframes(keyframes) =
                        &text_value(&project, key.clone())?.base
                    else {
                        return None;
                    };
                    keyframes
                        .iter()
                        .find(|keyframe| keyframe.id == id)
                        .map(|keyframe| keyframe.text_interpolation_to_next)
                })
            },
            set: {
                let key = key.clone();
                let project = project.clone();
                let player = player.clone();
                Rc::new(move |id, mode| {
                    set_text_interpolation(&project, &player, key.clone(), id, mode)
                })
            },
        }),
        toggle_playback: Rc::new(move || player_state::toggle_playing(&player)),
    }
}

fn update_value(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    next: String,
) -> bool {
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(time) = local_time(&project, key.clone(), position) else {
        return false;
    };
    let step = keyframe_editor::project_frame_step(&project);
    let time = keyframe_model::snap_to_frame(time, step);
    let Some(value) = text_value_mut(&mut project, key.clone()) else {
        return false;
    };
    let changed = match &mut value.base {
        TimelineBase::Const(value) if *value != next => {
            *value = next;
            true
        }
        TimelineBase::Keyframes(keyframes) => set_key(keyframes, time, step, next),
        TimelineBase::Const(_) => false,
    };
    drop(project);
    if changed {
        refresh(player, false);
    }
    changed
}

fn toggle_keyframes(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    enabled: bool,
) -> bool {
    let position = player_state::snapshot(player).position;
    let mut project = project.borrow_mut();
    let Some(time) = local_time(&project, key.clone(), position) else {
        return false;
    };
    let time = keyframe_model::snap_to_frame(time, keyframe_editor::project_frame_step(&project));
    let Some(value) = text_value_mut(&mut project, key.clone()) else {
        return false;
    };
    let current = value.value_at(time);
    match (&mut value.base, enabled) {
        (TimelineBase::Const(_), false) | (TimelineBase::Keyframes(_), true) => return false,
        (base @ TimelineBase::Const(_), true) => {
            *base = TimelineBase::Keyframes(vec![String::keyframe(time, current)]);
        }
        (base @ TimelineBase::Keyframes(_), false) => *base = TimelineBase::Const(current),
    }
    shrimply_project::project::commit_edit(&project, "text-keyframes");
    drop(project);
    refresh(player, true);
    true
}

fn toggle_expression(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    enabled: bool,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = text_value_mut(&mut project, key.clone()) else {
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
    shrimply_project::project::commit_edit(&project, "text-expression");
    drop(project);
    refresh(player, true);
    true
}

fn update_expression(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    source: String,
) {
    let mut project = project.borrow_mut();
    let Some(expression) =
        text_value_mut(&mut project, key.clone()).and_then(|value| value.expression.as_mut())
    else {
        return;
    };
    if expression.source == source {
        return;
    }
    expression.source = source;
    shrimply_project::project::commit_coalesced_edit(&project, "text-expression");
    drop(project);
    refresh(player, false);
}

fn add_key(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let step = keyframe_editor::project_frame_step(&project);
    let Some(value) = text_value_mut(&mut project, key.clone()) else {
        return;
    };
    let current = value.value_at(time);
    if let TimelineBase::Keyframes(keyframes) = &mut value.base {
        set_key(keyframes, time, step, current);
    }
    shrimply_project::project::commit_edit(&project, "add-text-keyframe");
    drop(project);
    refresh(player, true);
}

fn delete_key(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    time: Time,
) {
    let mut project = project.borrow_mut();
    let step = keyframe_editor::project_frame_step(&project);
    let Some(value) = text_value_mut(&mut project, key.clone()) else {
        return;
    };
    let constant = if let TimelineBase::Keyframes(keyframes) = &mut value.base {
        keyframes
            .iter()
            .position(|item| keyframe_model::same_frame(item.time, time, step))
            .map(|index| keyframes.remove(index).value)
            .filter(|_| keyframes.is_empty())
    } else {
        None
    };
    if let Some(constant) = constant {
        value.base = TimelineBase::Const(constant);
    }
    shrimply_project::project::commit_edit(&project, "delete-text-keyframe");
    drop(project);
    refresh(player, true);
}

fn move_key(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    old: Time,
    time: Time,
) {
    let mut project = project.borrow_mut();
    if let Some(value) = text_value_mut(&mut project, key.clone())
        && let TimelineBase::Keyframes(keyframes) = &mut value.base
        && let Some(item) = keyframes.iter_mut().find(|item| item.time.approx_eq(old))
    {
        item.time = time;
        keyframes.sort_by_key(|item| item.time);
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

fn set_interpolation(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    id: Uuid,
    interpolation: Interpolation,
) {
    let mut project = project.borrow_mut();
    let Some(item) = text_key_mut(&mut project, key.clone(), id) else {
        return;
    };
    item.interpolation_to_next = interpolation;
    shrimply_project::project::commit_edit(&project, "text-interpolation");
    drop(project);
    refresh(player, false);
}

fn set_text_interpolation(
    project: &Rc<RefCell<Project>>,
    player: &SharedPlayerState,
    key: SelectedItem,
    id: Uuid,
    interpolation: TextInterpolation,
) {
    let mut project = project.borrow_mut();
    let Some(item) = text_key_mut(&mut project, key.clone(), id) else {
        return;
    };
    item.text_interpolation_to_next = interpolation;
    shrimply_project::project::commit_edit(&project, "text-change-interpolation");
    drop(project);
    refresh(player, false);
}

fn set_key(
    keyframes: &mut Vec<TimelineTextKeyframe>,
    time: Time,
    step: Time,
    value: String,
) -> bool {
    if let Some(keyframe) = keyframes
        .iter_mut()
        .find(|keyframe| keyframe_model::same_frame(keyframe.time, time, step))
    {
        if keyframe.value == value {
            return false;
        }
        keyframe.time = time;
        keyframe.value = value;
    } else {
        keyframes.push(String::keyframe(time, value));
        keyframes.sort_by_key(|keyframe| keyframe.time);
    }
    true
}

fn refresh(player: &SharedPlayerState, inspector: bool) {
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector,
            ..Default::default()
        },
    );
}

fn text_value(project: &Project, key: SelectedItem) -> Option<&TimelineValue<String>> {
    Some(&selected_text(project, key.clone())?.text)
}

fn text_value_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TimelineValue<String>> {
    Some(&mut selected_text_mut(project, key.clone())?.text)
}

fn text_key_mut(
    project: &mut Project,
    key: SelectedItem,
    id: Uuid,
) -> Option<&mut TimelineTextKeyframe> {
    let TimelineBase::Keyframes(keyframes) = &mut text_value_mut(project, key.clone())?.base else {
        return None;
    };
    keyframes.iter_mut().find(|keyframe| keyframe.id() == id)
}

fn selected_text(project: &Project, key: SelectedItem) -> Option<&TextItem> {
    let item = project.video_item(&key)?;
    let VideoItemContent::Text(text) = &item.content else {
        return None;
    };
    Some(text)
}

fn selected_text_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TextItem> {
    let item = project.video_item_mut(&key)?;
    let VideoItemContent::Text(text) = &mut item.content else {
        return None;
    };
    Some(text)
}

fn local_time(project: &Project, key: SelectedItem, position: Time) -> Option<Time> {
    crate::video::visual_local_time(project, key, position)
}

fn global_time(project: &Project, key: SelectedItem, time: Time) -> Option<Time> {
    crate::video::visual_global_time(project, key, time)
}

fn visible_area(project: &Project, key: SelectedItem) -> Option<(Time, Time)> {
    crate::video::visual_visible_area(project, key)
}
use shrimply_ui_foundation::tr;
