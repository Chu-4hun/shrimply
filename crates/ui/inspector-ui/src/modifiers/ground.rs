use gtk::prelude::*;
use shrimply_project::project::Project;
use shrimply_video_modifiers::{
    ModifierEffect,
    scene_3d::{GroundKind, GroundModifier, Scene3dModifierEffect},
};
use uuid::Uuid;

use crate::{
    InspectorContext,
    player_state::{self, ProjectChange},
    selector::selector,
};

use super::{ScalarOptions, scalar_row, vec3_row};

pub fn add_rows(value: &GroundModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&kind_selector(value.kind, id, context));
    if value.kind == GroundKind::Square {
        out.append(&number_row(
            "Size",
            &value.size,
            id,
            f64::EPSILON,
            None,
            context,
        ));
    }
    out.append(&composite_selector(value.composite_enabled, id, context));
    if value.composite_enabled {
        out.append(&number_row(
            "Ground intensity",
            &value.intensity,
            id,
            0.0,
            None,
            context,
        ));
    }
    out.append(&vec3_row("Position", &value.position, id, false, context));
    out.append(&vec3_row(
        "Rotation",
        &value.rotation_degrees,
        id,
        true,
        context,
    ));
    for (label, timeline) in [
        ("Opacity", &value.opacity),
        ("Shadow strength", &value.shadow_strength),
        ("Reflection opacity", &value.reflection),
        ("Roughness", &value.roughness),
    ] {
        out.append(&number_row(label, timeline, id, 0.0, Some(1.0), context));
    }
}

fn number_row(
    label: &str,
    value: &shrimply_core::timeline_value::TimelineValue<f32>,
    id: Uuid,
    minimum: f64,
    maximum: Option<f64>,
    context: &InspectorContext,
) -> gtk::Widget {
    scalar_row(
        label,
        value,
        id,
        ScalarOptions {
            minimum: Some(minimum),
            maximum,
            unit: None,
            rotating: false,
        },
        context,
    )
}

fn kind_selector(value: GroundKind, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Kind",
        value,
        [
            (GroundKind::Infinite, "Infinite"),
            (GroundKind::Square, "Square"),
        ],
        move |kind| {
            update_ground(&context, id, "edit-ground-kind", move |ground| {
                ground.kind = kind
            })
        },
    )
}

fn composite_selector(value: bool, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Composite background",
        value,
        [(false, "Off"), (true, "On")],
        move |enabled| {
            update_ground(&context, id, "edit-ground-composite", move |ground| {
                ground.composite_enabled = enabled
            })
        },
    )
}

fn update_ground(
    context: &InspectorContext,
    id: Uuid,
    commit: &str,
    update: impl FnOnce(&mut GroundModifier),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(ground) = ground_mut(&mut project, key.clone(), id) else {
        return;
    };
    update(ground);
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

fn ground_mut(
    project: &mut Project,
    key: crate::InspectedItem,
    id: Uuid,
) -> Option<&mut GroundModifier> {
    project
        .video_item_mut(&key)?
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Scene3d(effect) => match &mut **effect {
                Scene3dModifierEffect::Ground(ground) => Some(&mut **ground),
                _ => None,
            },
            _ => None,
        })
}
