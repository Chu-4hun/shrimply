use adw::prelude::*;

pub fn show_export_finished_for_widget(
    widget: &impl IsA<gtk::Widget>,
    title: &str,
    path: &std::path::Path,
) {
    let Some(parent) = widget.root().and_downcast::<adw::ApplicationWindow>() else {
        return;
    };
    let Some(toasts) = crate::toast::overlay_for_widget(widget) else {
        return;
    };
    show_export_finished(&toasts, &parent, title, path);
}

pub fn show_export_finished(
    toasts: &adw::ToastOverlay,
    parent: &adw::ApplicationWindow,
    title: &str,
    path: &std::path::Path,
) {
    let toast = crate::toast::with_button(title, "Show in Files");
    add_export_toast(toasts, parent, toast, path);
}

pub fn show_export_finished_text(
    toasts: &adw::ToastOverlay,
    parent: &adw::ApplicationWindow,
    title: &str,
    path: &std::path::Path,
) {
    let toast = crate::toast::with_button_text(title, "Show in Files");
    add_export_toast(toasts, parent, toast, path);
}

fn add_export_toast(
    toasts: &adw::ToastOverlay,
    parent: &adw::ApplicationWindow,
    toast: adw::Toast,
    path: &std::path::Path,
) {
    let reveal_parent = parent.clone();
    let path = path.to_path_buf();
    toast.connect_button_clicked(move |_| {
        crate::desktop_open::reveal_file(reveal_parent.upcast_ref(), path.clone());
    });
    crate::toast::add(toasts, toast);
}
