use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::I18nWidgetExt;
use std::{cell::Cell, rc::Rc, time::Duration};

use gtk::glib;
use gtk::prelude::*;
use shrimply_project::project::Project;
use shrimply_ui_foundation::ui::{ProgressButton, ProgressButtonState, enum_dropdown};
use shrimply_video_modifiers::{ModifierEffect, RasterModifierEffect, sam2::Sam2Modifier};
use uuid::Uuid;

use super::{ScalarOptions, scalar_row};
use crate::{
    InspectorContext,
    player_state::{self, ProjectChange},
    ui::control_row,
};

const ANALYZE_TOOLTIP: &str =
    "Precompute compact CPU mask frames so normal playback does not run SAM2";

pub fn add_rows(value: &Sam2Modifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    let prompt_signature = value.prompt_signature();
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let model = enum_dropdown(value.model, move |model| {
        update(&project, key.as_ref(), id, |sam2| sam2.model = model);
        refresh(&player);
    });
    out.append(&control_row("Model", &model));

    out.append(&scalar_row(
        "Threshold",
        &value.threshold,
        id,
        ScalarOptions {
            minimum: Some(-8.0),
            maximum: Some(8.0),
            unit: None,
            rotating: false,
        },
        context,
    ));
    out.append(&scalar_row(
        "Edge softness",
        &value.softness,
        id,
        ScalarOptions {
            minimum: Some(0.0),
            maximum: Some(8.0),
            unit: None,
            rotating: false,
        },
        context,
    ));

    for (index, point) in value.points.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let position = super::vec_row(
            &shrimply_ui_foundation::i18n::text_args(
                "Point %{number}",
                &[("number", (index + 1).to_string())],
            ),
            &point.position,
            id,
            true,
            Some((0.0, 1.0)),
            context,
        );
        position.set_hexpand(true);
        row.append(&position);
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        let point_id = point.id;
        let point_type = enum_dropdown(point.label, move |label| {
            update(&project, key.as_ref(), id, |sam2| {
                if let Some(point) = sam2.points.iter_mut().find(|point| point.id == point_id) {
                    point.label = label;
                }
            });
            refresh(&player);
        });
        point_type.set_tooltip_i18n("Point type");
        row.append(&point_type);
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(tr!("Remove point").as_ref())
            .css_classes(["flat"])
            .build();
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        let point_id = point.id;
        remove.connect_clicked(move |_| {
            update(&project, key.as_ref(), id, |sam2| {
                sam2.points.retain(|point| point.id != point_id);
                if sam2.points.is_empty() && sam2.box_prompt.is_none() {
                    sam2.seed_position = None;
                }
            });
            refresh(&player);
        });
        row.append(&remove);
        out.append(&row);
    }

    if value.box_prompt.is_some() {
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(tr!("Remove box").as_ref())
            .css_classes(["flat"])
            .build();
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        remove.connect_clicked(move |_| {
            update(&project, key.as_ref(), id, |sam2| {
                sam2.box_prompt = None;
                if sam2.points.is_empty() {
                    sam2.seed_position = None;
                }
            });
            refresh(&player);
        });
        out.append(&control_row("Box", &remove));
    }

    let analyze = ProgressButton::new("Analyze");
    let can_analyze = !value.points.is_empty() || value.box_prompt.is_some();
    analyze.widget().set_halign(gtk::Align::End);
    analyze.widget().set_sensitive(can_analyze);
    analyze.widget().set_tooltip_i18n(ANALYZE_TOOLTIP);
    analyze.widget().connect_clicked({
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        let current_generation = value.analysis_generation;
        let next_generation = current_generation.wrapping_add(1).max(1);
        let preferences = context.preferences.clone();
        move |_| {
            if shrimply_video::sam2_analysis::cancel(id, current_generation) {
                refresh(&player);
                return;
            }
            shrimply_video::sam2_analysis::start(
                id,
                next_generation,
                shrimply_video::sam2_analysis::Status::Running {
                    message: "Sending request…".to_string(),
                    completed_frames: 0,
                    total_frames: 0,
                    prompt_signature,
                    server_url: shrimply_state::preferences::snapshot(&preferences)
                        .compute_server_url,
                },
            );
            update(&project, key.as_ref(), id, |sam2| {
                sam2.analysis_generation = next_generation;
            });
            refresh(&player);
        }
    });
    let hovered = Rc::new(Cell::new(false));
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let analyze = analyze.clone();
        let hovered = hovered.clone();
        let generation = value.analysis_generation;
        move |_, _, _| {
            hovered.set(true);
            update_analysis_status(
                &analyze,
                analysis_status(id, generation, prompt_signature).0.as_ref(),
                true,
                can_analyze,
            );
        }
    });
    motion.connect_leave({
        let analyze = analyze.clone();
        let hovered = hovered.clone();
        let generation = value.analysis_generation;
        move |_| {
            hovered.set(false);
            update_analysis_status(
                &analyze,
                analysis_status(id, generation, prompt_signature).0.as_ref(),
                false,
                can_analyze,
            );
        }
    });
    analyze.widget().add_controller(motion);
    let analysis = analysis_status(id, value.analysis_generation, prompt_signature).0;
    update_analysis_status(&analyze, analysis.as_ref(), hovered.get(), can_analyze);
    let generation = value.analysis_generation;
    if generation > 0 {
        let analyze = analyze.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if analyze.widget().parent().is_none() {
                return glib::ControlFlow::Break;
            }
            let (analysis, stale) = analysis_status(id, generation, prompt_signature);
            let finished = stale
                || matches!(
                    analysis.as_ref(),
                    Some(
                        shrimply_video::sam2_analysis::Status::Complete { .. }
                            | shrimply_video::sam2_analysis::Status::Failed(_)
                            | shrimply_video::sam2_analysis::Status::Cancelled
                    )
                );
            update_analysis_status(&analyze, analysis.as_ref(), hovered.get(), can_analyze);
            if finished {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
    out.append(analyze.widget());

    let invert = gtk::Switch::builder()
        .active(value.invert)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    invert.connect_active_notify({
        let project = context.project.clone();
        let player = context.player_state.clone();
        let key = context.selected_item.clone();
        move |invert| {
            update(&project, key.as_ref(), id, |sam2| {
                sam2.invert = invert.is_active()
            });
            refresh(&player);
        }
    });
    out.append(&control_row("Invert", &invert));
}

fn update_analysis_status(
    button: &ProgressButton,
    status: Option<&shrimply_video::sam2_analysis::Status>,
    hovered: bool,
    can_analyze: bool,
) {
    button.widget().remove_css_class("destructive-action");
    button.widget().remove_css_class("suggested-action");
    button.widget().set_tooltip_i18n(ANALYZE_TOOLTIP);
    match status {
        Some(shrimply_video::sam2_analysis::Status::Running {
            message,
            completed_frames,
            total_frames,
            ..
        }) => {
            button.widget().set_sensitive(true);
            if hovered {
                button.set_label("Cancel");
            } else {
                button.set_label(message);
            }
            if hovered {
                button.widget().add_css_class("destructive-action");
            }
            if *total_frames == 0 {
                button.set_state(ProgressButtonState::Indeterminate);
            } else {
                button.set_state(ProgressButtonState::Progress(
                    *completed_frames as f64 / *total_frames as f64,
                ));
            }
        }
        Some(shrimply_video::sam2_analysis::Status::Complete { .. }) => {
            button.set_label("Reanalyze");
            button.widget().set_sensitive(can_analyze);
            button.set_state(ProgressButtonState::Idle);
        }
        Some(shrimply_video::sam2_analysis::Status::Failed(error)) => {
            if error == "Compute server connection failed" {
                button.widget().set_label(error);
            } else {
                button.set_label("Analyze");
            }
            button.widget().set_sensitive(can_analyze);
            button.widget().add_css_class("suggested-action");
            button.widget().set_tooltip_text(Some(error));
            button.set_state(ProgressButtonState::Idle);
        }
        Some(shrimply_video::sam2_analysis::Status::Cancelling) => {
            button.set_label("Cancelling…");
            button.widget().set_sensitive(false);
            button.set_state(ProgressButtonState::Indeterminate);
        }
        Some(shrimply_video::sam2_analysis::Status::Cancelled) => {
            button.set_label("Cancelled");
            button.widget().set_sensitive(can_analyze);
            button.widget().add_css_class("suggested-action");
            button.set_state(ProgressButtonState::Idle);
        }
        None => {
            button.set_label("Analyze");
            button.widget().set_sensitive(can_analyze);
            button.widget().add_css_class("suggested-action");
            button.set_state(ProgressButtonState::Idle);
        }
    }
}

fn analysis_status(
    modifier_id: Uuid,
    generation: u64,
    prompt_signature: u64,
) -> (Option<shrimply_video::sam2_analysis::Status>, bool) {
    let status = shrimply_video::sam2_analysis::get(modifier_id, generation);
    let stale = matches!(
        status.as_ref(),
        Some(
            shrimply_video::sam2_analysis::Status::Running {
                prompt_signature: stored,
                ..
            } | shrimply_video::sam2_analysis::Status::Complete {
                prompt_signature: stored,
            }
        ) if *stored != prompt_signature
    );
    if stale {
        shrimply_video::sam2_analysis::cancel(modifier_id, generation);
        (None, true)
    } else {
        (status, false)
    }
}

fn update(
    project: &std::rc::Rc<std::cell::RefCell<Project>>,
    key: Option<&shrimply_project::project::ItemAddress>,
    id: Uuid,
    action: impl FnOnce(&mut Sam2Modifier),
) {
    let Some(key) = key else {
        return;
    };
    let mut project = project.borrow_mut();
    let Some(item) = project.video_item_mut(key) else {
        return;
    };
    let Some(sam2) = item
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Raster(effect) => match &mut **effect {
                RasterModifierEffect::Sam2(sam2) => Some(sam2),
                _ => None,
            },
            _ => None,
        })
    else {
        return;
    };
    action(sam2);
    shrimply_project::project::commit_edit(&project, "edit-sam2-prompts");
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
