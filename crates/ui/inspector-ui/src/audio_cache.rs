use std::{cell::Cell, rc::Rc, time::Duration};

use gtk::{glib, prelude::*};
use shrimply_audio::modifier_cache::{self, Status};
use shrimply_audio_modifiers::{AudioModifierEffect, CacheFormat, CacheModifier, OpusCacheQuality};
use shrimply_ui_foundation::ui::{ProgressButton, ProgressButtonState, control_row, dropdown};
use uuid::Uuid;

use crate::{
    InspectorContext,
    player_state::{self, ProjectChange},
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum CachePreset {
    OpusCompact,
    OpusBalanced,
    OpusHigh,
    Flac,
}

pub fn rows(value: &CacheModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    let out = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let current = modifier_cache::status(id);
    let project = context.project.clone();
    let player = context.player_state.clone();
    let key = context.selected_item.clone();
    let format = dropdown(
        preset(value),
        [
            (CachePreset::OpusCompact, "Opus · Compact"),
            (CachePreset::OpusBalanced, "Opus · Balanced"),
            (CachePreset::OpusHigh, "Opus · High"),
            (CachePreset::Flac, "FLAC · Lossless"),
        ],
        move |preset| {
            let Some(key) = &key else {
                return;
            };
            let mut project = project.borrow_mut();
            let Some(cache) = project
                .audio_item_mut(key)
                .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
                .and_then(|modifier| match &mut modifier.effect {
                    AudioModifierEffect::Cache(cache) => Some(cache),
                    _ => None,
                })
            else {
                return;
            };
            if apply_preset(cache, preset) {
                shrimply_project::project::commit_edit(&project, "audio-cache-format");
                drop(project);
                player_state::refresh_project(
                    &player,
                    ProjectChange {
                        inspector: true,
                        ..Default::default()
                    },
                );
            }
        },
    );
    out.append(&control_row("Format", &format));

    let bake = ProgressButton::new("Bake");
    bake.widget().set_halign(gtk::Align::End);
    let hovered = Rc::new(Cell::new(false));
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let bake = bake.clone();
        let format = format.clone();
        let hovered = hovered.clone();
        move |_, _, _| {
            hovered.set(true);
            update_controls(&bake, &format, &modifier_cache::status(id), true);
        }
    });
    motion.connect_leave({
        let bake = bake.clone();
        let format = format.clone();
        let hovered = hovered.clone();
        move |_| {
            hovered.set(false);
            update_controls(&bake, &format, &modifier_cache::status(id), false);
        }
    });
    bake.widget().add_controller(motion);
    update_controls(&bake, &format, &current, false);
    out.append(bake.widget());

    bake.widget().connect_clicked({
        let project = context.project.clone();
        let key = context.selected_item.clone();
        let player = context.player_state.clone();
        let bake = bake.clone();
        let format = format.clone();
        move |_| {
            let result = if matches!(modifier_cache::status(id), Status::Baking { .. }) {
                modifier_cache::invalidate(id)
            } else if let Some(key) = key.clone() {
                modifier_cache::bake(project.borrow().clone(), key, id)
            } else {
                return;
            };
            match result {
                Ok(()) => refresh(&player),
                Err(error) => update_controls(&bake, &format, &Status::Failed(error), false),
            }
        }
    });

    let scope = Rc::downgrade(&context.listener_scope);
    let player = context.player_state.clone();
    let was_baking = Rc::new(Cell::new(matches!(current, Status::Baking { .. })));
    glib::timeout_add_local(Duration::from_millis(50), move || {
        if scope.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        let current = modifier_cache::status(id);
        update_controls(&bake, &format, &current, hovered.get());
        if matches!(current, Status::Baking { .. }) {
            was_baking.set(true);
        } else if was_baking.replace(false) {
            refresh(&player);
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });

    vec![out.upcast()]
}

fn preset(value: &CacheModifier) -> CachePreset {
    match (value.format, value.opus_quality) {
        (CacheFormat::Flac, _) => CachePreset::Flac,
        (CacheFormat::Opus, OpusCacheQuality::Compact) => CachePreset::OpusCompact,
        (CacheFormat::Opus, OpusCacheQuality::Balanced) => CachePreset::OpusBalanced,
        (CacheFormat::Opus, OpusCacheQuality::High) => CachePreset::OpusHigh,
    }
}

fn apply_preset(value: &mut CacheModifier, preset: CachePreset) -> bool {
    let (format, quality) = match preset {
        CachePreset::OpusCompact => (CacheFormat::Opus, OpusCacheQuality::Compact),
        CachePreset::OpusBalanced => (CacheFormat::Opus, OpusCacheQuality::Balanced),
        CachePreset::OpusHigh => (CacheFormat::Opus, OpusCacheQuality::High),
        CachePreset::Flac => (CacheFormat::Flac, value.opus_quality),
    };
    if value.format == format && value.opus_quality == quality {
        return false;
    }
    value.format = format;
    value.opus_quality = quality;
    true
}

fn update_controls(bake: &ProgressButton, format: &gtk::DropDown, status: &Status, hovered: bool) {
    bake.widget().remove_css_class("destructive-action");
    bake.widget().remove_css_class("suggested-action");
    format.set_sensitive(!matches!(status, Status::Baking { .. }));
    bake.widget().set_sensitive(true);
    match status {
        Status::Missing => {
            bake.set_label("Bake");
            bake.widget().add_css_class("suggested-action");
            bake.set_state(ProgressButtonState::Idle);
        }
        Status::Baking { completed, total } => {
            bake.set_label(if hovered { "Cancel" } else { "Baking…" });
            if hovered {
                bake.widget().add_css_class("destructive-action");
            }
            bake.set_state(if *total == 0 {
                ProgressButtonState::Indeterminate
            } else {
                ProgressButtonState::Progress(*completed as f64 / *total as f64)
            });
        }
        Status::Ready => {
            bake.set_label("Rebake");
            bake.widget().add_css_class("suggested-action");
            bake.set_state(ProgressButtonState::Idle);
        }
        Status::Failed(_) => {
            bake.set_label("Bake");
            bake.widget().add_css_class("suggested-action");
            bake.set_state(ProgressButtonState::Idle);
        }
    }
}

fn refresh(player: &crate::player_state::SharedPlayerState) {
    player_state::refresh_project(
        player,
        ProjectChange {
            audio: true,
            audio_waveforms: true,
            inspector: true,
            ..Default::default()
        },
    );
}
