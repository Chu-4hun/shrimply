use gtk::prelude::*;
use shrimply_core::timeline_value::{TimelineBase, TimelineValue, TimelineValueType};
use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::I18nWidgetExt;

use super::LABEL_WIDTH_CHARS;

pub(crate) fn control<T: TimelineValueType>(
    label: &str,
    value: &TimelineValue<T>,
    editor: gtk::Widget,
    body: Vec<gtk::Widget>,
    on_keyframes_changed: impl Fn(bool) + 'static,
    on_expression_changed: impl Fn(bool) + 'static,
) -> gtk::Widget {
    build(
        label,
        value,
        editor,
        body,
        on_keyframes_changed,
        on_expression_changed,
        false,
    )
}

pub(crate) fn wide_control<T: TimelineValueType>(
    label: &str,
    value: &TimelineValue<T>,
    editor: gtk::Widget,
    body: Vec<gtk::Widget>,
    on_keyframes_changed: impl Fn(bool) + 'static,
    on_expression_changed: impl Fn(bool) + 'static,
) -> gtk::Widget {
    build(
        label,
        value,
        editor,
        body,
        on_keyframes_changed,
        on_expression_changed,
        true,
    )
}

fn build<T: TimelineValueType>(
    label: &str,
    value: &TimelineValue<T>,
    editor: gtk::Widget,
    body: Vec<gtk::Widget>,
    on_keyframes_changed: impl Fn(bool) + 'static,
    on_expression_changed: impl Fn(bool) + 'static,
    wide: bool,
) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.set_hexpand(true);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_hexpand(true);
    let label = gtk::Label::builder()
        .label(tr!(label).as_ref())
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .width_chars(LABEL_WIDTH_CHARS)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    row.append(&label);
    label.set_hexpand(wide);
    if !wide {
        editor.set_hexpand(true);
        row.append(&editor);
    }

    let keyframes = toggle(
        "stopwatch-symbolic",
        "Keyframes",
        matches!(value.base, TimelineBase::Keyframes(_)),
    );
    row.append(&keyframes);
    let expression = toggle(
        "code-symbolic",
        "Expression",
        value
            .expression
            .as_ref()
            .is_some_and(|expression| expression.enabled),
    );
    row.append(&expression);
    root.append(&row);
    if wide {
        editor.set_hexpand(true);
        root.append(&editor);
    }

    for widget in body {
        root.append(&widget);
    }

    keyframes.connect_toggled(move |button| on_keyframes_changed(button.is_active()));
    expression.connect_toggled(move |button| on_expression_changed(button.is_active()));

    root.upcast()
}

fn toggle(icon: &str, tooltip: &str, active: bool) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_icon_name(icon);
    button.set_tooltip_i18n(tooltip);
    button.set_valign(gtk::Align::Center);
    button.set_size_request(32, 32);
    button.set_active(active);
    button.add_css_class("flat");
    button
}
