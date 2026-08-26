use adw::prelude::*;

const CONFIRMATION_TIMEOUT_SECONDS: u32 = 2;

pub fn show_confirmation(toasts: &adw::ToastOverlay, title: &str) {
    let toast = adw::Toast::builder()
        .title(title)
        .timeout(CONFIRMATION_TIMEOUT_SECONDS)
        .build();
    add(toasts, toast);
}

pub fn show_confirmation_for_widget(widget: &impl IsA<gtk::Widget>, title: &str) {
    let Some(toasts) = overlay_for_widget(widget) else {
        return;
    };
    show_confirmation(&toasts, title);
}

pub fn add(toasts: &adw::ToastOverlay, toast: adw::Toast) {
    toasts.add_toast(toast);
}

pub(crate) fn overlay_for_widget(widget: &impl IsA<gtk::Widget>) -> Option<adw::ToastOverlay> {
    widget
        .ancestor(adw::ToastOverlay::static_type())
        .and_downcast::<adw::ToastOverlay>()
}
