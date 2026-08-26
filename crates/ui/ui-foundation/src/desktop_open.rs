use gtk::gio::prelude::{AppLaunchContextExt, DBusProxyExt};
use gtk::glib;
use gtk::glib::variant::ToVariant;
use gtk::prelude::{Cast, DisplayExt, GdkAppLaunchContextExt, WidgetExt};
use gtk::{gdk, gio};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn show_path_in_folder(widget: &gtk::Widget, path: PathBuf) -> Result<(), String> {
    let path = absolute_path(&path);
    let metadata = path
        .metadata()
        .map_err(|error| format!("Unable to inspect {}: {error}", path.display()))?;

    if metadata.is_dir() {
        launch_file(widget, path);
    } else {
        reveal_file(widget, path);
    }

    Ok(())
}

pub fn reveal_file(widget: &gtk::Widget, path: PathBuf) {
    let path = absolute_path(&path);
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) => {
            tracing::warn!(
                "file reveal fd open failed path={}: {error}",
                path.display()
            );
            fallback_reveal_file(widget, path);
            return;
        }
    };

    // Do not simplify this without retesting both selection and focus on
    // Wayland. GtkFileLauncher::open_containing_folder can reveal the file but
    // fail to focus an existing file-manager window. The portal selects the
    // item, then opening the parent folder gives GTK a path that can activate
    // the file-manager window under Wayland focus rules.
    let path_display = path.display().to_string();
    let fd_list = gio::UnixFDList::from_array([file]);
    let options = glib::VariantDict::default();
    if let Some(token) = portal_activation_token(widget, &path) {
        options.insert("activation_token", token.as_str());
    } else {
        tracing::debug!("file reveal portal activation token unavailable path={path_display}");
    }
    let parameters = ("", glib::variant::Handle::from(0), options).to_variant();

    let widget_for_fallback = widget.clone();
    let fallback_path = path.clone();
    let widget_for_focus = widget.clone();
    let focus_path = path.clone();
    gio::DBusProxy::for_bus(
        gio::BusType::Session,
        gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES | gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
        None::<&gio::DBusInterfaceInfo>,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.OpenURI",
        None::<&gio::Cancellable>,
        move |proxy| match proxy {
            Ok(proxy) => {
                let widget_for_fallback = widget_for_fallback.clone();
                let fallback_path = fallback_path.clone();
                let path_display = path_display.clone();
                proxy.call_with_unix_fd_list(
                    "OpenDirectory",
                    Some(&parameters),
                    gio::DBusCallFlags::NONE,
                    -1,
                    Some(&fd_list),
                    None::<&gio::Cancellable>,
                    move |result| match result {
                        Ok(_) => {
                            tracing::info!("file reveal portal complete path={path_display}");
                            activate_parent_folder_after_reveal(&widget_for_focus, &focus_path);
                        }
                        Err(error) => {
                            tracing::warn!(
                                "file reveal portal failed path={path_display}: {error}"
                            );
                            fallback_reveal_file(&widget_for_fallback, fallback_path);
                        }
                    },
                );
            }
            Err(error) => {
                tracing::warn!("file reveal portal proxy failed path={path_display}: {error}");
                fallback_reveal_file(&widget_for_fallback, fallback_path);
            }
        },
    );
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn parent_window(widget: &gtk::Widget) -> Option<gtk::Window> {
    widget.root()?.downcast::<gtk::Window>().ok()
}

fn launch_file(widget: &gtk::Widget, path: PathBuf) {
    let path_display = path.display().to_string();
    let file = gio::File::for_path(path);
    let launcher = gtk::FileLauncher::new(Some(&file));
    let parent = parent_window(widget);
    launcher.launch(
        parent.as_ref(),
        None::<&gio::Cancellable>,
        move |result| match result {
            Ok(()) => tracing::info!("file manager opened path={path_display}"),
            Err(error) => tracing::warn!("file manager open failed path={path_display}: {error}"),
        },
    );
}

fn fallback_reveal_file(widget: &gtk::Widget, path: PathBuf) {
    let path_display = path.display().to_string();
    let file = gio::File::for_path(path);
    let launcher = gtk::FileLauncher::new(Some(&file));
    let parent = parent_window(widget);
    launcher.open_containing_folder(parent.as_ref(), None::<&gio::Cancellable>, move |result| {
        match result {
            Ok(()) => tracing::info!("file revealed path={path_display}"),
            Err(error) => tracing::warn!("file reveal failed path={path_display}: {error}"),
        }
    });
}

fn portal_activation_token(widget: &gtk::Widget, path: &Path) -> Option<glib::GString> {
    let context = widget.display().app_launch_context();
    context.set_timestamp(gdk::CURRENT_TIME);
    let files = [gio::File::for_path(path)];
    context.startup_notify_id(gio::AppInfo::NONE, &files)
}

fn activate_parent_folder_after_reveal(widget: &gtk::Widget, path: &Path) {
    let Some(parent_dir) = path.parent().map(PathBuf::from) else {
        return;
    };
    let widget = widget.clone();
    glib::timeout_add_local_once(Duration::from_millis(120), move || {
        launch_file(&widget, parent_dir);
    });
}
