use shrimply_ui_foundation::tr;
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use num_traits::ToPrimitive;

use super::layered;
use crate::InspectedItem as SelectedItem;
use crate::keyframe_editor::{self, KeyframeEditorActions, KeyframeGraph, SpeedSegment};
use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::timeline_value::*;
use crate::{InspectorContext, keyframe_model};
use shrimply_project::project::Time;
use shrimply_project::project::{Color, Project};
use shrimply_ui_foundation::ui::ColorPicker;
use uuid::Uuid;

pub(crate) type ColorGetMut = for<'a> fn(
    &'a mut Project,
    SelectedItem,
)
    -> Option<&'a mut TimelineValue<shrimply_core::Color<u8>>>;
pub(crate) type SceneColorGetMut = for<'a> fn(
    &'a mut shrimply_scene_3d::ObjScene,
) -> &'a mut TimelineValue<shrimply_core::Color<u8>>;

#[derive(Clone, Copy)]
pub(crate) enum ColorAccess {
    Item(ColorGetMut),
    Scene3d(SceneColorGetMut),
    Modifier { id: Uuid, value_id: Uuid },
    Background { value_id: Uuid },
    PaintPalette { value_id: Uuid },
}
impl ColorAccess {
    fn get_mut(
        self,
        p: &mut Project,
        k: SelectedItem,
    ) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
        match self {
            Self::Item(get) => get(p, k.clone()),
            Self::Scene3d(get) => scene_3d(p, k.clone()).map(get),
            Self::Modifier { id, value_id } => crate::modifiers::color_mut(
                &mut p
                    .video_item_mut(&k)?
                    .modifiers
                    .iter_mut()
                    .find(|m| m.id == id)?
                    .effect,
                value_id,
            ),
            Self::Background { value_id } => {
                let shrimply_project::project::VideoItemContent::Background(background) =
                    &mut p.video_item_mut(&k)?.content
                else {
                    return None;
                };
                background.generator.color_mut(value_id)
            }
            Self::PaintPalette { value_id } => {
                let shrimply_project::project::VideoItemContent::Paint(paint) =
                    &mut p.video_item_mut(&k)?.content
                else {
                    return None;
                };
                paint
                    .palette
                    .iter_mut()
                    .map(|entry| &mut entry.color)
                    .find(|color| color.id == value_id)
            }
        }
    }

    fn mark_mutated(self, project: &mut Project, key: SelectedItem) {
        if let Self::PaintPalette { .. } = self {
            crate::paint::bump_revision_for_key(project, key);
        }
    }
}

fn scene_3d(project: &mut Project, key: SelectedItem) -> Option<&mut shrimply_scene_3d::ObjScene> {
    let item = project.video_item_mut(&key)?;
    let shrimply_project::project::VideoItemContent::Obj(scene) = &mut item.content else {
        return None;
    };
    Some(scene)
}

#[derive(Clone, Copy)]
pub(crate) struct ColorTarget {
    pub(crate) access: ColorAccess,
    pub(crate) scope_id: Option<Uuid>,
    pub(crate) local_time: fn(&Project, SelectedItem, Time) -> Option<Time>,
    pub(crate) global_time: fn(&Project, SelectedItem, Time) -> Option<Time>,
    pub(crate) duration: fn(&Project, SelectedItem) -> Option<Time>,
    pub(crate) refresh: ProjectChange,
    pub(crate) commit_name: &'static str,
}

pub(crate) fn color_control(
    label: &str,
    value: &TimelineValue<shrimply_core::Color<u8>>,
    context: &InspectorContext,
    target: ColorTarget,
) -> gtk::Widget {
    let color = current_color(value, current_time(context, target));
    let Some(key) = context.selected_item.clone() else {
        let button = ColorPicker::builder(color)
            .title(tr!(label).as_ref())
            .hexpand(true)
            .build();
        return layered::control(label, value, button, Vec::new(), |_| {}, |_| {});
    };
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let refresh = context.refresh.clone();
    let button_key = key.clone();
    let button = ColorPicker::builder(color)
        .title(tr!(label).as_ref())
        .hexpand(true)
        .on_change(move |color| {
            if update_color(&project, &player_state, button_key.clone(), target, color) {
                refresh();
            }
        })
        .build();
    let keyframes = matches!(value.base, TimelineBase::Keyframes(_));
    let expression = value.expression.as_ref().is_some_and(|v| v.enabled);
    let mut body = Vec::new();
    if keyframes {
        let duration = {
            let project = context.project.borrow();
            (target.duration)(&project, key.clone()).unwrap_or(Time::ZERO)
        };
        let built = keyframe_editor::build(
            context,
            gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
            color_speed_graph(value),
            (Time::ZERO, duration),
            format!("color:{}:{:?}:{label}", target.commit_name, target.scope_id),
            keyframe_actions(context, key.clone(), target),
        );
        let project = context.project.clone();
        let graph_key = key.clone();
        keyframe_editor::connect_graph_refresh(
            context,
            "inspector color keyframe graph refresh",
            built.update_graph.clone(),
            move || {
                let mut project = project.borrow_mut();
                target
                    .access
                    .get_mut(&mut project, graph_key.clone())
                    .map(|value| color_speed_graph(value))
            },
        );
        body.push(built.widget);
    }
    if expression {
        let project = context.project.clone();
        let player = context.player_state.clone();
        let expression_key = key.clone();
        body.push(crate::rhai_editor::editor(
            value.expression_source().map(str::to_string),
            crate::rhai_editor::ExpressionValue::Color,
            move |source| {
                update_expression(&project, &player, expression_key.clone(), target, source)
            },
        ));
    }
    let kp = context.project.clone();
    let kpl = context.player_state.clone();
    let kr = context.refresh.clone();
    let ep = context.project.clone();
    let epl = context.player_state.clone();
    let er = context.refresh.clone();
    let keyframe_key = key.clone();
    let expression_key = key;
    layered::control(
        label,
        value,
        button,
        body,
        move |enabled| {
            if toggle_keyframes(&kp, &kpl, keyframe_key.clone(), target, enabled) {
                kr();
            }
        },
        move |enabled| {
            toggle_expression(&ep, &epl, expression_key.clone(), target, enabled);
            er();
        },
    )
}

fn update_color(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: ColorTarget,
    color: Color<u8>,
) -> bool {
    let mut project = project.borrow_mut();
    let position = player_state::snapshot(player_state).position;
    let Some(time) = (target.local_time)(&project, key.clone(), position) else {
        return false;
    };
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return false;
    };
    if current_color(value, time) == color {
        return false;
    }
    match &mut value.base {
        TimelineBase::Const(v) => *v = color,
        TimelineBase::Keyframes(values) => {
            if let Some(v) = values.iter_mut().find(|v| v.time.approx_eq(time)) {
                v.value = color
            } else {
                values.push(TimelineVectorKeyframe::<shrimply_core::Color<u8>> {
                    id: Uuid::new_v4(),
                    time,
                    value: color,
                    interpolation_to_next: Default::default(),
                });
                values.sort_by_key(|v| v.time)
            }
        }
    }
    target.access.mark_mutated(&mut project, key);
    shrimply_project::project::commit_coalesced_edit(&project, target.commit_name);
    drop(project);
    player_state::refresh_project(player_state, keyframe_model::live_refresh(target.refresh));
    true
}

fn current_time(c: &InspectorContext, t: ColorTarget) -> Time {
    let p = c.project.borrow();
    c.selected_item
        .clone()
        .and_then(|key| {
            (t.local_time)(
                &p,
                key.clone(),
                player_state::snapshot(&c.player_state).position,
            )
        })
        .unwrap_or(Time::ZERO)
}
fn current_color(value: &TimelineValue<shrimply_core::Color<u8>>, time: Time) -> Color<u8> {
    match &value.base {
        TimelineBase::Const(v) => *v,
        TimelineBase::Keyframes(v) => crate::transform_eval::color_keyframes_value(v, time),
    }
}

fn keyframe_actions(
    context: &InspectorContext,
    key: SelectedItem,
    target: ColorTarget,
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
                player_state::seek_time(&select_player, position);
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
        update_point: Rc::new(move |old_time, time, _| {
            update_keyframe_point(
                &point_project,
                &point_player,
                point_key.clone(),
                target,
                old_time,
                time,
            );
        }),
        copy_keyframes: Rc::new(move |times| {
            let mut project = copy_project.borrow_mut();
            target
                .access
                .get_mut(&mut project, copy_key.clone())
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
    target: ColorTarget,
    time: Time,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(value) = target.access.get_mut(&mut project, key.clone()) else {
        return false;
    };
    let next = current_color(value, time);
    let TimelineBase::Keyframes(keyframes) = &mut value.base else {
        return false;
    };
    if keyframes
        .iter()
        .any(|keyframe| keyframe.time.approx_eq(time))
    {
        return false;
    }
    let mut next = keyframe(time, next);
    if let Some(previous) = keyframes.iter().rev().find(|keyframe| keyframe.time < time)
        && keyframes.iter().any(|keyframe| keyframe.time > time)
    {
        next.interpolation_to_next = previous.interpolation_to_next;
    }
    keyframes.push(next);
    keyframes.sort_by_key(|keyframe| keyframe.time);
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
    target: ColorTarget,
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
    let removed = keyframes.remove(index);
    if keyframes.is_empty() {
        value.base = TimelineBase::Const(removed.value);
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
    target: ColorTarget,
    old_time: Time,
    time: Time,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(keyframes) = color_keyframes_mut(&mut project, key.clone(), target) else {
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
    player_state::refresh_project(player_state, keyframe_model::live_refresh(target.refresh));
    true
}

fn set_keyframe_interpolation(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    target: ColorTarget,
    owner_id: Uuid,
    interpolation: Interpolation,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(keyframes) = color_keyframes_mut(&mut project, key.clone(), target) else {
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

fn color_keyframes_mut(
    project: &mut Project,
    key: SelectedItem,
    target: ColorTarget,
) -> Option<&mut Vec<TimelineVectorKeyframe<shrimply_core::Color<u8>>>> {
    let value = target.access.get_mut(project, key.clone())?;
    match &mut value.base {
        TimelineBase::Keyframes(keyframes) => Some(keyframes),
        TimelineBase::Const(_) => None,
    }
}

fn color_speed_graph(value: &TimelineValue<shrimply_core::Color<u8>>) -> KeyframeGraph {
    let TimelineBase::Keyframes(keyframes) = &value.base else {
        return KeyframeGraph::Speed {
            segments: Vec::new(),
            keys: Vec::new(),
            static_value: 0.0,
        };
    };
    let segments = color_speed_segments(keyframes);
    KeyframeGraph::Speed {
        segments,
        keys: keyframes.iter().map(|keyframe| keyframe.time).collect(),
        static_value: 0.0,
    }
}

fn color_speed_segments(
    keyframes: &[TimelineVectorKeyframe<shrimply_core::Color<u8>>],
) -> Vec<SpeedSegment> {
    keyframes
        .windows(2)
        .filter_map(|pair| {
            let span = pair[1].time.seconds - pair[0].time.seconds;
            let seconds = span.to_f64()?;
            if seconds <= f64::EPSILON {
                return None;
            }
            Some(SpeedSegment {
                owner_id: pair[0].id,
                start: pair[0].time,
                end: pair[1].time,
                value: pair[0].value.oklaba_distance(pair[1].value) as f64 / seconds,
                interpolation: pair[0].interpolation_to_next,
            })
        })
        .collect()
}

fn keyframe(time: Time, value: Color<u8>) -> TimelineVectorKeyframe<shrimply_core::Color<u8>> {
    TimelineVectorKeyframe::<shrimply_core::Color<u8>> {
        id: Uuid::new_v4(),
        time,
        value,
        interpolation_to_next: Default::default(),
    }
}
fn toggle_keyframes(
    p: &Rc<RefCell<Project>>,
    s: &SharedPlayerState,
    k: SelectedItem,
    t: ColorTarget,
    enabled: bool,
) -> bool {
    let position = player_state::snapshot(s).position;
    let mut p = p.borrow_mut();
    let Some(time) = (t.local_time)(&p, k.clone(), position) else {
        return false;
    };
    let Some(v) = t.access.get_mut(&mut p, k.clone()) else {
        return false;
    };
    let current = current_color(v, time);
    match (&mut v.base, enabled) {
        (TimelineBase::Const(_), false) | (TimelineBase::Keyframes(_), true) => return false,
        (base @ TimelineBase::Const(_), true) => {
            *base = TimelineBase::Keyframes(vec![keyframe(time, current)]);
        }
        (base @ TimelineBase::Keyframes(_), false) => *base = TimelineBase::Const(current),
    }
    t.access.mark_mutated(&mut p, k);
    shrimply_project::project::commit_edit(&p, t.commit_name);
    drop(p);
    player_state::refresh_project(s, t.refresh);
    true
}
fn toggle_expression(
    p: &Rc<RefCell<Project>>,
    s: &SharedPlayerState,
    k: SelectedItem,
    t: ColorTarget,
    enabled: bool,
) {
    let mut p = p.borrow_mut();
    let Some(v) = t.access.get_mut(&mut p, k.clone()) else {
        return;
    };
    let changed = match &mut v.expression {
        Some(e) if e.enabled != enabled => {
            e.enabled = enabled;
            true
        }
        Some(_) => false,
        None if enabled => {
            v.expression = Some(TimelineExpression {
                id: Uuid::new_v4(),
                enabled: true,
                source: "value".into(),
            });
            true
        }
        None => false,
    };
    if !changed {
        return;
    }
    t.access.mark_mutated(&mut p, k);
    shrimply_project::project::commit_edit(&p, t.commit_name);
    drop(p);
    player_state::refresh_project(s, t.refresh)
}
fn update_expression(
    p: &Rc<RefCell<Project>>,
    s: &SharedPlayerState,
    k: SelectedItem,
    t: ColorTarget,
    source: String,
) {
    let mut p = p.borrow_mut();
    let Some(e) = t
        .access
        .get_mut(&mut p, k.clone())
        .and_then(|v| v.expression.as_mut())
    else {
        return;
    };
    if e.source == source {
        return;
    }
    e.source = source;
    t.access.mark_mutated(&mut p, k);
    shrimply_project::project::commit_coalesced_edit(&p, t.commit_name);
    drop(p);
    player_state::refresh_project(s, keyframe_model::live_refresh(t.refresh))
}
