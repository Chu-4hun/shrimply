use std::{cell::Cell, rc::Rc, time::Duration};

use gtk::{glib, prelude::*};
use shrimply_project::project::Project;
use shrimply_ui_foundation::ui::{NumberPicker, ProgressButton, ProgressButtonState};
use shrimply_video::transparent_fill_analysis::Status;
use shrimply_video_modifiers::{
    ModifierEffect, RasterModifierEffect,
    transparent_fill::{MAXIMUM_GAP, TransparentFillModifier},
};
use uuid::Uuid;

use super::{ScalarOptions, scalar_row};
use crate::{
    InspectorContext,
    player_state::{self, ProjectChange},
    ui::control_row,
};

const ANALYZE_TOOLTIP: &str = "Precompute exact one-bit transparency masks for every frame";

pub fn add_rows(
    value: &TransparentFillModifier,
    out: &gtk::Box,
    id: Uuid,
    context: &InspectorContext,
) {
    out.append(&scalar_row(
        "Tolerance",
        &value.tolerance,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(1.0),
            unit: None,
            rotating: false,
        },
        context,
    ));

    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let gap = NumberPicker::integer_builder(value.maximum_gap)
        .minimum(0.0)
        .maximum(f64::from(MAXIMUM_GAP))
        .on_change_integer(move |maximum_gap: u32| {
            update(&project, key.as_ref(), id, |fill| {
                fill.maximum_gap = maximum_gap
            });
            refresh(&player);
        })
        .build();
    gap.set_tooltip_text(Some(
        "0 disables gap closing; positive values set the maximum gap in pixels",
    ));
    out.append(&control_row("Maximum gap", &gap));

    for (index, point) in value.points.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let position = super::vec_row(
            &format!("Point {}", index + 1),
            &point.position,
            id,
            true,
            Some((0.0, 1.0)),
            context,
        );
        position.set_hexpand(true);
        row.append(&position);
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove point")
            .css_classes(["flat"])
            .build();
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        let point_id = point.id;
        remove.connect_clicked(move |_| {
            update(&project, key.as_ref(), id, |fill| {
                fill.points.retain(|point| point.id != point_id)
            });
            refresh(&player);
        });
        row.append(&remove);
        out.append(&row);
    }

    let analyze = ProgressButton::new("Analyze");
    let can_analyze = !value.points.is_empty();
    analyze.widget().set_halign(gtk::Align::End);
    analyze.widget().set_sensitive(can_analyze);
    analyze.widget().set_tooltip_text(Some(ANALYZE_TOOLTIP));
    analyze.widget().connect_clicked({
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        move |_| {
            if shrimply_video::transparent_fill_analysis::cancel(id) {
                refresh(&player);
                return;
            }
            update(&project, key.as_ref(), id, |fill| {
                fill.analysis_generation = fill.analysis_generation.wrapping_add(1).max(1)
            });
            if let Some(key) = key.as_ref() {
                let snapshot = project.borrow().clone();
                if let Err(error) =
                    shrimply_video::transparent_fill_analysis::analyze(snapshot, key, id)
                {
                    tracing::error!("start transparent fill analysis: {error}");
                }
            }
            refresh(&player);
        }
    });

    let hovered = Rc::new(Cell::new(false));
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let analyze = analyze.clone();
        let hovered = hovered.clone();
        let project = context.project.clone();
        let key = context.selected_item.clone();
        move |_, _, _| {
            hovered.set(true);
            update_analysis_status(
                &analyze,
                current_status(&project, key.as_ref(), id),
                true,
                can_analyze,
            );
        }
    });
    motion.connect_leave({
        let analyze = analyze.clone();
        let hovered = hovered.clone();
        let project = context.project.clone();
        let key = context.selected_item.clone();
        move |_| {
            hovered.set(false);
            update_analysis_status(
                &analyze,
                current_status(&project, key.as_ref(), id),
                false,
                can_analyze,
            );
        }
    });
    analyze.widget().add_controller(motion);
    let status = current_status(&context.project, context.selected_item.as_ref(), id);
    update_analysis_status(&analyze, status.clone(), false, can_analyze);
    if matches!(status, Status::Running { .. }) {
        let analyze_poll = analyze.clone();
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if analyze_poll.widget().parent().is_none() {
                return glib::ControlFlow::Break;
            }
            let status = current_status(&project, key.as_ref(), id);
            update_analysis_status(&analyze_poll, status.clone(), hovered.get(), can_analyze);
            if matches!(status, Status::Running { .. }) {
                glib::ControlFlow::Continue
            } else {
                refresh(&player);
                glib::ControlFlow::Break
            }
        });
    }
    out.append(analyze.widget());
}

fn current_status(
    project: &Rc<std::cell::RefCell<Project>>,
    key: Option<&shrimply_project::project::ItemAddress>,
    id: Uuid,
) -> Status {
    let Some(key) = key else {
        return Status::Missing;
    };
    shrimply_video::transparent_fill_analysis::status(&project.borrow(), key, id)
}

fn update_analysis_status(
    button: &ProgressButton,
    status: Status,
    hovered: bool,
    can_analyze: bool,
) {
    button.widget().remove_css_class("destructive-action");
    button.widget().remove_css_class("suggested-action");
    button.widget().set_tooltip_text(Some(ANALYZE_TOOLTIP));
    match status {
        Status::Running { completed, total } => {
            button.widget().set_sensitive(true);
            button
                .widget()
                .set_label(if hovered { "Cancel" } else { "Analyzing…" });
            if hovered {
                button.widget().add_css_class("destructive-action");
            }
            if total == 0 {
                button.set_state(ProgressButtonState::Indeterminate);
            } else {
                button.set_state(ProgressButtonState::Progress(
                    completed as f64 / total as f64,
                ));
            }
        }
        Status::Complete => {
            button.widget().set_label("Reanalyze");
            button.widget().set_sensitive(can_analyze);
            button.set_state(ProgressButtonState::Idle);
        }
        Status::Failed(error) => {
            button.widget().set_label("Analyze");
            button.widget().set_sensitive(can_analyze);
            button.widget().add_css_class("suggested-action");
            button.widget().set_tooltip_text(Some(&error));
            button.set_state(ProgressButtonState::Idle);
        }
        Status::Missing | Status::Cancelled => {
            button.widget().set_label("Analyze");
            button.widget().set_sensitive(can_analyze);
            button.widget().add_css_class("suggested-action");
            button.set_state(ProgressButtonState::Idle);
        }
    }
}

fn update(
    project: &Rc<std::cell::RefCell<Project>>,
    key: Option<&shrimply_project::project::ItemAddress>,
    id: Uuid,
    action: impl FnOnce(&mut TransparentFillModifier),
) {
    let Some(key) = key else {
        return;
    };
    let mut project = project.borrow_mut();
    let Some(fill) = project
        .video_item_mut(key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::TransparentFill(fill) => Some(fill),
                _ => None,
            },
            _ => None,
        })
    else {
        return;
    };
    action(fill);
    shrimply_project::project::commit_edit(&project, "edit-transparent-fill");
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
