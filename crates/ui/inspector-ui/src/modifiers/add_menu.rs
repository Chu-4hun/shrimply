use gtk::prelude::*;

use crate::{InspectorContext, player_state};
use shrimply_video_modifiers::{
    ModifierEffect, ModifierModel, RasterModifierEffect, VectorModifierEffect,
};

pub(super) fn button(context: &InspectorContext) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name("list-add-symbolic"));
    content.append(&gtk::Label::new(Some("Add modifier")));

    let button = gtk::MenuButton::builder()
        .child(&content)
        .halign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search modifiers")
        .hexpand(true)
        .build();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroller = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(280)
        .min_content_height(240)
        .max_content_height(360)
        .build();
    let popover_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    popover_content.append(&search);
    popover_content.append(&scroller);
    let popover = gtk::Popover::builder()
        .child(&popover_content)
        .has_arrow(false)
        .build();
    popover.add_css_class("menu");
    button.set_popover(Some(&popover));

    populate(&list, "", context, &popover);
    search.connect_search_changed({
        let list = list.clone();
        let context = context.detached();
        let popover = popover.clone();
        move |search| populate(&list, search.text().as_str(), &context, &popover)
    });
    popover.connect_show(move |_| {
        search.grab_focus();
    });
    button.upcast()
}

fn populate(list: &gtk::Box, query: &str, context: &InspectorContext, popover: &gtk::Popover) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let query = query.trim().to_lowercase();
    let state = context.selected_item.clone().and_then(|key| {
        context
            .project
            .borrow()
            .video_item(&key)?
            .modifier_output_state()
            .ok()
    });
    let Some(state) = state else {
        return;
    };
    let canvas = context.project.borrow().canvas_size;
    let canvas_size = glam::Vec2::new(canvas.width.max(1) as f32, canvas.height.max(1) as f32);
    let transform_center = transform_center(context, canvas_size * 0.5);
    let mut effects = ModifierEffect::catalog()
        .filter_map(|effect| effect.adapted_for(state))
        .filter_map(|mut effect| {
            match &mut effect {
                ModifierEffect::Vector(effect) => {
                    if let VectorModifierEffect::Transform(transform) = &mut **effect {
                        **transform =
                            shrimply_video_modifiers::transform::TransformModifier::centered_at(
                                transform_center,
                            );
                    }
                }
                ModifierEffect::Rasterize(rasterize) => {
                    *rasterize =
                        shrimply_video_modifiers::rasterize::RasterizeModifier::new(canvas_size);
                }
                ModifierEffect::Raster(effect) => {
                    if let RasterModifierEffect::Transform(transform) = &mut **effect {
                        **transform =
                            shrimply_video_modifiers::transform::TransformModifier::centered_at(
                                transform_center,
                            );
                    }
                }
                ModifierEffect::Scene3d(_) | ModifierEffect::Vectorize(_) => {}
            }
            let rank = if effect.display_name().to_lowercase().contains(&query) {
                0
            } else if effect
                .keywords()
                .iter()
                .any(|keyword| keyword.to_lowercase().contains(&query))
            {
                1
            } else {
                return None;
            };
            Some((rank, effect))
        })
        .collect::<Vec<_>>();
    effects.sort_by_key(|(rank, _)| *rank);
    for (_, effect) in effects {
        let name = effect.display_name();
        let row = gtk::Button::builder()
            .label(name)
            .halign(gtk::Align::Fill)
            .hexpand(true)
            .css_classes(["flat"])
            .build();
        let context = context.detached();
        let popover = popover.clone();
        row.connect_clicked(move |_| {
            super::add_effect(effect.clone(), &context);
            popover.popdown();
        });
        list.append(&row);
    }
}

fn transform_center(context: &InspectorContext, fallback: glam::Vec2) -> glam::Vec2 {
    let Some(key) = context.selected_item.clone() else {
        return fallback;
    };
    let position = player_state::snapshot(&context.player_state).position;
    let audio = context.audio_analysis_at(position);
    let project = context.project.borrow();
    let Some(item) = project.video_item(&key) else {
        return fallback;
    };
    let center = shrimply_evaluation::resolve_item_transform_with_audio(
        &project,
        item,
        position,
        &audio,
        &mut Default::default(),
    )
    .position;
    if center.is_finite() { center } else { fallback }
}
