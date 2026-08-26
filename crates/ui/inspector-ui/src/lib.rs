mod alpha_mask;
mod audio;
mod audio_cache;
mod audio_generator;
mod audio_modifiers;
mod background;
mod benchmarking;
mod camera_source;
mod caption;
pub(crate) mod font_cache;
pub(crate) mod font_selector;
mod gaussian_3d;
mod generated;
mod info;
mod item;
mod keyframe_editor;
mod keyframe_graph;
mod keyframe_model;
mod list;
pub(crate) mod modifiers;
mod paint;
mod project;
mod rhai_editor;
mod scene_3d;
mod section;
mod selector;
mod text_input;
mod timeline_value;
mod track;
mod transform;
mod transition;
mod video;

pub const INSPECTOR_MIN_WIDTH: i32 = 320;

pub fn font_family_selector(
    selected: &shrimply_project::project::FontFamily,
    on_change: impl Fn(shrimply_project::project::FontFamily) + 'static,
) -> gtk::Widget {
    font_selector::project_font_selector(selected, on_change)
}

use std::cell::{Cell, RefCell};

pub use shrimply_evaluation as transform_eval;
pub use shrimply_math_color::Color;
pub use shrimply_project::time_format;
pub use shrimply_render_core::math;
pub use shrimply_state::player_state;
pub use shrimply_state::preview_focus;
pub use shrimply_timeline::selection_state;
pub use shrimply_ui_foundation::{desktop_open, skia_font, ui};

pub mod timeline {
    pub use shrimply_ui_foundation::{canvas as renderer, cursor};
}

use hashbrown::{HashMap, HashSet};
use std::rc::Rc;

use crate::player_state::SharedPlayerState;
use crate::preview_focus::SharedPreviewFocus;
use crate::selection_state::{SelectedItemKind, SelectedTrack, SharedSelectionState};
use gtk::glib;
use gtk::prelude::*;
use shrimply_project::project::{
    ItemAddress, ItemRef, Project, Time, TrackAddress, TransitionSide, VideoItem, VideoItemContent,
};
use shrimply_state::preferences::SharedPreferences;

use section::InspectorSection;
use track::TrackInspection;

type KeyframeGraphViewStates = Rc<RefCell<HashMap<String, keyframe_editor::SharedGraphViewState>>>;
pub(crate) type InspectedItem = ItemAddress;

#[derive(Clone, Debug, Eq, PartialEq)]
enum InspectorTarget {
    Project,
    Item(ItemAddress),
    Track(TrackAddress),
    Transition {
        item: ItemAddress,
        side: TransitionSide,
    },
}

#[derive(Clone)]
struct InspectorState {
    listener_scope: Rc<RefCell<Rc<()>>>,
    keyframe_graph_views: KeyframeGraphViewStates,
    fallback_focus: gtk::Widget,
    rebuild_pending: Rc<Cell<bool>>,
    expanded_rows: list::ExpandedRows,
    active_categories: list::ActiveCategories,
    volume: Rc<RefCell<shrimply_audio::streaming::FrameAudioSampler>>,
    preferences: SharedPreferences,
    preview_focus: SharedPreviewFocus,
    property_clipboard: shrimply_property_transfer::SharedClipboard,
    scroll_positions: Rc<RefCell<Vec<(InspectorTarget, f64)>>>,
    active_inspector: Rc<RefCell<Option<InspectorTarget>>>,
}

#[derive(Clone)]
pub(crate) struct InspectorContext {
    pub(crate) project: Rc<RefCell<Project>>,
    pub(crate) player_state: SharedPlayerState,
    pub(crate) selected_item: Option<InspectedItem>,
    pub(crate) volume: Rc<RefCell<shrimply_audio::streaming::FrameAudioSampler>>,
    pub(crate) preferences: SharedPreferences,
    pub(crate) preview_focus: SharedPreviewFocus,
    pub(crate) preview_item: Option<ItemAddress>,
    pub(crate) property_clipboard: shrimply_property_transfer::SharedClipboard,
    keyframe_graph_views: KeyframeGraphViewStates,
    // Per-row inspector listeners must use this scope instead of GTK weak refs.
    // Removed widgets can stay alive through signal/controller cycles after rebuild.
    pub(crate) listener_scope: Rc<()>,
    pub(crate) refresh: Rc<dyn Fn()>,
    expansion_target: Option<ItemAddress>,
    expanded_rows: list::ExpandedRows,
    category_bar: gtk::Box,
    active_categories: list::ActiveCategories,
}

impl InspectorContext {
    pub(crate) fn audio_analysis_at(
        &self,
        position: Time,
    ) -> shrimply_evaluation::FrameAudioAnalysis {
        let revision = player_state::snapshot(&self.player_state).revision;
        self.volume
            .borrow_mut()
            .sample(&self.project.borrow(), position, revision)
    }

    fn detached(&self) -> Self {
        let mut context = self.clone();
        context.listener_scope = Rc::new(());
        context
    }

    pub(crate) fn keyframe_graph_view_state(
        &self,
        scope: impl Into<String>,
    ) -> keyframe_editor::SharedGraphViewState {
        let selected = match &self.selected_item {
            Some(item) => {
                format!("{:?}:{}:{}", item.kind(), item.track_id(), item.item_id())
            }
            None => "none".to_string(),
        };
        let key = format!("{selected}:{}", scope.into());
        self.keyframe_graph_views
            .borrow_mut()
            .entry(key.clone())
            .or_insert_with(keyframe_editor::new_graph_view_state)
            .clone()
    }
}

pub(crate) trait Inspectable {
    fn title(&self) -> &'static str;
    fn add_rows(&self, section: &InspectorSection, context: &InspectorContext);

    fn default_action(&self, _context: &InspectorContext) -> Option<Box<dyn Fn() + 'static>> {
        None
    }

    fn controls(&self, context: &InspectorContext) -> Vec<gtk::Widget> {
        let section = InspectorSection::controls();
        self.add_rows(&section, context);
        vec![section.into_widget()]
    }

    fn inspect(&self, context: &InspectorContext) -> Vec<gtk::Widget> {
        let section = InspectorSection::new(self.title(), self.default_action(context));
        self.add_rows(&section, context);
        vec![section.into_widget()]
    }
}

pub fn new(
    project: Rc<RefCell<Project>>,
    player_state: SharedPlayerState,
    selection_state: SharedSelectionState,
    preview_focus: SharedPreviewFocus,
    preferences: SharedPreferences,
    fallback_focus: gtk::Widget,
    property_clipboard: shrimply_property_transfer::SharedClipboard,
) -> gtk::Widget {
    let default_font = shrimply_state::preferences::snapshot(&preferences).default_text_font_family;
    activate_project_google_fonts(&project.borrow(), &default_font, &player_state);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let category_bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    category_bar.set_visible(false);
    let state = InspectorState {
        listener_scope: Rc::new(RefCell::new(Rc::new(()))),
        keyframe_graph_views: Rc::new(RefCell::new(HashMap::new())),
        fallback_focus,
        rebuild_pending: Rc::new(Cell::new(false)),
        expanded_rows: Default::default(),
        active_categories: Default::default(),
        volume: Rc::new(RefCell::new(
            shrimply_audio::streaming::FrameAudioSampler::preview(48_000),
        )),
        preferences,
        preview_focus,
        property_clipboard,
        scroll_positions: Default::default(),
        active_inspector: Default::default(),
    };
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&content)
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(INSPECTOR_MIN_WIDTH)
        .build();
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&category_bar);
    layout.append(&scrolled);

    let selection_content = content.clone();
    let selection_category_bar = category_bar.clone();
    let selection_scrolled = scrolled.clone();
    let selection_project = project.clone();
    let selection_player_state = player_state.clone();
    let selection_state_for_listener = selection_state.clone();
    let selection_inspector_state = state.clone();
    selection_state::connect_named(&selection_state, "inspector selection rebuild", move || {
        queue_rebuild(
            &selection_content,
            &selection_category_bar,
            &selection_scrolled,
            &selection_project,
            &selection_player_state,
            &selection_state_for_listener,
            &selection_inspector_state,
        );
    });

    let project_content = content.clone();
    let project_category_bar = category_bar.clone();
    let project_scrolled = scrolled.clone();
    let project_for_listener = project.clone();
    let project_player_state = player_state.clone();
    let project_selection_state = selection_state.clone();
    let project_inspector_state = state.clone();
    player_state::connect_named(&player_state, "inspector project rebuild", move |event| {
        let crate::player_state::PlayerEvent::Project(change) = event else {
            return;
        };
        if !change.inspector {
            return;
        }
        queue_rebuild(
            &project_content,
            &project_category_bar,
            &project_scrolled,
            &project_for_listener,
            &project_player_state,
            &project_selection_state,
            &project_inspector_state,
        );
    });

    rebuild(
        &content,
        &category_bar,
        &scrolled,
        &project,
        &player_state,
        &selection_state,
        &state,
    );

    layout.upcast()
}

fn activate_project_google_fonts(
    project: &Project,
    default_font: &shrimply_project::project::FontFamily,
    player_state: &SharedPlayerState,
) {
    let mut families = HashSet::new();
    for item in project
        .video_tracks
        .iter()
        .chain(
            project
                .folded_sequences
                .iter()
                .flat_map(|sequence| &sequence.video_tracks),
        )
        .flat_map(|track| &track.items)
    {
        let mut add = |font_families: &[shrimply_project::project::FontFamily]| {
            for family in font_families {
                if let shrimply_project::project::FontFamily::GoogleFonts { name } = family
                    && !name.trim().is_empty()
                {
                    families.insert(name.clone());
                }
            }
        };
        if let VideoItemContent::Text(text) = &item.content {
            add(&text.font_families);
        }
        for modifier in &item.modifiers {
            let shrimply_video_modifiers::ModifierEffect::Scene3d(effect) = &modifier.effect else {
                continue;
            };
            if let shrimply_video_modifiers::scene_3d::Scene3dModifierEffect::Text(text) = &**effect
            {
                add(&text.font_families);
            }
        }
    }
    if let shrimply_project::project::FontFamily::GoogleFonts { name } = default_font
        && !name.trim().is_empty()
    {
        families.insert(name.clone());
    }
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::spawn(move || {
        for family in families {
            let result = font_cache::cached_family(&family).and_then(|cached| {
                if cached.is_none() {
                    let metadata = font_cache::lookup_google_family(&family)?;
                    font_cache::download_google_family(&metadata)?;
                }
                font_cache::materialize_family(&family).and_then(font_cache::activate_family)
            });
            if let Err(error) = result {
                tracing::warn!(family, "Could not activate project Google font: {error}");
            }
        }
        let _ = sender.send_blocking(());
    });
    let player_state = player_state.clone();
    glib::spawn_future_local(async move {
        if receiver.recv().await.is_ok() {
            player_state::refresh_project(
                &player_state,
                player_state::ProjectChange {
                    video: true,
                    ..player_state::ProjectChange::default()
                },
            );
        }
    });
}

fn queue_rebuild(
    content: &gtk::Box,
    category_bar: &gtk::Box,
    scrolled: &gtk::ScrolledWindow,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    state: &InspectorState,
) {
    if state.rebuild_pending.replace(true) {
        return;
    }
    // Retire the old tree's listeners before the current event continues dispatching.
    *state.listener_scope.borrow_mut() = Rc::new(());
    let content = content.clone();
    let category_bar = category_bar.clone();
    let scrolled = scrolled.clone();
    let project = project.clone();
    let player_state = player_state.clone();
    let selection_state = selection_state.clone();
    let state = state.clone();
    glib::idle_add_local_once(move || {
        if content.root().is_some() {
            rebuild(
                &content,
                &category_bar,
                &scrolled,
                &project,
                &player_state,
                &selection_state,
                &state,
            );
        }
        state.rebuild_pending.set(false);
    });
}

fn rebuild(
    content: &gtk::Box,
    category_bar: &gtk::Box,
    scrolled: &gtk::ScrolledWindow,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    state: &InspectorState,
) {
    let scoped_listeners = state.listener_scope.borrow().clone();
    if let Some(active) = state.active_inspector.borrow().as_ref() {
        let value = scrolled.vadjustment().value();
        let mut positions = state.scroll_positions.borrow_mut();
        if let Some((_, saved)) = positions.iter_mut().find(|(target, _)| target == active) {
            *saved = value;
        } else {
            positions.push((active.clone(), value));
        }
    }
    if content
        .root()
        .and_downcast::<gtk::Window>()
        .and_then(|window| GtkWindowExt::focus(&window))
        .is_some_and(|focus| focus.is_ancestor(content) || focus.is_ancestor(category_bar))
    {
        state.fallback_focus.grab_focus();
    }
    while let Some(child) = content.first_child() {
        content.remove(&child);
    }
    while let Some(child) = category_bar.first_child() {
        category_bar.remove(&child);
    }
    category_bar.set_visible(false);

    let target = inspector_target(project, selection_state);
    let scroll_value = state
        .scroll_positions
        .borrow()
        .iter()
        .find_map(|(saved, value)| (saved == &target).then_some(*value))
        .unwrap_or_default();
    *state.active_inspector.borrow_mut() = Some(target.clone());
    let (inspectable, selected_item, expansion_target, preview_item) =
        resolve_target(project, &target, &state.preview_focus);

    let context = InspectorContext {
        project: project.clone(),
        player_state: player_state.clone(),
        selected_item,
        volume: state.volume.clone(),
        preferences: state.preferences.clone(),
        preview_focus: state.preview_focus.clone(),
        preview_item,
        property_clipboard: state.property_clipboard.clone(),
        keyframe_graph_views: state.keyframe_graph_views.clone(),
        listener_scope: scoped_listeners,
        refresh: {
            let content = content.downgrade();
            let category_bar = category_bar.downgrade();
            let scrolled = scrolled.downgrade();
            let project = project.clone();
            let player_state = player_state.clone();
            let selection_state = selection_state.clone();
            let state = state.clone();
            Rc::new(move || {
                let (Some(content), Some(category_bar), Some(scrolled)) = (
                    content.upgrade(),
                    category_bar.upgrade(),
                    scrolled.upgrade(),
                ) else {
                    return;
                };
                queue_rebuild(
                    &content,
                    &category_bar,
                    &scrolled,
                    &project,
                    &player_state,
                    &selection_state,
                    &state,
                )
            })
        },
        expansion_target,
        expanded_rows: state.expanded_rows.clone(),
        category_bar: category_bar.clone(),
        active_categories: state.active_categories.clone(),
    };
    for widget in inspectable.inspect(&context) {
        content.append(&widget);
    }
    let scrolled = scrolled.clone();
    glib::idle_add_local_once(move || {
        let adjustment = scrolled.vadjustment();
        let max = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
        adjustment.set_value(scroll_value.clamp(adjustment.lower(), max));
    });
}

fn validate_preview_focus(
    preview_focus: &SharedPreviewFocus,
    address: &ItemAddress,
    item: &VideoItem,
) {
    let Some(focused) = preview_focus::snapshot(preview_focus) else {
        return;
    };
    if &focused.item != address {
        preview_focus::clear(preview_focus);
        return;
    }
    if !item.owns_preview_target(focused.target) {
        preview_focus::clear(preview_focus);
    }
}

fn inspector_target(
    project: &Rc<RefCell<Project>>,
    selection_state: &SharedSelectionState,
) -> InspectorTarget {
    let project = project.borrow();
    if let Some((item, side)) =
        selection_state::focused_transition_address(selection_state, &project)
    {
        return InspectorTarget::Transition { item, side };
    }
    if let Some(item) = selection_state::focused_item_address(selection_state, &project) {
        return InspectorTarget::Item(item);
    }
    focused_track_address(&project, selection_state)
        .map(InspectorTarget::Track)
        .unwrap_or(InspectorTarget::Project)
}

fn resolve_target(
    project: &Rc<RefCell<Project>>,
    target: &InspectorTarget,
    preview_focus: &SharedPreviewFocus,
) -> (
    Box<dyn Inspectable>,
    Option<ItemAddress>,
    Option<ItemAddress>,
    Option<ItemAddress>,
) {
    let mut preview_video = None;
    let resolved = {
        let project = project.borrow();
        match target {
            InspectorTarget::Transition { item, side } => {
                transition::resolve(&project, item, *side)
                    .map(|transition| {
                        (
                            Box::new(transition) as Box<dyn Inspectable>,
                            Some(item.clone()),
                            Some(item.clone()),
                            None,
                        )
                    })
                    .unwrap_or_else(|| project_target(&project))
            }
            InspectorTarget::Item(address) => match project.item(address) {
                Some(ItemRef::Caption(item)) => (
                    Box::new(item.clone()) as Box<dyn Inspectable>,
                    Some(address.clone()),
                    Some(address.clone()),
                    None,
                ),
                Some(ItemRef::Video(item)) => {
                    preview_video = Some((address.clone(), item.clone()));
                    (
                        Box::new(item.clone()) as Box<dyn Inspectable>,
                        Some(address.clone()),
                        Some(address.clone()),
                        Some(address.clone()),
                    )
                }
                Some(ItemRef::Audio(item)) => (
                    Box::new(item.clone()) as Box<dyn Inspectable>,
                    Some(address.clone()),
                    Some(address.clone()),
                    None,
                ),
                None => project_target(&project),
            },
            InspectorTarget::Track(address) => TrackInspection::resolve(&project, address.clone())
                .map(|track| (Box::new(track) as Box<dyn Inspectable>, None, None, None))
                .unwrap_or_else(|| project_target(&project)),
            InspectorTarget::Project => project_target(&project),
        }
    };
    if let Some((address, item)) = preview_video {
        validate_preview_focus(preview_focus, &address, &item);
    } else {
        preview_focus::clear(preview_focus);
    }
    resolved
}

fn project_target(
    project: &Project,
) -> (
    Box<dyn Inspectable>,
    Option<ItemAddress>,
    Option<ItemAddress>,
    Option<ItemAddress>,
) {
    (Box::new(project.clone()), None, None, None)
}

fn focused_track_address(
    project: &Project,
    selection_state: &SharedSelectionState,
) -> Option<TrackAddress> {
    let SelectedTrack { kind, track_index } = selection_state::focused_track(selection_state)?;
    Some(match kind {
        SelectedItemKind::Caption => TrackAddress::Caption {
            track_id: project.caption_tracks.get(track_index)?.id,
        },
        SelectedItemKind::Video => TrackAddress::Video {
            sequence_path: Vec::new(),
            track_id: project.video_tracks.get(track_index)?.id,
        },
        SelectedItemKind::Audio => TrackAddress::Audio {
            sequence_path: Vec::new(),
            track_id: project.audio_tracks.get(track_index)?.id,
        },
    })
}
