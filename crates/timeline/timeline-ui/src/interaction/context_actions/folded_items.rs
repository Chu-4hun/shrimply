use super::*;
use shrimply_ui_foundation::ui::I18nMenuExt;

#[allow(clippy::too_many_arguments)]
pub(super) fn show_folded_item_context_menu(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    runtime: &Rc<RefCell<TimelineRuntime>>,
    context: SequenceTimeline,
    hit: crate::project::ItemAddress,
    x: f64,
    y: f64,
) {
    let (folder, groupable, ungroupable, can_replace_properties, can_paste_modifiers) = {
        let project = project.borrow();
        let selected = selection_state::selected_item_addresses(selection_state, &project);
        let folder = match project.item(&hit) {
            Some(crate::project::ItemRef::Video(item)) => {
                matches!(
                    item.content,
                    crate::project::VideoItemContent::FoldedSequence(_)
                )
            }
            Some(crate::project::ItemRef::Audio(item)) => {
                matches!(item.source, crate::project::AudioSource::FoldedSequence(_))
            }
            Some(crate::project::ItemRef::Caption(_)) | None => false,
        };
        let clipboard = runtime.borrow().property_clipboard.clone();
        let clipboard = clipboard.borrow();
        (
            folder,
            selected.len() >= 2,
            selected
                .iter()
                .any(|item| crate::items::item_address_group_id(&project, item).is_some()),
            clipboard.can_replace_properties(&project, &selected),
            clipboard.can_append_modifiers(&project, &selected),
        )
    };

    let menu = gio::Menu::new();
    menu.append_i18n("Move Out", "timeline.move-out-of-sequence");

    if groupable || ungroupable {
        let section = gio::Menu::new();
        if groupable {
            section.append_i18n("Group", "timeline.group");
        }
        if ungroupable {
            section.append_i18n("Ungroup", "timeline.ungroup");
        }
        menu.append_section(None, &section);
    }

    if folder {
        let section = gio::Menu::new();
        section.append_i18n("Add Track at Top", "timeline.add-folder-track-top");
        section.append_i18n("Add Track at Bottom", "timeline.add-folder-track-bottom");
        menu.append_section(None, &section);
    }

    let property_section = gio::Menu::new();
    property_section.append_i18n("Replace Properties", "timeline.replace-properties");
    property_section.append_i18n("Paste Modifiers", "timeline.paste-modifiers");
    menu.append_section(None, &property_section);

    let actions = gio::SimpleActionGroup::new();
    add_menu_action(&actions, "move-out-of-sequence", {
        let area = area.clone();
        let project = project.clone();
        let player_state = player_state.clone();
        let selection_state = selection_state.clone();
        let hit = hit.clone();
        move || {
            move_item_out_of_sequence(
                &area,
                &project,
                &player_state,
                &selection_state,
                &context,
                &hit,
            );
        }
    });
    if groupable {
        add_menu_action(&actions, "group", {
            let area = area.clone();
            let project = project.clone();
            let selection_state = selection_state.clone();
            move || group_selected_timeline_items(&area, &project, &selection_state)
        });
    }
    if ungroupable {
        add_menu_action(&actions, "ungroup", {
            let area = area.clone();
            let project = project.clone();
            let selection_state = selection_state.clone();
            move || ungroup_selected_timeline_items(&area, &project, &selection_state)
        });
    }
    if folder {
        for (name, at_top) in [
            ("add-folder-track-top", true),
            ("add-folder-track-bottom", false),
        ] {
            add_menu_action(&actions, name, {
                let area = area.clone();
                let project = project.clone();
                let player_state = player_state.clone();
                let folder = hit.clone();
                move || create_folded_track(&area, &project, &player_state, &folder, at_top)
            });
        }
    }
    add_menu_action_enabled(&actions, "replace-properties", can_replace_properties, {
        let area = area.clone();
        let project = project.clone();
        let player_state = player_state.clone();
        let selection_state = selection_state.clone();
        let runtime = runtime.clone();
        move || {
            replace_selected_item_properties(
                &area,
                &project,
                &player_state,
                &selection_state,
                &runtime,
            );
        }
    });
    add_menu_action_enabled(&actions, "paste-modifiers", can_paste_modifiers, {
        let area = area.clone();
        let project = project.clone();
        let player_state = player_state.clone();
        let selection_state = selection_state.clone();
        let runtime = runtime.clone();
        move || {
            append_selected_item_modifiers(
                &area,
                &project,
                &player_state,
                &selection_state,
                &runtime,
            );
        }
    });

    popup_timeline_context_menu(area, runtime, &menu, &actions, None, x, y);
    area.queue_render();
}

fn move_item_out_of_sequence(
    area: &gtk::GLArea,
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    selection_state: &SharedSelectionState,
    context: &dyn TimelineOperationContext,
    address: &crate::project::ItemAddress,
) {
    let (moved, kind, duration) = {
        let mut project = project.borrow_mut();
        let Some(moved) = context.move_item_out(&mut project, address) else {
            return;
        };
        let kind = moved.kind();
        let duration = project.duration();
        project.normalize_clip_transitions();
        crate::project::commit_edit(&project, "move-timeline-item-out-of-sequence");
        (moved, kind, duration)
    };
    let project = project.borrow();
    selection_state::set_selected_item_addresses(
        selection_state,
        &project,
        vec![moved.clone()],
        Some(moved),
    );
    drop(project);
    player_state::refresh_project(
        player_state,
        ProjectChange {
            duration: Some(duration),
            audio: kind == crate::project::ItemKind::Audio,
            audio_beats: kind == crate::project::ItemKind::Audio,
            audio_waveforms: kind == crate::project::ItemKind::Audio,
            video: kind == crate::project::ItemKind::Video,
            live_preview: false,
            captions: kind == crate::project::ItemKind::Caption,
            inspector: true,
        },
    );
    area.queue_render();
}
