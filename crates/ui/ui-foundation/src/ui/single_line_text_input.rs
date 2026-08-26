use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

pub struct SingleLineTextInput;

pub struct SingleLineTextInputBuilder {
    value: String,
    placeholder: Option<String>,
    max_length: Option<usize>,
    on_change: Option<Box<dyn Fn(String)>>,
    on_commit: Option<Box<dyn Fn(String)>>,
}

impl SingleLineTextInput {
    pub fn builder(value: impl Into<String>) -> SingleLineTextInputBuilder {
        SingleLineTextInputBuilder {
            value: value.into(),
            placeholder: None,
            max_length: None,
            on_change: None,
            on_commit: None,
        }
    }
}

impl SingleLineTextInputBuilder {
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub fn on_change(mut self, on_change: impl Fn(String) + 'static) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_commit(mut self, on_commit: impl Fn(String) + 'static) -> Self {
        self.on_commit = Some(Box::new(on_commit));
        self
    }

    pub fn build(self) -> gtk::Entry {
        let placeholder = self.placeholder.as_deref().map(crate::i18n::text);
        let entry = gtk::Entry::builder()
            .text(&self.value)
            .hexpand(true)
            .max_length(
                self.max_length
                    .map_or(0, |length| i32::try_from(length).unwrap_or(i32::MAX)),
            )
            .css_classes(["inspector-selector"])
            .build();
        entry.set_placeholder_text(placeholder.as_deref());
        let dirty = Rc::new(Cell::new(false));
        if let Some(on_change) = self.on_change {
            let dirty = dirty.clone();
            entry.connect_changed(move |entry| {
                dirty.set(true);
                on_change(entry.text().to_string());
            });
        } else {
            let dirty = dirty.clone();
            entry.connect_changed(move |_| dirty.set(true));
        }

        if let Some(on_commit) = self.on_commit {
            let on_commit: Rc<dyn Fn(String)> = Rc::from(on_commit);
            let commit = {
                let entry = entry.clone();
                let dirty = dirty.clone();
                let on_commit = on_commit.clone();
                move || {
                    if dirty.replace(false) {
                        on_commit(entry.text().to_string());
                    }
                }
            };
            entry.connect_activate(move |_| commit());

            let focus = gtk::EventControllerFocus::new();
            focus.connect_leave({
                let entry = entry.clone();
                move |_| {
                    if dirty.replace(false) {
                        on_commit(entry.text().to_string());
                    }
                }
            });
            entry.add_controller(focus);
        }
        entry
    }
}
