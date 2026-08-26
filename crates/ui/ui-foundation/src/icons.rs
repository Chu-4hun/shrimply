use std::path::Path;

pub fn register_bundled() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    gtk::IconTheme::for_display(&display)
        .add_search_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/icons"));
}
