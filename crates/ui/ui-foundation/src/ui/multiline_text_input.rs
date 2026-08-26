use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::glib::{self, SourceId};
use gtk::prelude::*;
use gtk::{gio, gio::prelude::ActionMapExt};
use sourceview5::prelude::*;

const MAX_TYPO_CORRECTIONS: usize = 6;
const TEXT_COMMIT_DEBOUNCE: Duration = Duration::from_millis(750);
const DEFAULT_MIN_CONTENT_HEIGHT: i32 = 96;
const TEXT_MARGIN: i32 = 8;

#[derive(Clone)]
pub struct MultilineTextInput {
    widget: gtk::Widget,
    set_text: Rc<dyn Fn(&str)>,
}

pub struct MultilineTextInputBuilder {
    value: String,
    min_content_height: i32,
    max_length: Option<usize>,
    on_change: Option<Box<dyn Fn(String) -> bool>>,
    on_commit: Option<Box<dyn Fn()>>,
}

impl MultilineTextInput {
    pub fn builder(value: impl Into<String>) -> MultilineTextInputBuilder {
        MultilineTextInputBuilder {
            value: value.into(),
            min_content_height: DEFAULT_MIN_CONTENT_HEIGHT,
            max_length: None,
            on_change: None,
            on_commit: None,
        }
    }

    pub fn widget(&self) -> &gtk::Widget {
        &self.widget
    }

    pub fn set_text(&self, text: &str) {
        (self.set_text)(text);
    }

    pub fn set_text_handler(&self) -> Rc<dyn Fn(&str)> {
        self.set_text.clone()
    }
}

impl MultilineTextInputBuilder {
    pub fn min_content_height(mut self, min_content_height: i32) -> Self {
        self.min_content_height = min_content_height;
        self
    }

    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub fn on_change(mut self, on_change: impl Fn(String) -> bool + 'static) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_commit(mut self, on_commit: impl Fn() + 'static) -> Self {
        self.on_commit = Some(Box::new(on_commit));
        self
    }

    pub fn build(self) -> MultilineTextInput {
        let value = limited_text(&self.value, self.max_length);
        let buffer = sourceview5::Buffer::new(None);
        set_text_style_scheme(&buffer);
        buffer.set_text(&value);
        let typo_tag = gtk::TextTag::builder()
            .name("typo")
            .underline(gtk::pango::Underline::Error)
            .underline_rgba(&gtk::gdk::RGBA::new(0.92, 0.18, 0.18, 1.0))
            .build();
        buffer.tag_table().add(&typo_tag);
        let typo_marks = Rc::new(RefCell::new(Vec::new()));
        let syncing = Rc::new(Cell::new(false));
        update_typo_marks(&buffer, &typo_tag, &typo_marks);

        let view = sourceview5::View::with_buffer(&buffer);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_height_request(self.min_content_height);
        view.set_hexpand(true);
        view.set_monospace(false);
        view.set_show_line_numbers(false);
        view.set_top_margin(TEXT_MARGIN);
        view.set_bottom_margin(TEXT_MARGIN);
        view.set_left_margin(TEXT_MARGIN);
        view.set_right_margin(TEXT_MARGIN);
        view.set_has_tooltip(true);
        connect_typo_tooltips(&view, typo_marks.clone());
        connect_typo_context_menu(&view, &buffer, typo_marks.clone());

        let pending_commit = Rc::new(RefCell::new(PendingCommit::new(&value)));
        let on_commit: Rc<dyn Fn()> = match self.on_commit {
            Some(on_commit) => Rc::from(on_commit),
            None => Rc::new(|| {}),
        };
        let on_change: Rc<dyn Fn(String) -> bool> = match self.on_change {
            Some(on_change) => Rc::from(on_change),
            None => Rc::new(|_| false),
        };
        let changed_pending_commit = pending_commit.clone();
        let changed_on_commit = on_commit.clone();
        let changed_syncing = syncing.clone();
        let changed_typo_marks = typo_marks.clone();
        buffer.connect_changed(move |buffer| {
            if changed_syncing.get() {
                update_typo_marks(buffer, &typo_tag, &changed_typo_marks);
                return;
            }
            let (start, end) = buffer.bounds();
            let entered = buffer.text(&start, &end, true).to_string();
            let text = limited_text(&entered, self.max_length);
            if text != entered {
                changed_syncing.set(true);
                buffer.set_text(&text);
                buffer.place_cursor(&buffer.end_iter());
                changed_syncing.set(false);
            }
            update_typo_marks(buffer, &typo_tag, &changed_typo_marks);
            if on_change(text.clone()) {
                schedule_pending_commit(&changed_pending_commit, changed_on_commit.clone(), text);
            }
        });

        let focus = gtk::EventControllerFocus::new();
        let focus_pending_commit = pending_commit.clone();
        let focus_on_commit = on_commit.clone();
        focus.connect_leave(move |_| {
            flush_pending_commit(&focus_pending_commit, &focus_on_commit, true);
        });
        view.add_controller(focus);

        let scroller = gtk::ScrolledWindow::builder()
            .child(&view)
            .min_content_height(self.min_content_height)
            .hexpand(true)
            .build();
        let destroy_pending_commit = pending_commit.clone();
        scroller.connect_destroy(move |_| {
            flush_pending_commit(&destroy_pending_commit, &on_commit, true);
        });
        let set_text = {
            let buffer = buffer.clone();
            let view = view.clone();
            let pending_commit = pending_commit.clone();
            let max_length = self.max_length;
            Rc::new(move |text: &str| {
                if view.has_focus() {
                    return;
                }
                let text = limited_text(text, max_length);
                let (start, end) = buffer.bounds();
                if buffer.text(&start, &end, true).as_str() == text {
                    return;
                }
                syncing.set(true);
                buffer.set_text(&text);
                syncing.set(false);
                let mut pending = pending_commit.borrow_mut();
                if let Some(source_id) = pending.source_id.take() {
                    source_id.remove();
                }
                pending.dirty = false;
                text.clone_into(&mut pending.latest_text);
                text.clone_into(&mut pending.committed_text);
            }) as Rc<dyn Fn(&str)>
        };
        MultilineTextInput {
            widget: scroller.upcast(),
            set_text,
        }
    }
}

fn limited_text(text: &str, max_length: Option<usize>) -> String {
    let Some(max_length) = max_length else {
        return text.to_string();
    };
    text.chars().take(max_length).collect()
}

struct PendingCommit {
    source_id: Option<SourceId>,
    dirty: bool,
    latest_text: String,
    committed_text: String,
}

impl PendingCommit {
    fn new(value: &str) -> Self {
        Self {
            source_id: None,
            dirty: false,
            latest_text: value.to_string(),
            committed_text: value.to_string(),
        }
    }
}

fn schedule_pending_commit(
    pending: &Rc<RefCell<PendingCommit>>,
    on_commit: Rc<dyn Fn()>,
    text: String,
) {
    let timeout_pending = pending.clone();
    let timeout_on_commit = on_commit.clone();
    let source_id = glib::timeout_add_local_once(TEXT_COMMIT_DEBOUNCE, move || {
        flush_pending_commit(&timeout_pending, &timeout_on_commit, false);
    });

    let mut pending = pending.borrow_mut();
    if let Some(source_id) = pending.source_id.replace(source_id) {
        source_id.remove();
    }
    pending.dirty = true;
    pending.latest_text = text;
}

fn flush_pending_commit(
    pending: &Rc<RefCell<PendingCommit>>,
    on_commit: &Rc<dyn Fn()>,
    remove_source: bool,
) {
    let should_commit = {
        let mut pending = pending.borrow_mut();
        if !pending.dirty {
            return;
        }
        if let Some(source_id) = pending.source_id.take()
            && remove_source
        {
            source_id.remove();
        }
        if pending.latest_text == pending.committed_text {
            pending.dirty = false;
            return;
        }
        pending.committed_text = pending.latest_text.clone();
        pending.dirty = false;
        true
    };
    if should_commit {
        on_commit();
    }
}

#[derive(Clone)]
struct TypoMark {
    start: i32,
    end: i32,
    message: String,
    corrections: Vec<String>,
}

fn update_typo_marks(
    buffer: &sourceview5::Buffer,
    typo_tag: &gtk::TextTag,
    typo_marks: &Rc<RefCell<Vec<TypoMark>>>,
) {
    let (start, end) = buffer.bounds();
    buffer.remove_tag(typo_tag, &start, &end);
    let text = buffer.text(&start, &end, true).to_string();
    let marks = text_typo_marks(&text);
    for mark in &marks {
        let start = buffer.iter_at_offset(mark.start);
        let end = buffer.iter_at_offset(mark.end);
        buffer.apply_tag(typo_tag, &start, &end);
    }
    *typo_marks.borrow_mut() = marks;
}

fn text_typo_marks(text: &str) -> Vec<TypoMark> {
    let tokenizer = typos::tokens::Tokenizer::new();
    let mut marks = Vec::new();
    for ident in tokenizer.parse_str(text) {
        for word in ident.split() {
            let Some(corrections) = typos_dict::WORD.find(&unicase::UniCase::new(word.token()))
            else {
                continue;
            };
            let start = char_offset(text, word.offset());
            let end = char_offset(text, word.offset() + word.token().len());
            let message = if corrections.is_empty() {
                format!("Possible typo: {}", word.token())
            } else {
                format!(
                    "Possible typo: {} -> {}",
                    word.token(),
                    corrections.join(", ")
                )
            };
            marks.push(TypoMark {
                start,
                end,
                message,
                corrections: corrections
                    .iter()
                    .map(|correction| correction.to_string())
                    .collect(),
            });
        }
    }
    marks
}

fn char_offset(text: &str, byte_offset: usize) -> i32 {
    text[..byte_offset].chars().count().min(i32::MAX as usize) as i32
}

fn connect_typo_tooltips(view: &sourceview5::View, typo_marks: Rc<RefCell<Vec<TypoMark>>>) {
    view.connect_query_tooltip(move |view, x, y, keyboard_tooltip, tooltip| {
        let offset = if keyboard_tooltip {
            view.buffer().cursor_position()
        } else {
            let (x, y) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x, y);
            let Some(iter) = view.iter_at_location(x, y) else {
                return false;
            };
            iter.offset()
        };
        let typo_marks = typo_marks.borrow();
        let Some(mark) = typo_marks
            .iter()
            .find(|mark| offset >= mark.start && offset < mark.end)
        else {
            return false;
        };
        tooltip.set_text(Some(&mark.message));
        true
    });
}

fn connect_typo_context_menu(
    view: &sourceview5::View,
    buffer: &sourceview5::Buffer,
    typo_marks: Rc<RefCell<Vec<TypoMark>>>,
) {
    let menu = gio::Menu::new();
    let actions = gio::SimpleActionGroup::new();
    view.set_extra_menu(Some(&menu));
    view.insert_action_group("texttypo", Some(&actions));

    let secondary_click = gtk::GestureClick::new();
    secondary_click.set_button(gtk::gdk::BUTTON_SECONDARY);
    secondary_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let event_view = view.clone();
    let event_buffer = buffer.clone();
    let event_menu = menu.clone();
    let event_actions = actions.clone();
    secondary_click.connect_pressed(move |_, _, x, y| {
        let mark = typo_mark_at_view_position(&event_view, &typo_marks, x, y);
        set_typo_context_menu(&event_menu, &event_actions, &event_buffer, mark);
    });
    view.add_controller(secondary_click);

    buffer.connect_changed(move |_| {
        clear_typo_context_menu(&menu, &actions);
    });
}

fn set_typo_context_menu(
    menu: &gio::Menu,
    actions: &gio::SimpleActionGroup,
    buffer: &sourceview5::Buffer,
    mark: Option<TypoMark>,
) {
    clear_typo_context_menu(menu, actions);
    let Some(mark) = mark else {
        return;
    };
    if mark.corrections.is_empty() {
        return;
    }

    let section = gio::Menu::new();
    for (index, correction) in mark
        .corrections
        .iter()
        .take(MAX_TYPO_CORRECTIONS)
        .enumerate()
    {
        let action_name = format!("fix{index}");
        section.append(
            Some(&format!("Fix typo: {correction}")),
            Some(&format!("texttypo.{action_name}")),
        );

        let action = gio::SimpleAction::new(&action_name, None);
        let buffer = buffer.clone();
        let correction = correction.clone();
        let mark = mark.clone();
        action.connect_activate(move |_, _| {
            replace_text_range(&buffer, mark.start, mark.end, &correction);
        });
        actions.add_action(&action);
    }
    menu.append_section(None, &section);
}

fn clear_typo_context_menu(menu: &gio::Menu, actions: &gio::SimpleActionGroup) {
    menu.remove_all();
    for index in 0..MAX_TYPO_CORRECTIONS {
        actions.remove_action(&format!("fix{index}"));
    }
}

fn typo_mark_at_view_position(
    view: &sourceview5::View,
    typo_marks: &Rc<RefCell<Vec<TypoMark>>>,
    x: f64,
    y: f64,
) -> Option<TypoMark> {
    let (x, y) = view.window_to_buffer_coords(
        gtk::TextWindowType::Widget,
        x.round() as i32,
        y.round() as i32,
    );
    let iter = view.iter_at_location(x, y)?;
    typo_mark_at_offset(typo_marks, iter.offset())
}

fn typo_mark_at_offset(typo_marks: &Rc<RefCell<Vec<TypoMark>>>, offset: i32) -> Option<TypoMark> {
    typo_marks
        .borrow()
        .iter()
        .find(|mark| offset >= mark.start && offset < mark.end)
        .cloned()
}

fn replace_text_range(buffer: &sourceview5::Buffer, start: i32, end: i32, replacement: &str) {
    let mut start = buffer.iter_at_offset(start);
    let mut end = buffer.iter_at_offset(end);
    buffer.delete(&mut start, &mut end);
    buffer.insert(&mut start, replacement);
}

fn set_text_style_scheme(buffer: &sourceview5::Buffer) {
    let manager = sourceview5::StyleSchemeManager::default();
    let scheme_name = if adw::StyleManager::default().is_dark() {
        "Adwaita-dark"
    } else {
        "Adwaita"
    };
    if let Some(scheme) = manager.scheme(scheme_name) {
        buffer.set_style_scheme(Some(&scheme));
    }
}
