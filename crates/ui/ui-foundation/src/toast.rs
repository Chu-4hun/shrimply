use adw::prelude::*;

const CONFIRMATION_TIMEOUT_SECONDS: u32 = 2;

pub fn new(title: &str) -> adw::Toast {
    adw::Toast::builder()
        .title(crate::i18n::text(title).as_ref())
        .build()
}

pub fn new_text(title: &str) -> adw::Toast {
    adw::Toast::builder().title(title).build()
}

pub fn with_button(title: &str, button_label: &str) -> adw::Toast {
    adw::Toast::builder()
        .title(crate::i18n::text(title).as_ref())
        .button_label(crate::i18n::text(button_label).as_ref())
        .build()
}

pub fn with_button_text(title: &str, button_label: &str) -> adw::Toast {
    adw::Toast::builder()
        .title(title)
        .button_label(crate::i18n::text(button_label).as_ref())
        .build()
}

pub fn show_confirmation(toasts: &adw::ToastOverlay, title: &str) {
    let toast = new(title);
    toast.set_timeout(CONFIRMATION_TIMEOUT_SECONDS);
    add(toasts, toast);
}

pub fn show_confirmation_text(toasts: &adw::ToastOverlay, title: &str) {
    let toast = new_text(title);
    toast.set_timeout(CONFIRMATION_TIMEOUT_SECONDS);
    add(toasts, toast);
}

pub fn show_confirmation_for_widget(widget: &impl IsA<gtk::Widget>, title: &str) {
    let Some(toasts) = overlay_for_widget(widget) else {
        return;
    };
    show_confirmation(&toasts, title);
}

pub fn show_confirmation_text_for_widget(widget: &impl IsA<gtk::Widget>, title: &str) {
    let Some(toasts) = overlay_for_widget(widget) else {
        return;
    };
    show_confirmation_text(&toasts, title);
}

pub fn add(toasts: &adw::ToastOverlay, toast: adw::Toast) {
    toasts.add_toast(toast);
}

pub(crate) fn overlay_for_widget(widget: &impl IsA<gtk::Widget>) -> Option<adw::ToastOverlay> {
    widget
        .ancestor(adw::ToastOverlay::static_type())
        .and_downcast::<adw::ToastOverlay>()
}
