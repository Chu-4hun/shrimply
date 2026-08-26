use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

use gtk::{gio, glib, prelude::*};

static LAST_FOLDERS: LazyLock<Mutex<HashMap<String, PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn open<P: IsA<gtk::Window>>(
    label: &str,
    dialog: &gtk::FileDialog,
    parent: Option<&P>,
    selected: impl FnOnce(Result<gio::File, glib::Error>) + 'static,
) {
    restore_folder(label, dialog);
    let label = label.to_owned();
    dialog.open(parent, None::<&gio::Cancellable>, move |result| {
        remember_parent(&label, &result);
        selected(result);
    });
}

pub fn save<P: IsA<gtk::Window>>(
    label: &str,
    dialog: &gtk::FileDialog,
    parent: Option<&P>,
    selected: impl FnOnce(Result<gio::File, glib::Error>) + 'static,
) {
    restore_folder(label, dialog);
    let label = label.to_owned();
    dialog.save(parent, None::<&gio::Cancellable>, move |result| {
        remember_parent(&label, &result);
        selected(result);
    });
}

fn restore_folder(label: &str, dialog: &gtk::FileDialog) {
    let folder = LAST_FOLDERS
        .lock()
        .expect("file picker cache lock poisoned")
        .get(label)
        .cloned();
    if let Some(folder) = folder {
        dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
    }
}

fn remember_parent(label: &str, result: &Result<gio::File, glib::Error>) {
    let Some(folder) = result
        .as_ref()
        .ok()
        .and_then(gio::File::path)
        .and_then(|path| path.parent().map(PathBuf::from))
    else {
        return;
    };
    LAST_FOLDERS
        .lock()
        .expect("file picker cache lock poisoned")
        .insert(label.to_owned(), folder);
}
