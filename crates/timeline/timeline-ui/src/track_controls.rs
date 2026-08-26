use super::*;
use gtk::gdk;
use shrimply_ui_foundation::ui::I18nWidgetExt;

const TRACK_ADD_MENU_ICON_SIZE: i32 = 16;
const TRACK_ADD_MENU_ITEM_GAP: i32 = 12;

#[derive(Clone)]
struct TrackAddContext {
    area: gtk::GLArea,
    project: Rc<RefCell<Project>>,
    player_state: SharedPlayerState,
    selection_state: SharedSelectionState,
    runtime: Rc<RefCell<TimelineRuntime>>,
}

pub(super) fn timeline_sidebar(
    area: &gtk::GLArea,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    preferences: &preferences_store::SharedPreferences,
) -> gtk::Box {
    let toolbar = gtk::Box::new(gtk::Orientation::Vertical, 6);
    toolbar.set_width_request(SIDEBAR_WIDTH);
    toolbar.set_margin_top(4);
    toolbar.set_margin_bottom(4);
    toolbar.set_margin_start(4);
    toolbar.set_margin_end(4);

    let magnet = timeline_tool_button("magnet-tilted-symbolic");
    magnet.set_tooltip_i18n("Magnet");

    let beat_grid = timeline_tool_button("metronome-symbolic");
    beat_grid.set_tooltip_i18n("Beat Grid");

    let pointer = timeline_tool_button("pointer-primary-click-symbolic");
    pointer.set_tooltip_i18n("Pointer");

    let cut = timeline_tool_button("cut-symbolic");
    cut.set_tooltip_i18n("Cut");

    let overwrite = timeline_tool_button("track-insert-symbolic");
    overwrite.set_tooltip_i18n("Overwrite/Insert");

    let block = timeline_tool_button("track-block-symbolic");
    block.set_tooltip_i18n("Block");

    let new_track = timeline_tool_button("track-move-above-symbolic");
    new_track.set_tooltip_i18n("New Track");

    let snapshot = preferences_store::snapshot(preferences);
    magnet.set_active(timeline_magnet_from_preference(&snapshot.timeline_magnet));
    beat_grid.set_active(timeline_beat_grid_from_preference(
        &snapshot.timeline_beat_grid,
    ));
    pointer.set_active(snapshot.timeline_cursor == "pointer");
    cut.set_active(snapshot.timeline_cursor == "cut");
    overwrite.set_active(snapshot.timeline_drag_collision_mode == "overwrite");
    block.set_active(snapshot.timeline_drag_collision_mode == "block");
    new_track.set_active(snapshot.timeline_drag_collision_mode == "new_track");

    let runtime_for_magnet = runtime.clone();
    let area_for_magnet = area.clone();
    let preferences_for_magnet = preferences.clone();
    magnet.connect_toggled(move |button| {
        runtime_for_magnet.borrow_mut().snap_enabled = button.is_active();
        preferences_store::set_timeline_magnet(&preferences_for_magnet, button.is_active());
        area_for_magnet.queue_render();
    });

    let runtime_for_beat_grid = runtime.clone();
    let area_for_beat_grid = area.clone();
    let preferences_for_beat_grid = preferences.clone();
    beat_grid.connect_toggled(move |button| {
        runtime_for_beat_grid.borrow_mut().beat_grid_enabled = button.is_active();
        preferences_store::set_timeline_beat_grid(&preferences_for_beat_grid, button.is_active());
        area_for_beat_grid.queue_render();
    });

    let area_for_pointer = area.clone();
    let cut_for_pointer = cut.clone();
    let runtime_for_pointer = runtime.clone();
    let preferences_for_pointer = preferences.clone();
    pointer.connect_toggled(move |button| {
        if button.is_active() {
            cut_for_pointer.set_active(false);
            runtime_for_pointer.borrow_mut().cut_enabled = false;
            preferences_store::set_timeline_cursor(&preferences_for_pointer, "pointer");
            area_for_pointer.queue_render();
        } else if !cut_for_pointer.is_active() {
            button.set_active(true);
        }
    });

    let area_for_cut = area.clone();
    let pointer_for_cut = pointer.clone();
    let runtime_for_cut = runtime.clone();
    let preferences_for_cut = preferences.clone();
    cut.connect_toggled(move |button| {
        if button.is_active() {
            pointer_for_cut.set_active(false);
            runtime_for_cut.borrow_mut().cut_enabled = true;
            preferences_store::set_timeline_cursor(&preferences_for_cut, "cut");
            tracing::debug!("timeline cut tool enabled");
            area_for_cut.queue_render();
        } else if !pointer_for_cut.is_active() {
            runtime_for_cut.borrow_mut().cut_enabled = false;
            tracing::debug!("timeline cut tool disabled");
            button.set_active(true);
        }
    });

    let area_for_overwrite = area.clone();
    let block_for_overwrite = block.clone();
    let new_track_for_overwrite = new_track.clone();
    let runtime_for_overwrite = runtime.clone();
    let preferences_for_overwrite = preferences.clone();
    overwrite.connect_toggled(move |button| {
        if button.is_active() {
            block_for_overwrite.set_active(false);
            new_track_for_overwrite.set_active(false);
            runtime_for_overwrite.borrow_mut().drag_collision_mode = DragCollisionMode::Overwrite;
            preferences_store::set_timeline_drag_collision_mode(
                &preferences_for_overwrite,
                "overwrite",
            );
            area_for_overwrite.queue_render();
        } else if !block_for_overwrite.is_active() && !new_track_for_overwrite.is_active() {
            button.set_active(true);
        }
    });

    let area_for_block = area.clone();
    let overwrite_for_block = overwrite.clone();
    let new_track_for_block = new_track.clone();
    let runtime_for_block = runtime.clone();
    let preferences_for_block = preferences.clone();
    block.connect_toggled(move |button| {
        if button.is_active() {
            overwrite_for_block.set_active(false);
            new_track_for_block.set_active(false);
            runtime_for_block.borrow_mut().drag_collision_mode = DragCollisionMode::Block;
            preferences_store::set_timeline_drag_collision_mode(&preferences_for_block, "block");
            area_for_block.queue_render();
        } else if !overwrite_for_block.is_active() && !new_track_for_block.is_active() {
            button.set_active(true);
        }
    });

    let area_for_new_track = area.clone();
    let overwrite_for_new_track = overwrite.clone();
    let block_for_new_track = block.clone();
    let runtime_for_new_track = runtime.clone();
    let preferences_for_new_track = preferences.clone();
    new_track.connect_toggled(move |button| {
        if button.is_active() {
            overwrite_for_new_track.set_active(false);
            block_for_new_track.set_active(false);
            runtime_for_new_track.borrow_mut().drag_collision_mode = DragCollisionMode::NewTrack;
            preferences_store::set_timeline_drag_collision_mode(
                &preferences_for_new_track,
                "new_track",
            );
            area_for_new_track.queue_render();
        } else if !overwrite_for_new_track.is_active() && !block_for_new_track.is_active() {
            button.set_active(true);
        }
    });

    toolbar.append(&magnet);
    toolbar.append(&beat_grid);
    toolbar.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    toolbar.append(&pointer);
    toolbar.append(&cut);
    toolbar.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    toolbar.append(&overwrite);
    toolbar.append(&block);
    toolbar.append(&new_track);

    toolbar
}

pub(super) fn for_each_visible_track_row(
    project: &Project,
    view: TimelineViewState,
    height: f64,
    mut f: impl FnMut(&items::TrackRow, usize),
) {
    let (start_row, end_row) = visible_row_range(view, height);
    let rows = items::track_rows(project);
    for (row, track) in rows
        .iter()
        .enumerate()
        .take(end_row.min(rows.len()))
        .skip(start_row)
    {
        f(track, row);
    }
}

pub(super) fn visible_row_range(view: TimelineViewState, height: f64) -> (usize, usize) {
    let top = view.scroll_y.max(0.0);
    let bottom = (height - RULER_HEIGHT + view.scroll_y).max(top);
    (
        (top / TRACK_HEIGHT).floor().max(0.0) as usize,
        (bottom / TRACK_HEIGHT).ceil().max(0.0) as usize,
    )
}

pub(crate) fn track_label_action_at(
    project: &Project,
    view: TimelineViewState,
    x: f64,
    y: f64,
) -> Option<(TrackKey, TrackLabelAction)> {
    if !(0.0..timeline_x()).contains(&x) || y < RULER_HEIGHT {
        return None;
    }
    let content_y = y + view.scroll_y;
    let (kind, track_index, row) = items::track_at_y(project, content_y)?;
    let row_y = row_screen_y(row, view);
    let button_y = track_label_button_y(row_y);
    let key = TrackKey { kind, track_index };
    if hit_track_label_button(x, y, TRACK_LABEL_TOGGLE_X, button_y) {
        Some((key, TrackLabelAction::Toggle))
    } else if hit_track_label_button(x, y, TRACK_LABEL_ADD_X, button_y) {
        Some((key, TrackLabelAction::Add))
    } else if hit_track_label_button(x, y, TRACK_LABEL_RECORD_X, button_y) {
        match kind {
            TrackKind::Audio => Some((key, TrackLabelAction::AudioRecord)),
            TrackKind::Video => Some((key, TrackLabelAction::VideoRecord)),
            TrackKind::Caption => Some((key, TrackLabelAction::Select)),
        }
    } else {
        Some((key, TrackLabelAction::Select))
    }
}

pub(crate) fn track_button_at(
    project: &Project,
    view: TimelineViewState,
    x: f64,
    y: f64,
) -> Option<TrackButtonId> {
    let (key, action) = track_label_action_at(project, view, x, y)?;
    match action {
        TrackLabelAction::Select => None,
        _ => Some((key, action)),
    }
}

fn hit_track_label_button(x: f64, y: f64, button_x: f64, button_y: f64) -> bool {
    shrimply_skia_adw_ui::button::Button::hit_test(
        shrimply_skia_adw_ui::Rect::from_xywh(
            button_x as f32,
            button_y as f32,
            TRACK_LABEL_BUTTON_SIZE as f32,
            TRACK_LABEL_BUTTON_SIZE as f32,
        ),
        glam::Vec2::new(x as f32, y as f32),
    )
}

pub(super) fn track_label_button_y(row_y: f64) -> f64 {
    row_y + ((TRACK_HEIGHT - TRACK_LABEL_BUTTON_SIZE) / 2.0).max(0.0)
}

pub(super) fn track_enabled(project: &Project, key: TrackKey) -> bool {
    match key.kind {
        TrackKind::Caption => project
            .caption_tracks
            .get(key.track_index)
            .is_some_and(|track| track.enabled),
        TrackKind::Video => project
            .video_tracks
            .get(key.track_index)
            .is_some_and(|track| track.enabled),
        TrackKind::Audio => project
            .audio_tracks
            .get(key.track_index)
            .is_some_and(|track| track.enabled),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn show_track_add_menu(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    request: TrackAddMenuRequest,
) {
    let TrackAddMenuRequest {
        key,
        import_targets,
    } = request;
    let row = items::row_for_track(&project.borrow(), key.kind, key.track_index)
        .expect("add menu track must exist");
    let view = runtime.borrow().view;
    let context = TrackAddContext {
        area: area.clone(),
        project: project.clone(),
        player_state: player_state.clone(),
        selection_state: selection_state.clone(),
        runtime: runtime.clone(),
    };
    let popover = gtk::Popover::builder()
        .has_arrow(true)
        .position(gtk::PositionType::Bottom)
        .build();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    append_add_menu_item_i18n(
        &menu,
        &popover,
        if key.kind == TrackKind::Caption {
            "Import Captions…"
        } else {
            "Import Media…"
        },
        "document-open-symbolic",
        {
            let context = context.clone();
            move || {
                interaction::open_track_import_dialog(
                    &context.area,
                    &context.project,
                    &context.player_state,
                    &context.selection_state,
                    &context.runtime,
                    import_targets.clone(),
                )
            }
        },
    );

    match key.kind {
        TrackKind::Video => {
            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            add_video_items_to_menu(&menu, &popover, &context, key);
        }
        TrackKind::Audio => {
            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            add_audio_items_to_menu(&menu, &popover, &context, key);
        }
        TrackKind::Caption => {}
    }
    popover.set_child(Some(&menu));

    if let Some(existing) = runtime.borrow_mut().active_context_menu.take() {
        existing.popdown();
    }
    let button_y = track_label_button_y(row_screen_y(row, view));
    let parent = area.parent().expect("timeline GLArea must have a parent");
    let (x, y) = area
        .translate_coordinates(&parent, TRACK_LABEL_ADD_X, button_y)
        .expect("timeline coordinates must translate to its parent");
    popover.set_halign(gtk::Align::Center);
    popover.set_parent(&parent);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(
        x as i32,
        y as i32,
        TRACK_LABEL_BUTTON_SIZE as i32,
        TRACK_LABEL_BUTTON_SIZE as i32,
    )));
    popover.popup();
    runtime.borrow_mut().active_context_menu = Some(popover);
}

fn add_video_items_to_menu(
    menu: &gtk::Box,
    popover: &gtk::Popover,
    context: &TrackAddContext,
    key: TrackKey,
) {
    for (label, icon, kind) in [
        ("Text", "draw-text-symbolic", GeneratedItemKind::Text),
        ("Shape", "shapes-large-symbolic", GeneratedItemKind::Shape),
        (
            "Paint",
            "applications-graphics-symbolic",
            GeneratedItemKind::Paint,
        ),
        (
            "Background",
            "preferences-desktop-wallpaper-symbolic",
            GeneratedItemKind::Background,
        ),
        ("3D Scene", "3d-object-symbolic", GeneratedItemKind::Scene3d),
    ] {
        append_add_menu_item_i18n(menu, popover, label, icon, {
            let context = context.clone();
            move || {
                create_generated_item_at_playhead(
                    &context.area,
                    &context.project,
                    &context.player_state,
                    &context.selection_state,
                    &context.runtime,
                    key,
                    kind,
                )
            }
        });
    }
    append_add_menu_item_i18n(
        menu,
        popover,
        "Video Generation",
        "video-generation-symbolic",
        {
            let context = context.clone();
            move || {
                create_video_generation_item_at_playhead(
                    &context.area,
                    &context.project,
                    &context.player_state,
                    &context.selection_state,
                    &context.runtime,
                    key,
                )
            }
        },
    );
}

fn add_audio_items_to_menu(
    menu: &gtk::Box,
    popover: &gtk::Popover,
    context: &TrackAddContext,
    key: TrackKey,
) {
    append_add_menu_item_i18n(
        menu,
        popover,
        "Text to Speech",
        "font-x-generic-symbolic",
        {
            let context = context.clone();
            move || {
                create_tts_item_at_playhead(
                    &context.area,
                    &context.project,
                    &context.player_state,
                    &context.selection_state,
                    &context.runtime,
                    key,
                )
            }
        },
    );
    append_add_menu_item_i18n(menu, popover, "Audio Generator", "sound-symbolic", {
        let context = context.clone();
        move || {
            create_audio_generator_item_at_playhead(
                &context.area,
                &context.project,
                &context.player_state,
                &context.selection_state,
                &context.runtime,
                key,
            )
        }
    });
}

fn append_add_menu_item_i18n(
    menu: &gtk::Box,
    popover: &gtk::Popover,
    label: &str,
    icon: &str,
    activate: impl Fn() + 'static,
) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, TRACK_ADD_MENU_ITEM_GAP);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(TRACK_ADD_MENU_ICON_SIZE);
    let label = gtk::Label::new(Some(label));
    label.set_hexpand(true);
    label.set_xalign(0.0);
    content.append(&image);
    content.append(&label);

    let button = gtk::Button::builder()
        .has_frame(false)
        .child(&content)
        .build();
    let popover = popover.clone();
    button.connect_clicked(move |_| {
        popover.popdown();
        activate();
    });
    menu.append(&button);
}

pub(super) fn toggle_track_enabled(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: TrackKey,
) {
    let mut project_state = project.borrow_mut();
    let changed = match key.kind {
        TrackKind::Caption => project_state
            .caption_tracks
            .get_mut(key.track_index)
            .is_some_and(|track| {
                track.enabled = !track.enabled;
                true
            }),
        TrackKind::Video => project_state
            .video_tracks
            .get_mut(key.track_index)
            .is_some_and(|track| {
                track.enabled = !track.enabled;
                true
            }),
        TrackKind::Audio => project_state
            .audio_tracks
            .get_mut(key.track_index)
            .is_some_and(|track| {
                track.enabled = !track.enabled;
                true
            }),
    };
    if !changed {
        return;
    }
    crate::project::commit_edit(&project_state, "toggle-track-enabled");
    let duration = project_state.duration();
    drop(project_state);
    player_state::refresh_project(
        player_state,
        player_state::ProjectChange {
            duration: Some(duration),
            audio: key.kind == TrackKind::Audio,
            video: key.kind == TrackKind::Video,
            captions: key.kind == TrackKind::Caption,
            inspector: true,
            ..player_state::ProjectChange::default()
        },
    );
    area.queue_render();
}
