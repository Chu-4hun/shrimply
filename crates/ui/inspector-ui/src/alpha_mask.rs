use std::rc::Rc;

use gtk::prelude::*;
use shrimply_project::project::{AlphaMaskShape, VisualAlphaMask, VisualAlphaMaskTarget};
use shrimply_ui_foundation::ui::switch_row;

use crate::{
    InspectorContext,
    item::HeaderButtonToggle,
    player_state::{self, ProjectChange},
    preview_focus::{self, FocusedPreview, PreviewTarget},
    timeline_value::{
        scalar::{ScalarAccess, ScalarSpec, ScalarTarget, scalar_control},
        vector::vec2::{VecAccess, VecSpec, VecTarget, vec_control},
    },
};

pub(crate) fn button_toggle(
    target: VisualAlphaMaskTarget,
    context: &InspectorContext,
) -> HeaderButtonToggle {
    let active = context.selected_item.as_ref().is_some_and(|key| {
        context
            .project
            .borrow()
            .video_item(key)
            .and_then(|item| item.alpha_mask(target))
            .is_some_and(|mask| mask.enabled)
    });
    let context = context.detached();
    HeaderButtonToggle {
        icon: "select-symbolic",
        active,
        tooltip: "Mask",
        activate: Rc::new(move |active| set_open(&context, target, active)),
    }
}

pub(crate) fn widget(target: VisualAlphaMaskTarget, context: &InspectorContext) -> gtk::Widget {
    let out = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let mask = context.selected_item.as_ref().and_then(|key| {
        context
            .project
            .borrow()
            .video_item(key)
            .and_then(|item| item.alpha_mask(target))
            .cloned()
    });
    let Some(mask) = mask.filter(|mask| mask.enabled) else {
        return out.upcast();
    };

    out.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let click = gtk::GestureClick::new();
    click.set_button(0);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    click.connect_pressed({
        let context = context.detached();
        move |_, _, _, _| focus_mask(&context, target, mask.shape)
    });
    out.add_controller(click);

    out.append(&crate::ui::selector(
        "Shape",
        mask.shape,
        [
            (AlphaMaskShape::Rectangle, "Rectangle"),
            (AlphaMaskShape::Ellipse, "Ellipse"),
            (AlphaMaskShape::Polygon, "Polygon"),
        ],
        {
            let context = context.detached();
            move |shape| {
                update(&context, target, "alpha-mask-shape", |mask| {
                    mask.shape = shape
                });
                focus_mask(&context, target, shape);
            }
        },
    ));

    out.append(&switch_row("Invert", None, mask.invert, {
        let context = context.detached();
        move |invert| {
            update(&context, target, "invert-alpha-mask", |mask| {
                mask.invert = invert
            });
        }
    }));

    out.append(&vec_control(
        "Center",
        &mask.center,
        context,
        vec_target(target, mask.center.id),
        VecSpec {
            first_prefix: "X",
            second_prefix: "Y",
            drag_step: 0.01,
            digits: 2,
            width_chars: 7,
            minimum: None,
            maximum: None,
            unit_name: "x",
        },
    ));
    out.append(&vec_control(
        "Size",
        &mask.size,
        context,
        vec_target(target, mask.size.id),
        VecSpec {
            first_prefix: "W",
            second_prefix: "H",
            drag_step: 0.01,
            digits: 2,
            width_chars: 7,
            minimum: Some(0.0),
            maximum: None,
            unit_name: "x",
        },
    ));
    out.append(&scalar_control(
        "Rotation",
        &mask.rotation_degrees,
        context,
        scalar_target(target, mask.rotation_degrees.id),
        ScalarSpec {
            drag_step: 1.0,
            digits: 1,
            integer: false,
            width_chars: 8,
            minimum: None,
            maximum: None,
            unit_name: Some("°"),
            rotating_icon: Some(("arrow3-up-symbolic", 0.0)),
            display: f64::from,
            store: |value| value as f32,
            clamp: |value| value,
        },
    ));
    if mask.shape == AlphaMaskShape::Rectangle {
        out.append(&scalar_control(
            "Roundness",
            &mask.rounding,
            context,
            scalar_target(target, mask.rounding.id),
            ScalarSpec {
                drag_step: 1.0,
                digits: 1,
                integer: false,
                width_chars: 8,
                minimum: Some(0.0),
                maximum: Some(100.0),
                unit_name: Some("%"),
                rotating_icon: None,
                display: |value| f64::from(value) * 100.0,
                store: |value| (value / 100.0) as f32,
                clamp: |value| value.clamp(0.0, 1.0),
            },
        ));
    }
    out.append(&scalar_control(
        "Feather",
        &mask.feather,
        context,
        scalar_target(target, mask.feather.id),
        ScalarSpec {
            drag_step: 1.0,
            digits: 1,
            integer: false,
            width_chars: 8,
            minimum: Some(0.0),
            maximum: Some(100.0),
            unit_name: Some("%"),
            rotating_icon: None,
            display: |value| f64::from(value) * 100.0,
            store: |value| (value / 100.0) as f32,
            clamp: |value| value.clamp(0.0, 1.0),
        },
    ));

    out.upcast()
}

fn vec_target(target: VisualAlphaMaskTarget, value_id: uuid::Uuid) -> VecTarget {
    VecTarget {
        access: VecAccess::AlphaMask { target, value_id },
        scope_id: Some(value_id),
        local_time: crate::video::visual_local_time,
        duration: crate::video::visual_duration,
        refresh: ProjectChange {
            video: true,
            ..Default::default()
        },
        commit_name: "visual-alpha-mask-vector",
    }
}

fn scalar_target(target: VisualAlphaMaskTarget, value_id: uuid::Uuid) -> ScalarTarget {
    ScalarTarget {
        access: ScalarAccess::AlphaMask { target, value_id },
        scope_id: Some(value_id),
        local_time: crate::video::visual_local_time,
        duration: crate::video::visual_duration,
        refresh: ProjectChange {
            video: true,
            ..Default::default()
        },
        commit_name: "visual-alpha-mask-scalar",
    }
}

fn focus_mask(context: &InspectorContext, target: VisualAlphaMaskTarget, _shape: AlphaMaskShape) {
    let Some(item) = context.preview_item.clone() else {
        return;
    };
    let Some(item_id) = context
        .project
        .borrow()
        .video_item(&item)
        .map(|item| item.id)
    else {
        return;
    };
    let (card_key, target) = match target {
        VisualAlphaMaskTarget::Compositing => (
            "compositing".to_string(),
            PreviewTarget::new(
                item_id,
                shrimply_project::project::COMPOSITING_ALPHA_MASK_PREVIEW_FACET,
            ),
        ),
        VisualAlphaMaskTarget::Modifier(id) => (
            format!("modifier:{id}"),
            PreviewTarget::new(
                id,
                shrimply_project::project::MODIFIER_ALPHA_MASK_PREVIEW_FACET,
            ),
        ),
    };
    preview_focus::set(
        &context.preview_focus,
        FocusedPreview {
            item,
            card_key,
            target,
        },
    );
}

fn update(
    context: &InspectorContext,
    target: VisualAlphaMaskTarget,
    commit_name: &'static str,
    change: impl FnOnce(&mut VisualAlphaMask),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(mask) = project
        .video_item_mut(&key)
        .and_then(|item| item.alpha_mask_mut(target))
    else {
        return;
    };
    change(mask);
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    let refresh = context.refresh.clone();
    gtk::glib::idle_add_local_once(move || refresh());
}

fn set_open(context: &InspectorContext, target: VisualAlphaMaskTarget, open: bool) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    let shape = if open {
        if let Some(mask) = item.alpha_mask_mut(target) {
            if mask.enabled {
                return;
            }
            mask.enabled = true;
            mask.shape
        } else {
            let mask = VisualAlphaMask::default();
            let shape = mask.shape;
            if !item.set_alpha_mask(target, Some(mask)) {
                return;
            }
            shape
        }
    } else {
        let Some(shape) = item.alpha_mask(target).map(|mask| mask.shape) else {
            return;
        };
        if !item.set_alpha_mask(target, None) {
            return;
        }
        shape
    };
    shrimply_project::project::commit_edit(
        &project,
        if open {
            "add-alpha-mask"
        } else {
            "remove-alpha-mask"
        },
    );
    drop(project);
    if open {
        focus_mask(context, target, shape);
    } else {
        focus_card(context, target);
    }
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
    let refresh = context.refresh.clone();
    gtk::glib::idle_add_local_once(move || refresh());
}

fn focus_card(context: &InspectorContext, target: VisualAlphaMaskTarget) {
    let Some(item) = context.preview_item.clone() else {
        return;
    };
    let Some(item_id) = context
        .project
        .borrow()
        .video_item(&item)
        .map(|item| item.id)
    else {
        return;
    };
    let (card_key, target) = match target {
        VisualAlphaMaskTarget::Compositing => (
            "compositing".to_string(),
            PreviewTarget::new(item_id, shrimply_project::project::ITEM_PREVIEW_FACET),
        ),
        VisualAlphaMaskTarget::Modifier(id) => (
            format!("modifier:{id}"),
            PreviewTarget::new(id, shrimply_video_modifiers::MODIFIER_PREVIEW_FACET),
        ),
    };
    preview_focus::set(
        &context.preview_focus,
        FocusedPreview {
            item,
            card_key,
            target,
        },
    );
}
