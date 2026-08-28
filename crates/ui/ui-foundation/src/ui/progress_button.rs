use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use gtk::prelude::*;

thread_local! {
    static CSS_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

static NEXT_BUTTON_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ProgressButtonState {
    #[default]
    Idle,
    Indeterminate,
    Progress(f64),
}

#[derive(Clone)]
pub struct ProgressButton {
    button: gtk::Button,
    progress_css: gtk::CssProvider,
    state: Rc<Cell<ProgressButtonState>>,
}

impl ProgressButton {
    pub fn new(label: &str) -> Self {
        CSS_INSTALLED.with(|installed| {
            if installed.replace(true) {
                return;
            }
            let provider = gtk::CssProvider::new();
            provider.load_from_string(
                "@keyframes progress-button-unknown-move { \
                     0% { background-position: 0%; } \
                     50% { background-position: 100%; } \
                     100% { background-position: 0%; } \
                 } \
                 button.progress-button { \
                     background-image: linear-gradient(to top, @accent_bg_color 2px, \
                                                       alpha(@accent_bg_color, 0) 2px); \
                     background-repeat: no-repeat; \
                     background-position: 0 bottom; \
                     background-size: 0; \
                     transition: none; \
                 } \
                 button.progress-button.indeterminate { \
                     background-size: 25%; \
                     animation: progress-button-unknown-move infinite linear 2s; \
                 } \
                 button.progress-button:dir(rtl) { background-position: 100% bottom; }",
            );
            gtk::style_context_add_provider_for_display(
                &gtk::gdk::Display::default().expect("GTK display must exist"),
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });

        let button = gtk::Button::with_label(crate::i18n::text(label).as_ref());
        button.set_widget_name(&format!(
            "progress-button-{}",
            NEXT_BUTTON_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let progress_css = gtk::CssProvider::new();
        let display = button.display();
        gtk::style_context_add_provider_for_display(
            &display,
            &progress_css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        button.connect_destroy({
            let display = display.clone();
            let progress_css = progress_css.clone();
            move |_| gtk::style_context_remove_provider_for_display(&display, &progress_css)
        });

        Self {
            button,
            progress_css,
            state: Rc::new(Cell::new(ProgressButtonState::Idle)),
        }
    }

    pub fn widget(&self) -> &gtk::Button {
        &self.button
    }

    pub fn set_label(&self, key: &str) {
        self.button.set_label(crate::i18n::text(key).as_ref());
    }

    pub fn set_state(&self, state: ProgressButtonState) {
        if let ProgressButtonState::Progress(value) = state {
            assert!(value.is_finite(), "progress must be finite");
        }
        if self.state.get() == state {
            return;
        }
        self.state.set(state);
        match state {
            ProgressButtonState::Idle => {
                self.button.remove_css_class("progress-button");
                self.button.remove_css_class("indeterminate");
                self.progress_css.load_from_string("");
            }
            ProgressButtonState::Indeterminate => {
                self.button.add_css_class("progress-button");
                self.button.add_css_class("indeterminate");
                self.progress_css.load_from_string("");
            }
            ProgressButtonState::Progress(value) => {
                self.button.add_css_class("progress-button");
                self.button.remove_css_class("indeterminate");
                self.progress_css.load_from_string(&format!(
                    "#{} {{ background-size: {:.3}%; animation: none; }}",
                    self.button.widget_name(),
                    value.clamp(0.0, 1.0) * 100.0
                ));
            }
        }
    }
}
