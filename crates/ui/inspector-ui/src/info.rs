use std::{path::PathBuf, thread};

use adw::prelude::*;
use ffmpeg::{format, media};
use ffmpeg_next as ffmpeg;
use glam::UVec2;
use gtk::glib;
use shrimply_media_info::{ExactRatio, ExactTime, FileInfo, StreamInfo};
use shrimply_project::project::{Asset, Time};
use shrimply_ui_foundation::tr;

use super::{
    InspectorContext,
    item::{InspectorListItem, flat},
};

const ARTWORK_HEIGHT: i32 = 220;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum SourceMetadata {
    None,
    Video(u32),
    Audio(u32),
}

pub(super) struct ItemInfo {
    pub(super) leading: Vec<gtk::Widget>,
    pub(super) kind: &'static str,
    pub(super) natural_duration: Option<Time>,
    pub(super) start: Time,
    pub(super) end: Time,
    pub(super) source_offset: Option<Time>,
    pub(super) dimensions: Option<UVec2>,
    pub(super) file: Option<Asset>,
    pub(super) source_metadata: SourceMetadata,
}

pub(super) fn item(context: &InspectorContext, info: ItemInfo) -> InspectorListItem {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let summary = adw::PreferencesGroup::new();
    for leading in info.leading {
        summary.add(&leading);
    }
    add_row(&summary, "Type", info.kind);
    if let Some(address) = &context.selected_item {
        add_row(&summary, "Item ID", &address.item_id().to_string());
        add_row(&summary, "Track ID", &address.track_id().to_string());
        if !address.sequence_path().is_empty() {
            add_row(
                &summary,
                "Sequence Path",
                &address
                    .sequence_path()
                    .iter()
                    .map(uuid::Uuid::to_string)
                    .collect::<Vec<_>>()
                    .join(" / "),
            );
        }
        if let Some((start, end)) = context.project.borrow().projected_item_times(address) {
            add_row(
                &summary,
                "Timeline Start",
                &crate::time_format::playback_time(start),
            );
            add_row(
                &summary,
                "Timeline End",
                &crate::time_format::playback_time(end),
            );
        }
    }
    add_row(
        &summary,
        "Local Start",
        &crate::time_format::playback_time(info.start),
    );
    add_row(
        &summary,
        "Local End",
        &crate::time_format::playback_time(info.end),
    );
    if let Some(duration) = info.natural_duration {
        add_row(
            &summary,
            "Natural Duration",
            &crate::time_format::playback_time(duration),
        );
    }
    add_row(
        &summary,
        "Timeline Duration",
        &crate::time_format::playback_time(info.end.saturating_sub(info.start)),
    );
    if let Some(offset) = info.source_offset {
        add_row(
            &summary,
            "Source Offset",
            &crate::time_format::playback_time(offset),
        );
    }
    if let Some(size) = info.dimensions.filter(|size| size.x > 0 && size.y > 0) {
        add_row(&summary, "Dimensions", &format!("{} × {}", size.x, size.y));
    }
    content.append(&summary);

    if let Some(file) = info.file.filter(|file| !file.path().as_os_str().is_empty()) {
        content.append(&file_row(file.path().to_path_buf()));
        let metadata = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let loading = adw::PreferencesGroup::new();
        let loading_row = adw::ActionRow::builder()
            .title(tr!("File Metadata").as_ref())
            .subtitle(tr!("Loading…").as_ref())
            .build();
        loading_row.add_suffix(&adw::Spinner::new());
        loading.add(&loading_row);
        metadata.append(&loading);
        content.append(&metadata);

        let revision = file.snapshot().ok().map(|snapshot| snapshot.revision());
        let path = file.path().to_path_buf();
        let (sender, receiver) = async_channel::bounded(1);
        thread::spawn(move || {
            let _ = sender.send_blocking(shrimply_media_info::inspect(&path, revision));
        });
        let weak_metadata = metadata.downgrade();
        glib::spawn_future_local(async move {
            let Ok(result) = receiver.recv().await else {
                return;
            };
            let Some(metadata) = weak_metadata.upgrade() else {
                return;
            };
            while let Some(child) = metadata.first_child() {
                metadata.remove(&child);
            }
            match result {
                Ok(file) => render_file_info(&metadata, &file, info.source_metadata),
                Err(error) => metadata.append(&diagnostic_group(&[error])),
            }
        });
    }

    flat(content)
}

fn file_row(path: PathBuf) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title(tr!("File Location").as_ref())
        .subtitle(path.to_string_lossy())
        .build();
    let button = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text(tr!("Show in folder").as_ref())
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    button.connect_clicked(move |button| {
        if let Err(error) =
            crate::desktop_open::show_path_in_folder(button.upcast_ref(), path.clone())
        {
            let dialog = adw::AlertDialog::new(Some("Could not show media file"), Some(&error));
            dialog.add_response("close", tr!("Close").as_ref());
            dialog.present(Some(button));
        }
    });
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    group.add(&row);
    group
}

fn render_file_info(container: &gtk::Box, info: &FileInfo, selected: SourceMetadata) {
    if let Some(artwork) = primary_artwork(info)
        && let Ok(texture) =
            gtk::gdk::Texture::from_bytes(&glib::Bytes::from_owned(artwork.data.clone()))
    {
        let group = adw::PreferencesGroup::builder()
            .title(tr!("Artwork").as_ref())
            .build();
        let picture = gtk::Picture::for_paintable(&texture);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_can_shrink(true);
        picture.set_height_request(ARTWORK_HEIGHT);
        group.add(&picture);
        container.append(&group);
    }

    let file = adw::PreferencesGroup::builder()
        .title(tr!("File").as_ref())
        .build();
    if let Some(name) = &info.file.file_name {
        add_row(&file, "Name", name);
    }
    if let Some(extension) = &info.file.extension {
        add_row(&file, "Extension", extension);
    }
    add_row(&file, "Size", &format_bytes(info.file.byte_size));
    if let Some(path) = &info.file.canonical_path
        && path != &info.file.path
    {
        add_row(&file, "Canonical Path", path);
    }
    if let Some(created) = info.file.created_unix_seconds.and_then(format_date) {
        add_row(&file, "Created", &created);
    }
    if let Some(modified) = info.file.modified_unix_seconds.and_then(format_date) {
        add_row(&file, "Modified", &modified);
    }
    container.append(&file);

    if let Some(format) = &info.container {
        let group = adw::PreferencesGroup::builder()
            .title(tr!("Media").as_ref())
            .build();
        add_row(&group, "Format", &format.description);
        add_row(&group, "Format IDs", &format.format_names.join(", "));
        if !format.extensions.is_empty() {
            add_row(&group, "Known Extensions", &format.extensions.join(", "));
        }
        if !format.mime_types.is_empty() {
            add_row(&group, "MIME Type", &format.mime_types.join(", "));
        }
        if let Some(start_time) = &format.start_time {
            add_row(&group, "File Start Time", &format_time(start_time));
        }
        if let Some(duration) = &format.duration {
            add_row(&group, "File Duration", &format_time(duration));
        }
        if let Some(bit_rate) = format.bit_rate {
            add_row(&group, "Bit Rate", &format_rate(bit_rate));
        }
        add_row(&group, "Streams", &info.streams.len().to_string());
        container.append(&group);
    }

    render_common_tags(container, info);
    render_artwork_inventory(container, info);
    let selected_index = selected_stream_index(&info.streams, selected);
    for stream in &info.streams {
        render_stream(container, stream, selected_index == Some(stream.index));
    }
    if !info.chapters.is_empty() {
        let group = adw::PreferencesGroup::builder()
            .title(tr!("Chapters").as_ref())
            .build();
        for (index, chapter) in info.chapters.iter().enumerate() {
            add_row(
                &group,
                &format!("Chapter {}", index + 1),
                &format!(
                    "{} – {}",
                    format_time(&chapter.start),
                    format_time(&chapter.end)
                ),
            );
            add_tags(&group, &chapter.tags);
        }
        container.append(&group);
    }
    if let Some(image) = &info.image {
        let group = adw::PreferencesGroup::builder()
            .title(tr!("Image").as_ref())
            .build();
        add_row(&group, "Image Format", &image.image_type);
        add_row(
            &group,
            "Pixel Dimensions",
            &format!("{} × {}", image.width, image.height),
        );
        for field in &image.exif {
            add_row(
                &group,
                &format!("{} / {}", field.ifd, field.tag),
                &field.value,
            );
        }
        container.append(&group);
    }
    if let Some(format) = &info.container
        && !format.tags.is_empty()
    {
        let group = adw::PreferencesGroup::builder()
            .title(tr!("Container Tags").as_ref())
            .build();
        add_tags(&group, &format.tags);
        container.append(&group);
    }
    if !info.tags.is_empty() {
        let group = adw::PreferencesGroup::builder()
            .title(tr!("Raw Audio Tags").as_ref())
            .build();
        for tag in &info.tags {
            add_row(&group, &format!("{} · {}", tag.key, tag.source), &tag.value);
        }
        container.append(&group);
    }
    if !info.diagnostics.is_empty() {
        container.append(&diagnostic_group(&info.diagnostics));
    }
}

fn render_common_tags(container: &gtk::Box, info: &FileInfo) {
    let tags = &info.common_tags;
    let has_tags = tags.title.is_some()
        || tags.artist.is_some()
        || tags.album.is_some()
        || tags.album_artist.is_some()
        || tags.genre.is_some()
        || tags.year.is_some()
        || tags.track.is_some()
        || tags.disc.is_some();
    if !has_tags {
        return;
    }
    let group = adw::PreferencesGroup::builder()
        .title(tr!("Tags").as_ref())
        .build();
    for (title, value) in [
        ("Title", tags.title.as_deref()),
        ("Artist", tags.artist.as_deref()),
        ("Album", tags.album.as_deref()),
        ("Album Artist", tags.album_artist.as_deref()),
        ("Genre", tags.genre.as_deref()),
    ] {
        if let Some(value) = value {
            add_row(&group, title, value);
        }
    }
    if let Some(year) = tags.year {
        add_row(&group, "Year", &year.to_string());
    }
    if let Some(track) = tags.track {
        add_row(&group, "Track", &number_of(track, tags.track_total));
    }
    if let Some(disc) = tags.disc {
        add_row(&group, "Disc", &number_of(disc, tags.disc_total));
    }
    container.append(&group);
}

fn render_artwork_inventory(container: &gtk::Box, info: &FileInfo) {
    if info.artwork.is_empty() {
        return;
    }
    let group = adw::PreferencesGroup::builder()
        .title(tr!("Embedded Artwork").as_ref())
        .build();
    for artwork in &info.artwork {
        let dimensions = artwork
            .width
            .zip(artwork.height)
            .map(|(width, height)| format!("{width} × {height}, "))
            .unwrap_or_default();
        let color_depth = artwork
            .color_depth
            .map(|depth| format!("{depth}-bit, "))
            .unwrap_or_default();
        let description = artwork
            .description
            .as_deref()
            .filter(|description| !description.is_empty())
            .map(|description| format!(" · {description}"))
            .unwrap_or_default();
        let mime = artwork.mime_type.as_deref().unwrap_or("unknown type");
        add_row(
            &group,
            &format!("{} {}", tr!("Artwork").as_ref(), artwork.index + 1),
            &format!(
                "{}{description} · {mime} · {dimensions}{color_depth}{}",
                artwork.picture_type,
                format_bytes(artwork.byte_size as u64)
            ),
        );
    }
    container.append(&group);
}

fn render_stream(container: &gtk::Box, stream: &StreamInfo, selected: bool) {
    let selection = if selected {
        format!(" · {}", tr!("Selected").as_ref())
    } else {
        String::new()
    };
    let group = adw::PreferencesGroup::builder()
        .title(format!(
            "{} {} ({}){selection}",
            tr!("Stream").as_ref(),
            stream.index + 1,
            stream.kind
        ))
        .build();
    add_row(&group, "Stream Index", &stream.index.to_string());
    add_row(&group, "Stream ID", &stream.id.to_string());
    add_row(&group, "Codec", &stream.codec);
    if let Some(description) = &stream.codec_description {
        add_row(&group, "Codec Description", description);
    }
    if let Some(profile) = &stream.profile {
        add_row(&group, "Profile", profile);
    }
    if let Some(bit_rate) = stream.bit_rate {
        add_row(&group, "Bit Rate", &format_rate(bit_rate));
    }
    if let Some(time_base) = &stream.time_base {
        add_row(&group, "Time Base", &format_ratio(time_base));
    }
    if let Some(start_time) = &stream.start_time {
        add_row(&group, "Start Time", &format_time(start_time));
    }
    if let Some(duration) = &stream.duration {
        add_row(&group, "Duration", &format_time(duration));
    }
    if let Some(frames) = stream.frame_count {
        add_row(&group, "Frames", &frames.to_string());
    }
    add_row(&group, "Disposition", &stream.disposition);
    if let Some(video) = &stream.video {
        add_row(
            &group,
            "Dimensions",
            &format!("{} × {}", video.width, video.height),
        );
        add_row(&group, "Pixel Format", &video.pixel_format);
        if let Some(rate) = &video.average_frame_rate {
            add_row(&group, "Frame Rate", &format!("{} FPS", format_ratio(rate)));
        }
        if let Some(rate) = &video.nominal_frame_rate {
            add_row(
                &group,
                "Nominal Frame Rate",
                &format!("{} FPS", format_ratio(rate)),
            );
        }
        if let Some(aspect) = &video.sample_aspect_ratio {
            add_row(&group, "Pixel Aspect Ratio", &format_ratio(aspect));
        }
        add_row(&group, "Color Range", &video.color_range);
        add_row(&group, "Color Space", &video.color_space);
        add_row(&group, "Color Primaries", &video.color_primaries);
        add_row(&group, "Color Transfer", &video.color_transfer);
        add_row(&group, "Chroma Location", &video.chroma_location);
        add_row(
            &group,
            "B-Frames",
            if video.has_b_frames { "Yes" } else { "No" },
        );
    }
    if let Some(audio) = &stream.audio {
        add_row(&group, "Sample Rate", &format!("{} Hz", audio.sample_rate));
        add_row(&group, "Channels", &audio.channels.to_string());
        add_row(&group, "Channel Layout", &audio.channel_layout);
        add_row(&group, "Sample Format", &audio.sample_format);
        add_row(&group, "Frame Size", &audio.frame_size.to_string());
    }
    add_tags(&group, &stream.tags);
    container.append(&group);
}

fn primary_artwork(info: &FileInfo) -> Option<&shrimply_media_info::Artwork> {
    info.artwork
        .iter()
        .find(|artwork| artwork.picture_type == "CoverFront")
        .or_else(|| info.artwork.first())
}

fn selected_stream_index(streams: &[StreamInfo], selected: SourceMetadata) -> Option<usize> {
    let (kind, ordinal) = match selected {
        SourceMetadata::None => return None,
        SourceMetadata::Video(index) => ("video", index),
        SourceMetadata::Audio(index) => ("audio", index),
    };
    streams
        .iter()
        .filter(|stream| stream.kind == kind)
        .nth(ordinal as usize)
        .map(|stream| stream.index)
}

fn diagnostic_group(diagnostics: &[String]) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr!("Diagnostics").as_ref())
        .build();
    for diagnostic in diagnostics {
        add_row(&group, "Metadata", diagnostic);
    }
    group
}

fn add_tags(group: &adw::PreferencesGroup, tags: &[shrimply_media_info::TagValue]) {
    for tag in tags {
        add_row(group, &tag.key, &tag.value);
    }
}

fn add_row(group: &adw::PreferencesGroup, title: &str, value: &str) {
    group.add(
        &adw::ActionRow::builder()
            .title(tr!(title).as_ref())
            .subtitle(value)
            .build(),
    );
}

fn number_of(value: u32, total: Option<u32>) -> String {
    total.map_or_else(|| value.to_string(), |total| format!("{value} of {total}"))
}

fn format_ratio(value: &ExactRatio) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn format_time(value: &ExactTime) -> String {
    format!("{} × {} s", value.value, format_ratio(&value.time_base))
}

fn format_rate(bits_per_second: i64) -> String {
    const KILOBIT: u64 = 1_000;
    const MEGABIT: u64 = KILOBIT * 1_000;
    const GIGABIT: u64 = MEGABIT * 1_000;
    let bits = bits_per_second.max(0) as u64;
    let (value, unit) = if bits >= GIGABIT {
        (GIGABIT, "Gb/s")
    } else if bits >= MEGABIT {
        (MEGABIT, "Mb/s")
    } else if bits >= KILOBIT {
        (KILOBIT, "kb/s")
    } else {
        return format!("{bits} b/s");
    };
    format!("{}.{} {unit}", bits / value, bits % value * 10 / value)
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    let (value, unit) = if bytes >= GIB {
        (GIB, "GiB")
    } else if bytes >= MIB {
        (MIB, "MiB")
    } else if bytes >= KIB {
        (KIB, "KiB")
    } else {
        return format!("{bytes} B");
    };
    format!("{}.{} {unit}", bytes / value, bytes % value * 10 / value)
}

fn format_date(seconds: i64) -> Option<String> {
    glib::DateTime::from_unix_local(seconds)
        .ok()
        .and_then(|date| date.format("%x %X").ok())
        .map(|date| date.to_string())
}

pub(super) fn video_stream_count(file: &std::path::Path) -> usize {
    format::input(file).map_or(0, |input| {
        input
            .streams()
            .filter(|stream| stream.parameters().medium() == media::Type::Video)
            .count()
    })
}

pub(super) fn audio_stream_count(file: &std::path::Path) -> usize {
    format::input(file).map_or(0, |input| {
        input
            .streams()
            .filter(|stream| stream.parameters().medium() == media::Type::Audio)
            .count()
    })
}
