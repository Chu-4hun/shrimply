use hashbrown::HashMap;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use shrimply_project::project::ItemAddress;
use shrimply_ui_foundation::tr;

use super::InspectorContext;
use super::item::{HeaderAction, HeaderButtonToggle, HeaderToggle, InspectorListItem};
use crate::item::PreviewFocusTarget;
use crate::preview_focus::{self, FocusedPreview};

pub(super) type ExpandedRows = Rc<RefCell<HashMap<(ItemAddress, String), bool>>>;
pub(super) type ActiveCategories = Rc<RefCell<HashMap<ItemAddress, String>>>;

pub(super) struct InspectorCategory {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) icon: &'static str,
    pub(super) items: Vec<InspectorListItem>,
}

thread_local! {
    static EXPANDER_CSS_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn render_categories(
    categories: Vec<InspectorCategory>,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let selector = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    selector.set_hexpand(true);
    selector.set_homogeneous(true);
    selector.set_margin_top(12);
    selector.set_margin_bottom(8);
    selector.set_margin_start(16);
    selector.set_margin_end(16);
    selector.add_css_class("linked");
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_vhomogeneous(false);

    let remembered = context
        .expansion_target
        .as_ref()
        .and_then(|target| context.active_categories.borrow().get(target).cloned());
    let active = categories
        .iter()
        .position(|category| remembered.as_deref() == Some(category.key))
        .unwrap_or(0) as u32;

    let mut first_button = None;
    for (index, category) in categories.into_iter().enumerate() {
        let toggle_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        toggle_content.set_halign(gtk::Align::Center);
        toggle_content.append(&gtk::Image::from_icon_name(category.icon));
        toggle_content.append(&gtk::Label::new(Some(tr!(category.label).as_ref())));
        stack.add_named(&render_list(category.items, context), Some(category.key));
        if index as u32 == active {
            stack.set_visible_child_name(category.key);
        }

        let mut button = gtk::ToggleButton::builder()
            .hexpand(true)
            .tooltip_text(tr!(category.label).as_ref())
            .child(&toggle_content);
        if let Some(first_button) = &first_button {
            button = button.group(first_button);
        }
        let button = button.active(index as u32 == active).build();
        let target = context.expansion_target.clone();
        let active_categories = context.active_categories.clone();
        let category_stack = stack.clone();
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            category_stack.set_visible_child_name(category.key);
            if let Some(target) = &target {
                active_categories
                    .borrow_mut()
                    .insert(target.clone(), category.key.to_string());
            }
        });
        first_button.get_or_insert_with(|| button.clone());
        selector.append(&button);
    }

    context.category_bar.append(&selector);
    context.category_bar.set_visible(true);
    vec![stack.upcast()]
}

fn render_list(items: Vec<InspectorListItem>, context: &InspectorContext) -> gtk::Box {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    list.set_margin_top(4);
    list.set_margin_bottom(16);
    list.set_margin_start(16);
    list.set_margin_end(16);

    for item in items {
        match item {
            InspectorListItem::Item(item) => {
                let expanded = context
                    .expansion_target
                    .as_ref()
                    .and_then(|target| {
                        context
                            .expanded_rows
                            .borrow()
                            .get(&(target.clone(), item.key().to_string()))
                            .copied()
                    })
                    .unwrap_or(matches!(item.key(), "transform" | "text" | "caption-text"));

                let row = gtk::Box::new(gtk::Orientation::Vertical, 0);
                row.add_css_class("card");
                add_preview_focus(&row, item.key(), item.preview_target(), context);
                let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                header.set_margin_top(6);
                header.set_margin_bottom(6);
                header.set_margin_start(8);
                header.set_margin_end(8);
                install_expander_css(&header.display());
                let expander_icon = gtk::Image::from_icon_name("pan-end-symbolic");
                expander_icon.add_css_class("inspector-expander-icon");
                if expanded {
                    expander_icon.add_css_class("expanded");
                }
                let expander = gtk::Button::builder()
                    .child(&expander_icon)
                    .tooltip_text(tr!(if expanded { "Collapse" } else { "Expand" }).as_ref())
                    .css_classes(["flat"])
                    .build();
                header.append(&expander);
                header.append(
                    &gtk::Label::builder()
                        .label(tr!(item.title()).as_ref())
                        .halign(gtk::Align::Start)
                        .hexpand(true)
                        .css_classes(["heading"])
                        .build(),
                );

                if let Some(toggle) = item.toggle() {
                    add_toggle(&header, toggle);
                }
                if let Some(toggle) = item.button_toggle() {
                    add_button_toggle(&header, toggle);
                }

                let reset = item.reset(context);
                add_action(
                    &header,
                    &HeaderAction {
                        icon: "edit-undo-symbolic",
                        tooltip: "Reset",
                        sensitive: true,
                        activate: reset,
                    },
                );
                for action in item.actions() {
                    add_action(&header, action);
                }

                let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
                controls.set_margin_top(4);
                controls.set_margin_bottom(12);
                controls.set_margin_start(12);
                controls.set_margin_end(12);
                for control in item.controls(context) {
                    controls.append(&control);
                }
                let revealer = gtk::Revealer::builder()
                    .child(&controls)
                    .reveal_child(expanded)
                    .transition_type(gtk::RevealerTransitionType::SlideDown)
                    .transition_duration(180)
                    .build();
                expander.connect_clicked({
                    let revealer = revealer.clone();
                    let expander_icon = expander_icon.clone();
                    let expanded_rows = context.expanded_rows.clone();
                    let target = context.expansion_target.clone();
                    let key = item.key().to_string();
                    move |button| {
                        let expanded = !revealer.reveals_child();
                        revealer.set_reveal_child(expanded);
                        if expanded {
                            expander_icon.add_css_class("expanded");
                        } else {
                            expander_icon.remove_css_class("expanded");
                        }
                        button.set_tooltip_text(Some(
                            tr!(if expanded { "Collapse" } else { "Expand" }).as_ref(),
                        ));
                        if let Some(target) = &target {
                            expanded_rows
                                .borrow_mut()
                                .insert((target.clone(), key.clone()), expanded);
                        }
                    }
                });
                row.append(&header);
                row.append(&revealer);
                list.append(&row);
            }
            InspectorListItem::Flat(widget) => list.append(&widget),
        }
    }

    list
}

fn add_preview_focus(
    row: &gtk::Box,
    card_key: &str,
    target: PreviewFocusTarget,
    context: &InspectorContext,
) {
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
    let target = target.resolve(item_id);
    let card_key = card_key.to_string();
    sync_preview_focus_class(
        row,
        preview_focus::snapshot(&context.preview_focus).as_ref(),
        &item,
        &card_key,
    );

    let click = gtk::GestureClick::new();
    click.set_button(0);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let click_focus = context.preview_focus.clone();
    let click_card_key = card_key.clone();
    let click_item = item.clone();
    click.connect_pressed(move |_, _, _, _| {
        preview_focus::set(
            &click_focus,
            FocusedPreview {
                item: click_item.clone(),
                card_key: click_card_key.clone(),
                target,
            },
        );
    });
    row.add_controller(click);

    let focus = gtk::EventControllerFocus::new();
    let keyboard_focus = context.preview_focus.clone();
    let keyboard_card_key = card_key.clone();
    let keyboard_item = item.clone();
    focus.connect_enter(move |_| {
        preview_focus::set(
            &keyboard_focus,
            FocusedPreview {
                item: keyboard_item.clone(),
                card_key: keyboard_card_key.clone(),
                target,
            },
        );
    });
    row.add_controller(focus);

    let weak_row = row.downgrade();
    let listener_scope = Rc::downgrade(&context.listener_scope);
    let listener_focus = context.preview_focus.clone();
    preview_focus::connect_while_alive_named(
        &context.preview_focus,
        "inspector preview focus style",
        move || listener_scope.upgrade().is_some(),
        move || {
            let Some(row) = weak_row.upgrade() else {
                return;
            };
            sync_preview_focus_class(
                &row,
                preview_focus::snapshot(&listener_focus).as_ref(),
                &item,
                &card_key,
            );
        },
    );
}

fn sync_preview_focus_class(
    row: &gtk::Box,
    focused: Option<&FocusedPreview>,
    item: &ItemAddress,
    card_key: &str,
) {
    if focused.is_some_and(|focused| &focused.item == item && focused.card_key == card_key) {
        row.add_css_class("accent");
    } else {
        row.remove_css_class("accent");
    }
}

fn add_toggle(header: &gtk::Box, toggle: &HeaderToggle) {
    let switch = gtk::Switch::builder()
        .active(toggle.active)
        .tooltip_text(tr!(toggle.tooltip).as_ref())
        .valign(gtk::Align::Center)
        .build();
    let activate = toggle.activate.clone();
    switch.connect_active_notify(move |switch| activate(switch.is_active()));
    header.append(&switch);
}

fn add_button_toggle(header: &gtk::Box, toggle: &HeaderButtonToggle) {
    let button = gtk::ToggleButton::builder()
        .icon_name(toggle.icon)
        .active(toggle.active)
        .tooltip_text(tr!(toggle.tooltip).as_ref())
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let activate = toggle.activate.clone();
    button.connect_toggled(move |button| activate(button.is_active()));
    header.append(&button);
}

fn install_expander_css(display: &gtk::gdk::Display) {
    EXPANDER_CSS_INSTALLED.with(|installed| {
        if installed.replace(true) {
            return;
        }
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            ".inspector-expander-icon { \
                 -gtk-icon-transform: rotate(0deg); \
                 transition: 180ms ease; \
             } \
             .inspector-expander-icon.expanded { \
                 -gtk-icon-transform: rotate(90deg); \
             }",
        );
        gtk::style_context_add_provider_for_display(
            display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
}

fn add_action(header: &gtk::Box, action: &HeaderAction) {
    let button = gtk::Button::builder()
        .icon_name(action.icon)
        .tooltip_text(tr!(action.tooltip).as_ref())
        .sensitive(action.sensitive)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let activate = action.activate.clone();
    button.connect_clicked(move |_| activate());
    header.append(&button);
}
