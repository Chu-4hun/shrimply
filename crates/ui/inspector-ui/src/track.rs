use adw::prelude::*;
use shrimply_ui_foundation::tr;

use crate::player_state::{self, ProjectChange};
use shrimply_project::project::{ItemKind, Project, TrackAddress, TrackMut, TrackRef};

use super::{Inspectable, InspectorContext, item::flat, list, section::InspectorSection};

pub(super) struct TrackInspection {
    address: TrackAddress,
    kind: ItemKind,
    ordinal: usize,
    pub(super) enabled: bool,
    pub(super) item_count: usize,
}

impl TrackInspection {
    pub(super) fn resolve(project: &Project, address: TrackAddress) -> Option<Self> {
        let (enabled, item_count) = match project.track(&address)? {
            TrackRef::Caption(track) => (track.enabled, track.items.len()),
            TrackRef::Video(track) => (track.enabled, track.items.len()),
            TrackRef::Audio(track) => (track.enabled, track.items.len()),
        };
        let ordinal = match &address {
            TrackAddress::Caption { track_id } => project
                .caption_tracks
                .iter()
                .position(|track| track.id == *track_id)?,
            TrackAddress::Video {
                sequence_path,
                track_id,
            } => project
                .video_tracks_for_path(sequence_path)?
                .iter()
                .position(|track| track.id == *track_id)?,
            TrackAddress::Audio {
                sequence_path,
                track_id,
            } => project
                .audio_tracks_for_path(sequence_path)?
                .iter()
                .position(|track| track.id == *track_id)?,
        };
        Some(Self {
            kind: address.kind(),
            address,
            ordinal,
            enabled,
            item_count,
        })
    }
}

impl Inspectable for TrackInspection {
    fn title(&self) -> &'static str {
        match self.kind {
            ItemKind::Video => "Video Track",
            ItemKind::Caption => "Caption Track",
            ItemKind::Audio => "Audio Track",
        }
    }

    fn add_rows(&self, _section: &InspectorSection, _context: &InspectorContext) {}

    fn inspect(&self, context: &InspectorContext) -> Vec<gtk::Widget> {
        let actions = adw::PreferencesGroup::new();
        let enabled = adw::ActionRow::builder()
            .title(tr!("Enabled").as_ref())
            .subtitle(tr!("Include this track in playback and export").as_ref())
            .build();
        let enabled_toggle = gtk::Switch::builder()
            .active(self.enabled)
            .valign(gtk::Align::Center)
            .build();
        enabled.add_suffix(&enabled_toggle);
        enabled.set_activatable_widget(Some(&enabled_toggle));
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let kind = self.kind;
        let address = self.address.clone();
        enabled_toggle.connect_active_notify(move |toggle| {
            let next = toggle.is_active();
            let mut project = project.borrow_mut();
            let Some(track) = project.track_mut(&address) else {
                return;
            };
            let enabled = match track {
                TrackMut::Caption(track) => &mut track.enabled,
                TrackMut::Video(track) => &mut track.enabled,
                TrackMut::Audio(track) => &mut track.enabled,
            };
            if *enabled == next {
                return;
            }
            *enabled = next;
            shrimply_project::project::commit_edit(&project, "toggle-track-enabled");
            let duration = project.duration();
            drop(project);
            player_state::refresh_project(
                &player_state,
                ProjectChange {
                    duration: Some(duration),
                    frame_rate: None,
                    audio: kind == ItemKind::Audio,
                    audio_beats: kind == ItemKind::Audio,
                    audio_waveforms: kind == ItemKind::Audio,
                    video: kind == ItemKind::Video,
                    live_preview: false,
                    captions: kind == ItemKind::Caption,
                    inspector: true,
                },
            );
        });
        actions.add(&enabled);

        let info = adw::PreferencesGroup::new();
        info.add(
            &adw::ActionRow::builder()
                .title(tr!("Type").as_ref())
                .subtitle(self.title())
                .build(),
        );
        info.add(
            &adw::ActionRow::builder()
                .title(tr!("Track").as_ref())
                .subtitle((self.ordinal + 1).to_string())
                .build(),
        );
        info.add(
            &adw::ActionRow::builder()
                .title(tr!("Items").as_ref())
                .subtitle(self.item_count.to_string())
                .build(),
        );

        list::render_categories(
            vec![
                list::InspectorCategory {
                    key: "actions",
                    label: "Actions",
                    icon: "general-properties-symbolic",
                    items: vec![flat(actions)],
                },
                list::InspectorCategory {
                    key: "info",
                    label: "Info",
                    icon: "info-outline-symbolic",
                    items: vec![flat(info)],
                },
            ],
            context,
        )
    }
}
