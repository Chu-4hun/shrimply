pub mod canvas;
pub mod cursor;
pub mod desktop_open;
pub mod export_feedback;
pub mod file_picker;
pub mod gl_loader;
pub mod icons;
pub mod i18n {
    pub use shrimply_i18n::{init_system_locale, text, text_args};
}
pub mod playback_shortcuts;
pub mod project_open;
pub mod project_settings;
pub mod resource_pipeline;
pub mod skia_font;
pub mod skia_system_font;
pub mod toast;
pub mod ui;

#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::text($key)
    };
}
