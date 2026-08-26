use gtk::prelude::*;
use shrimply_project::project::Project;
use shrimply_scene_3d::{MAX_IOR, MIN_IOR, MIN_ROUGHNESS, NormalMode};
use shrimply_shape_3d::{
    MAX_SMOOTHNESS, MIN_SMOOTHNESS, Shape3dKind, Shape3dModifier, Shape3dRoundingStrategy,
};
use shrimply_video_modifiers::{ModifierEffect, scene_3d::Scene3dModifierEffect};
use uuid::Uuid;

use crate::{
    InspectorContext,
    player_state::{self, ProjectChange},
    selector::{enum_selector, selector},
};

use super::{ScalarOptions, color_row, integer_scalar_row, scalar_row, vec3_row, vec3_scale_row};

pub fn add_rows(value: &Shape3dModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&shape_selector(value.shape, id, context));
    out.append(&vec3_scale_row("Size", &value.size, id, context));
    match value.shape {
        Shape3dKind::Star => {
            out.append(&integer_scalar_row(
                "Points",
                &value.star_points,
                id,
                scalar_options(Some(3.0), Some(64.0)),
                context,
            ));
            out.append(&number_row(
                "Inner radius",
                &value.star_inner_radius_percent,
                id,
                Some(0.0),
                Some(1.0),
                Some("%"),
                context,
            ));
        }
        Shape3dKind::Arrow => {
            out.append(&number_row(
                "Shaft width",
                &value.arrow_shaft_width_percent,
                id,
                Some(0.0),
                Some(1.0),
                Some("%"),
                context,
            ));
            out.append(&number_row(
                "Head length",
                &value.arrow_head_length_percent,
                id,
                Some(0.0),
                Some(1.0),
                Some("%"),
                context,
            ));
        }
        Shape3dKind::Cross => out.append(&number_row(
            "Arm thickness",
            &value.cross_arm_thickness_percent,
            id,
            Some(0.0),
            Some(1.0),
            Some("%"),
            context,
        )),
        Shape3dKind::Disk => {
            out.append(&number_row(
                "Completion",
                &value.disk_completion_degrees,
                id,
                Some(0.0),
                Some(360.0),
                Some("deg"),
                context,
            ));
            out.append(&number_row(
                "Inner radius",
                &value.disk_inner_radius_percent,
                id,
                Some(0.0),
                Some(1.0),
                Some("%"),
                context,
            ));
        }
        Shape3dKind::Torus => out.append(&number_row(
            "Inner radius",
            &value.torus_inner_radius_percent,
            id,
            Some(0.0),
            Some(1.0),
            Some("%"),
            context,
        )),
        _ => {}
    }
    if value.shape.has_profile_corners() {
        out.append(&number_row(
            "Corner radius",
            &value.corner_radius,
            id,
            Some(0.0),
            None,
            None,
            context,
        ));
        out.append(&rounding_selector(value.rounding_strategy, id, context));
    }
    if value.shape.is_extruded() || value.shape == Shape3dKind::Cone {
        out.append(&number_row(
            "Depth edge roundness",
            &value.edge_roundness,
            id,
            Some(0.0),
            None,
            None,
            context,
        ));
    }
    out.append(&integer_scalar_row(
        "Smoothness",
        &value.smoothness,
        id,
        scalar_options(Some(MIN_SMOOTHNESS.into()), Some(MAX_SMOOTHNESS.into())),
        context,
    ));

    for (label, timeline, degrees) in [
        ("Position", &value.transform.position, false),
        ("Anchor", &value.transform.anchor, false),
        ("Rotation", &value.transform.rotation_degrees, true),
    ] {
        out.append(&vec3_row(label, timeline, id, degrees, context));
    }
    out.append(&vec3_scale_row(
        "Scale",
        &value.transform.scale,
        id,
        context,
    ));
    out.append(&color_row(
        "Base color",
        &value.material.base_color,
        id,
        context,
    ));
    for (label, timeline, minimum, maximum) in [
        ("Metallic", &value.material.metallic, Some(0.0), Some(1.0)),
        (
            "Roughness",
            &value.material.roughness,
            Some(MIN_ROUGHNESS as f64),
            Some(1.0),
        ),
        (
            "Subsurface",
            &value.material.subsurface,
            Some(0.0),
            Some(1.0),
        ),
        ("Clearcoat", &value.material.clearcoat, Some(0.0), Some(1.0)),
        ("Sheen", &value.material.sheen, Some(0.0), Some(1.0)),
        (
            "Transmission",
            &value.material.transmission,
            Some(0.0),
            Some(1.0),
        ),
        (
            "Index of refraction",
            &value.material.ior,
            Some(MIN_IOR as f64),
            Some(MAX_IOR as f64),
        ),
    ] {
        out.append(&number_row(
            label, timeline, id, minimum, maximum, None, context,
        ));
    }
    out.append(&normal_selector(value.material.normal_mode, id, context));
}

fn scalar_options(minimum: Option<f64>, maximum: Option<f64>) -> ScalarOptions {
    ScalarOptions {
        minimum,
        maximum,
        unit: None,
        rotating: false,
    }
}

fn number_row(
    label: &str,
    value: &shrimply_core::timeline_value::TimelineValue<f32>,
    id: Uuid,
    minimum: Option<f64>,
    maximum: Option<f64>,
    unit: Option<&'static str>,
    context: &InspectorContext,
) -> gtk::Widget {
    scalar_row(
        label,
        value,
        id,
        ScalarOptions {
            minimum,
            maximum,
            unit,
            rotating: false,
        },
        context,
    )
}

fn shape_selector(value: Shape3dKind, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Shape",
        value,
        [
            (Shape3dKind::Box, "Box"),
            (Shape3dKind::Disk, "Disk / cylinder"),
            (Shape3dKind::Triangle, "Triangle"),
            (Shape3dKind::Star, "Star"),
            (Shape3dKind::Arrow, "Arrow"),
            (Shape3dKind::Diamond, "Diamond"),
            (Shape3dKind::Pentagon, "Pentagon"),
            (Shape3dKind::Hexagon, "Hexagon"),
            (Shape3dKind::Heart, "Heart"),
            (Shape3dKind::Octagon, "Octagon"),
            (Shape3dKind::Cross, "Cross"),
            (Shape3dKind::Sphere, "Sphere"),
            (Shape3dKind::Cone, "Cone"),
            (Shape3dKind::Torus, "Torus"),
            (Shape3dKind::Capsule, "Capsule"),
        ],
        move |shape| {
            update_shape(&context, id, "edit-3d-shape-kind", move |value| {
                value.shape = shape
            })
        },
    )
}

fn rounding_selector(
    value: Shape3dRoundingStrategy,
    id: Uuid,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Corner rounding",
        value,
        [
            (Shape3dRoundingStrategy::Continuous, "Continuous"),
            (Shape3dRoundingStrategy::Circular, "Circular"),
            (Shape3dRoundingStrategy::Chamfer, "Chamfer"),
        ],
        move |rounding| {
            update_shape(&context, id, "edit-3d-shape-rounding", move |value| {
                value.rounding_strategy = rounding
            })
        },
    )
}

fn normal_selector(value: NormalMode, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    enum_selector("Normals", value, move |normal_mode| {
        update_shape(&context, id, "edit-3d-shape-normals", move |value| {
            value.material.normal_mode = normal_mode
        })
    })
}

fn update_shape(
    context: &InspectorContext,
    id: Uuid,
    commit: &str,
    update: impl FnOnce(&mut Shape3dModifier),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(shape) = shape_mut(&mut project, key, id) else {
        return;
    };
    update(shape);
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

fn shape_mut(
    project: &mut Project,
    key: crate::InspectedItem,
    id: Uuid,
) -> Option<&mut Shape3dModifier> {
    project
        .video_item_mut(&key)?
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Scene3d(effect) => match &mut **effect {
                Scene3dModifierEffect::Shape(shape) => Some(&mut **shape),
                _ => None,
            },
            _ => None,
        })
}
