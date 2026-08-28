use adw::prelude::*;
use shrimply_ui_foundation::{
    tr,
    ui::{StringChoice, labeled_string_selector, switch_row},
};

use crate::player_state::{self, ProjectChange};
use shrimply_project::project::{
    ItemKind, Project, TrackAddress, TrackMut, TrackRef, caption_languages,
    supported_caption_language,
};

use super::{Inspectable, InspectorContext, item::flat, list, section::InspectorSection};

pub(super) struct TrackInspection {
    address: TrackAddress,
    kind: ItemKind,
    ordinal: usize,
    pub(super) enabled: bool,
    pub(super) language: Option<String>,
    pub(super) item_count: usize,
}

impl TrackInspection {
    pub(super) fn resolve(project: &Project, address: TrackAddress) -> Option<Self> {
        let (enabled, language, item_count) = match project.track(&address)? {
            TrackRef::Caption(track) => (
                track.enabled,
                supported_caption_language(&track.language),
                track.items.len(),
            ),
            TrackRef::Video(track) => (track.enabled, None, track.items.len()),
            TrackRef::Audio(track) => (track.enabled, None, track.items.len()),
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
            language,
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
        let track = InspectorSection::controls();
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let kind = self.kind;
        let address = self.address.clone();
        let enabled = switch_row(
            "Enabled",
            Some("Include this track in playback and export"),
            self.enabled,
            move |next| {
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
            },
        );
        track.add_wide_control(&enabled);

        if self.kind == ItemKind::Caption {
            let language = labeled_string_selector(
                "Language",
                self.language.as_deref().unwrap_or_default(),
                std::iter::once(StringChoice {
                    value: String::new(),
                    label: tr!("None").into_owned(),
                })
                .chain(caption_languages().iter().map(|language| StringChoice {
                    value: language.clone(),
                    label: language.clone(),
                }))
                .collect(),
                {
                    let project = context.project.clone();
                    let player_state = context.player_state.clone();
                    let address = self.address.clone();
                    move |language| {
                        let language = (!language.is_empty()).then_some(language);
                        let mut project = project.borrow_mut();
                        let Some(TrackMut::Caption(track)) = project.track_mut(&address) else {
                            return;
                        };
                        if track.language == language {
                            return;
                        }
                        track.language = language;
                        shrimply_project::project::commit_edit(
                            &project,
                            "set-caption-track-language",
                        );
                        drop(project);
                        player_state::refresh_project(
                            &player_state,
                            ProjectChange {
                                captions: true,
                                inspector: true,
                                ..ProjectChange::default()
                            },
                        );
                    }
                },
            );
            track.add_wide_control(language.widget());
        }

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
                    key: "track",
                    label: "Track",
                    icon: "sliders-horizontal-symbolic",
                    items: vec![flat(track.into_widget())],
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
