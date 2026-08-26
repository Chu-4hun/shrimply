use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use adw::prelude::*;
use ffmpeg::{codec, format, media};
use ffmpeg_next as ffmpeg;
use glam::UVec2;
use hashbrown::HashMap;

use shrimply_project::project::Time;

use super::item::{InspectorListItem, flat};

type SourceRateCache = Mutex<HashMap<(PathBuf, SourceMetadata), Option<String>>>;

static SOURCE_RATE_CACHE: OnceLock<SourceRateCache> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum SourceMetadata {
    None,
    Video(u32),
    Audio(u32),
}

pub(super) fn item(
    leading: Vec<gtk::Widget>,
    kind: &str,
    natural_duration: Option<Time>,
    timeline_duration: Time,
    dimensions: Option<UVec2>,
    file: Option<&Path>,
    metadata: SourceMetadata,
) -> InspectorListItem {
    let group = adw::PreferencesGroup::new();
    for leading in leading {
        group.add(&leading);
    }
    group.add(
        &adw::ActionRow::builder()
            .title("Type")
            .subtitle(kind)
            .build(),
    );
    if let Some(duration) = natural_duration {
        group.add(
            &adw::ActionRow::builder()
                .title("Natural Duration")
                .subtitle(crate::time_format::playback_time(duration))
                .build(),
        );
    }
    group.add(
        &adw::ActionRow::builder()
            .title("Timeline Duration")
            .subtitle(crate::time_format::playback_time(timeline_duration))
            .build(),
    );
    if let Some(size) = dimensions.filter(|size| size.x > 0 && size.y > 0) {
        group.add(
            &adw::ActionRow::builder()
                .title("Dimensions")
                .subtitle(format!("{} × {}", size.x, size.y))
                .build(),
        );
    }
    if let Some(file) = file.filter(|file| !file.as_os_str().is_empty()) {
        if let Some(rate) = source_rate(file, metadata) {
            let title = match metadata {
                SourceMetadata::Video(_) => "Frame Rate",
                SourceMetadata::Audio(_) => "Sample Rate",
                SourceMetadata::None => unreachable!(),
            };
            group.add(
                &adw::ActionRow::builder()
                    .title(title)
                    .subtitle(rate)
                    .build(),
            );
        }
        let row = adw::ActionRow::builder()
            .title("File Location")
            .subtitle(file.to_string_lossy())
            .build();
        let button = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Show in folder")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let path = file.to_path_buf();
        button.connect_clicked(move |button| {
            if let Err(error) =
                crate::desktop_open::show_path_in_folder(button.upcast_ref(), path.clone())
            {
                let dialog = adw::AlertDialog::new(Some("Could not show media file"), Some(&error));
                dialog.add_response("close", "Close");
                dialog.present(Some(button));
            }
        });
        row.add_suffix(&button);
        row.set_activatable_widget(Some(&button));
        group.add(&row);
    }

    flat(group)
}

fn source_rate(file: &Path, metadata: SourceMetadata) -> Option<String> {
    if metadata == SourceMetadata::None {
        return None;
    }
    let key = (file.to_path_buf(), metadata);
    if let Some(rate) = SOURCE_RATE_CACHE
        .get_or_init(Mutex::default)
        .lock()
        .ok()
        .and_then(|cache| cache.get(&key).cloned())
    {
        return rate;
    }
    let rate = inspect_source_rate(file, metadata);
    if let Ok(mut cache) = SOURCE_RATE_CACHE.get_or_init(Mutex::default).lock() {
        cache.insert(key.clone(), rate.clone());
    }
    rate
}

fn inspect_source_rate(file: &Path, metadata: SourceMetadata) -> Option<String> {
    let input = format::input(file).ok()?;
    match metadata {
        SourceMetadata::None => None,
        SourceMetadata::Video(index) => {
            let stream = input
                .streams()
                .filter(|stream| stream.parameters().medium() == media::Type::Video)
                .nth(index as usize)?;
            let mut rate = stream.avg_frame_rate();
            if rate.numerator() <= 0 || rate.denominator() <= 0 {
                rate = stream.rate();
            }
            let rate = f64::from(rate);
            (rate.is_finite() && rate > 0.0).then(|| format!("{rate:.3} FPS"))
        }
        SourceMetadata::Audio(index) => {
            let stream = input
                .streams()
                .filter(|stream| stream.parameters().medium() == media::Type::Audio)
                .nth(index as usize)?;
            let rate = codec::context::Context::from_parameters(stream.parameters())
                .ok()?
                .decoder()
                .audio()
                .ok()?
                .rate();
            (rate > 0).then(|| format!("{rate} Hz"))
        }
    }
}

pub(super) fn video_stream_count(file: &Path) -> usize {
    format::input(file).map_or(0, |input| {
        input
            .streams()
            .filter(|stream| stream.parameters().medium() == media::Type::Video)
            .count()
    })
}

pub(super) fn audio_stream_count(file: &Path) -> usize {
    format::input(file).map_or(0, |input| {
        input
            .streams()
            .filter(|stream| stream.parameters().medium() == media::Type::Audio)
            .count()
    })
}
