use gtk::prelude::*;
use shrimply_project::project::Project;
use shrimply_ui_foundation::ui::{NumberPicker, control_row, enum_dropdown};
use shrimply_video_modifiers::{
    ModifierEffect,
    vectorize::{
        MAX_ANGLE_DEGREES, MAX_BINARY_THRESHOLD, MAX_COLOR_PRECISION, MAX_GRADIENT_STEP,
        MAX_ITERATIONS, MAX_PATH_PRECISION, MAX_SEGMENT_LENGTH, MAX_SPECKLE_SIZE,
        MIN_COLOR_PRECISION, MIN_SEGMENT_LENGTH, VectorizeColorMode, VectorizeModifier,
        VectorizePathMode, VectorizePreset,
    },
};
use uuid::Uuid;

use super::InspectorContext;
use crate::player_state::{self, ProjectChange};

pub fn add_rows(value: &VectorizeModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let color = value.color_mode == VectorizeColorMode::Color;
    let spline = value.path_mode == VectorizePathMode::Spline;
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let preset = enum_dropdown(value.preset, move |preset| {
        update(&project, key.as_ref(), id, |vectorize| {
            if preset == VectorizePreset::Custom {
                vectorize.preset = preset;
            } else {
                *vectorize = VectorizeModifier::from_preset(preset);
            }
        });
        commit(&project);
        refresh(&player);
    });
    out.append(&control_row("Preset", &preset));

    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let color_mode = enum_dropdown(value.color_mode, move |color_mode| {
        update(&project, key.as_ref(), id, |vectorize| {
            vectorize.color_mode = color_mode;
            vectorize.preset = VectorizePreset::Custom;
        });
        commit(&project);
        refresh(&player);
    });
    out.append(&control_row("Color mode", &color_mode));

    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let hierarchy = enum_dropdown(value.hierarchy, move |hierarchy| {
        update(&project, key.as_ref(), id, |vectorize| {
            vectorize.hierarchy = hierarchy;
            vectorize.preset = VectorizePreset::Custom;
        });
        commit(&project);
        refresh(&player);
    });
    hierarchy.set_sensitive(color);
    out.append(&control_row("Hierarchy", &hierarchy));

    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let path_mode = enum_dropdown(value.path_mode, move |path_mode| {
        update(&project, key.as_ref(), id, |vectorize| {
            vectorize.path_mode = path_mode;
            vectorize.preset = VectorizePreset::Custom;
        });
        commit(&project);
        refresh(&player);
    });
    out.append(&control_row("Path mode", &path_mode));

    out.append(&integer_row(
        "Speckle size",
        value.speckle_size,
        (0, MAX_SPECKLE_SIZE),
        true,
        (id, context),
        |vectorize, value| vectorize.speckle_size = value,
    ));
    out.append(&integer_row(
        "Color precision",
        value.color_precision,
        (MIN_COLOR_PRECISION, MAX_COLOR_PRECISION),
        color,
        (id, context),
        |vectorize, value| vectorize.color_precision = value,
    ));
    out.append(&integer_row(
        "Gradient step",
        value.gradient_step,
        (0, MAX_GRADIENT_STEP),
        color,
        (id, context),
        |vectorize, value| vectorize.gradient_step = value,
    ));
    out.append(&integer_row(
        "B&W threshold",
        value.binary_threshold,
        (0, MAX_BINARY_THRESHOLD),
        !color,
        (id, context),
        |vectorize, value| vectorize.binary_threshold = value,
    ));
    out.append(&integer_row(
        "Corner threshold",
        value.corner_threshold_degrees,
        (0, MAX_ANGLE_DEGREES),
        spline,
        (id, context),
        |vectorize, value| vectorize.corner_threshold_degrees = value,
    ));
    out.append(&decimal_row(
        "Segment length",
        value.segment_length,
        (MIN_SEGMENT_LENGTH, MAX_SEGMENT_LENGTH),
        spline,
        (id, context),
        |vectorize, value| vectorize.segment_length = value,
    ));
    out.append(&integer_row(
        "Max iterations",
        value.max_iterations,
        (0, MAX_ITERATIONS),
        spline,
        (id, context),
        |vectorize, value| vectorize.max_iterations = value,
    ));
    out.append(&integer_row(
        "Splice threshold",
        value.splice_threshold_degrees,
        (0, MAX_ANGLE_DEGREES),
        spline,
        (id, context),
        |vectorize, value| vectorize.splice_threshold_degrees = value,
    ));
    out.append(&integer_row(
        "Path precision",
        value.path_precision,
        (0, MAX_PATH_PRECISION),
        true,
        (id, context),
        |vectorize, value| vectorize.path_precision = value,
    ));
}

fn integer_row(
    label: &str,
    value: u32,
    range: (u32, u32),
    sensitive: bool,
    target: (Uuid, &InspectorContext),
    set: fn(&mut VectorizeModifier, u32),
) -> gtk::Widget {
    let (minimum, maximum) = range;
    let (id, context) = target;
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let commit_project = context.project.clone();
    let commit_player = context.player_state.clone();
    let picker = NumberPicker::integer_builder(value)
        .minimum(f64::from(minimum))
        .maximum(f64::from(maximum))
        .on_change_integer(move |value: u32| {
            update(&project, key.as_ref(), id, |vectorize| {
                set(vectorize, value);
                vectorize.preset = VectorizePreset::Custom;
            });
            refresh_video(&player);
        })
        .on_commit(move |_| {
            commit(&commit_project);
            refresh(&commit_player);
        })
        .build();
    picker.set_sensitive(sensitive);
    control_row(label, &picker)
}

fn decimal_row(
    label: &str,
    value: f32,
    range: (f32, f32),
    sensitive: bool,
    target: (Uuid, &InspectorContext),
    set: fn(&mut VectorizeModifier, f32),
) -> gtk::Widget {
    let (minimum, maximum) = range;
    let (id, context) = target;
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let commit_project = context.project.clone();
    let commit_player = context.player_state.clone();
    let picker = NumberPicker::builder(f64::from(value))
        .accepted_range(f64::from(minimum), f64::from(maximum))
        .drag_step(0.1)
        .digits(1)
        .on_change(move |value| {
            update(&project, key.as_ref(), id, |vectorize| {
                set(vectorize, value as f32);
                vectorize.preset = VectorizePreset::Custom;
            });
            refresh_video(&player);
        })
        .on_commit(move |_| {
            commit(&commit_project);
            refresh(&commit_player);
        })
        .build();
    picker.set_sensitive(sensitive);
    control_row(label, &picker)
}

fn update(
    project: &std::rc::Rc<std::cell::RefCell<Project>>,
    key: Option<&shrimply_project::project::ItemAddress>,
    id: Uuid,
    action: impl FnOnce(&mut VectorizeModifier),
) {
    let Some(key) = key else {
        return;
    };
    let mut project = project.borrow_mut();
    let Some(vectorize) = project
        .video_item_mut(key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Vectorize(vectorize) => Some(vectorize),
            _ => None,
        })
    else {
        return;
    };
    action(vectorize);
}

fn commit(project: &std::rc::Rc<std::cell::RefCell<Project>>) {
    shrimply_project::project::commit_edit(&project.borrow(), "edit-vectorize");
}

fn refresh(player: &crate::player_state::SharedPlayerState) {
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn refresh_video(player: &crate::player_state::SharedPlayerState) {
    player_state::refresh_project(
        player,
        ProjectChange {
            video: true,
            ..Default::default()
        },
    );
}
