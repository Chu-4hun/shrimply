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
    let toast = adw::Toast::builder()
        .title(title)
        .button_label("Show in Files")
        .build();
    let reveal_parent = parent.clone();
    let path = path.to_path_buf();
    toast.connect_button_clicked(move |_| {
        crate::desktop_open::reveal_file(reveal_parent.upcast_ref(), path.clone());
    });
    crate::toast::add(toasts, toast);
}
