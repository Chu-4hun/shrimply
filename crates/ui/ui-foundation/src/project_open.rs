use std::path::PathBuf;

use gtk::gio;
use gtk::prelude::*;

use crate::ui::I18nFileFilterExt;

pub fn project_file_filter() -> gtk::FileFilter {
    let filter = gtk::FileFilter::new();
    filter.set_name_i18n("Shrimply projects");
    filter.add_pattern("*.shrimp");
    filter.add_pattern("*.json");
    filter.add_pattern("*.otio");
    filter.add_pattern("*.kdenlive");
    filter
}

pub fn open_project(
    parent: &impl IsA<gtk::Window>,
    selected: impl FnOnce(Result<Option<PathBuf>, String>) + 'static,
) {
    let label = "Open Project";
    let filter = project_file_filter();
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder()
        .title(crate::i18n::text(label).as_ref())
        .filters(&filters)
        .default_filter(&filter)
        .build();
    crate::file_picker::open(label, &dialog, Some(parent), move |result| match result {
        Err(_) => selected(Ok(None)),
        Ok(file) => selected(
            file.path()
                .map(Some)
                .ok_or_else(|| "The selected file does not have a local path.".to_owned()),
        ),
    });
}
