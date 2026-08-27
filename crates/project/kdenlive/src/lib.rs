mod ass;
mod effects;
mod math;
mod xml;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};

use glam::Vec2;
use serde_json::Value;
use shrimply_core::timeline_value::TimelineValue;
use shrimply_math_core::Fraction;
use shrimply_project::{
    AudioItem, AudioSource, AudioSpeedMethod, AudioTrack, CanvasSize, CaptionTrack, FoldedSequence,
    LayerVisibility, LayeredImageItem, PROJECT_FORMAT_VERSION, PreviewGuides, Project,
    SequenceReference, Time, VideoItem, VideoItemContent, VisualTrack,
};
use uuid::Uuid;

use xml::Element;

pub struct ImportResult {
    pub project: Value,
    pub warnings: Vec<String>,
}

pub fn from_file(path: impl AsRef<Path>) -> Result<ImportResult, Box<dyn Error + Send + Sync>> {
    let path = path.as_ref();
    let root = xml::parse(path)?;
    if root.name != "mlt" {
        return Err(invalid("not an MLT/Kdenlive document"));
    }

    let profile = root
        .children_named("profile")
        .next()
        .ok_or_else(|| invalid("missing MLT profile"))?;
    let fps = Fraction::new(
        positive_u64(profile.attribute("frame_rate_num"), "frame_rate_num")?,
        positive_u64(profile.attribute("frame_rate_den"), "frame_rate_den")?,
    );
    let canvas_size = CanvasSize {
        width: positive_u32(profile.attribute("width"), "profile width")?,
        height: positive_u32(profile.attribute("height"), "profile height")?,
    };
    let root_dir = root
        .attribute("root")
        .map(PathBuf::from)
        .or_else(|| path.parent().map(Path::to_path_buf))
        .unwrap_or_default();

    let index = root
        .children
        .iter()
        .filter_map(|node| node.attribute("id").map(|id| (id, node)))
        .collect::<HashMap<_, _>>();
    let main_bin = root
        .children_named("playlist")
        .find(|node| node.attribute("id") == Some("main_bin"))
        .ok_or_else(|| invalid("missing Kdenlive main bin"))?;
    let active_uuid = parse_uuid(
        main_bin
            .property("kdenlive:docproperties.activetimeline")
            .ok_or_else(|| invalid("missing active Kdenlive sequence"))?,
    )?;
    let active = sequence_by_uuid(&root, active_uuid)
        .ok_or_else(|| invalid("active Kdenlive sequence does not exist"))?;

    let mut converter = Converter {
        root_dir,
        fps,
        canvas_size,
        index,
        warnings: BTreeSet::new(),
    };
    let mut main = converter.convert_sequence(active)?;

    let mut reachable = HashSet::new();
    converter.collect_reachable(active, &mut reachable);
    reachable.remove(&active_uuid);
    let mut folded_sequences = Vec::with_capacity(reachable.len());
    for sequence_id in reachable {
        let tractor = sequence_by_uuid(&root, sequence_id)
            .ok_or_else(|| invalid("referenced Kdenlive sequence does not exist"))?;
        let sequence = converter.convert_sequence(tractor)?;
        folded_sequences.push(FoldedSequence {
            id: sequence_id,
            video_tracks: sequence.video_tracks,
            audio_tracks: sequence.audio_tracks,
        });
    }

    main.caption_tracks = converter.caption_tracks(active)?;
    let project = Project {
        format_version: PROJECT_FORMAT_VERSION,
        name: path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Imported Kdenlive project")
            .to_owned(),
        fps,
        canvas_size,
        caption_tracks: main.caption_tracks,
        video_tracks: main.video_tracks,
        audio_tracks: main.audio_tracks,
        folded_sequences,
        expanded_sequence_paths: Vec::new(),
        cursor_position: None,
        timeline_zoom: None,
        preview_guides: Box::<PreviewGuides>::default(),
    };

    Ok(ImportResult {
        project: serde_json::to_value(project)?,
        warnings: converter.warnings.into_iter().collect(),
    })
}

struct ConvertedSequence {
    video_tracks: Vec<VisualTrack>,
    audio_tracks: Vec<AudioTrack>,
    caption_tracks: Vec<CaptionTrack>,
}

struct Converter<'a> {
    root_dir: PathBuf,
    fps: Fraction,
    canvas_size: CanvasSize,
    index: HashMap<&'a str, &'a Element>,
    warnings: BTreeSet<String>,
}

impl<'a> Converter<'a> {
    fn convert_sequence(
        &mut self,
        tractor: &Element,
    ) -> Result<ConvertedSequence, Box<dyn Error + Send + Sync>> {
        let mut video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();

        for (position, outer_track) in tractor.children_named("track").enumerate() {
            let Some(target) = outer_track
                .attribute("producer")
                .and_then(|id| self.index.get(id).copied())
            else {
                continue;
            };
            if position == 0 || target.property("kdenlive:playlistid") == Some("black_track") {
                continue;
            }
            if target.property("kdenlive:playlistid") == Some("timeline_preview") {
                continue;
            }

            let hidden = outer_track.attribute("hide").unwrap_or_default();
            let is_audio = target.property("kdenlive:audio_track") == Some("1");
            let lanes = target
                .children_named("track")
                .filter_map(|track| track.attribute("producer"))
                .filter_map(|id| self.index.get(id).copied())
                .collect::<Vec<_>>();
            if lanes.is_empty() {
                continue;
            }

            let nonempty = lanes
                .iter()
                .filter(|lane| lane.children.iter().any(|node| node.name == "entry"))
                .count();
            if nonempty > 1 {
                self.warnings.insert(
                    "A Kdenlive track used both playlist lanes; its second lane was imported as an adjacent track."
                        .to_owned(),
                );
            }

            let lanes_to_import = if nonempty == 0 { 1 } else { lanes.len() };
            for lane in lanes.into_iter().take(lanes_to_import) {
                if nonempty != 0 && !lane.children.iter().any(|node| node.name == "entry") {
                    continue;
                }
                if is_audio {
                    audio_tracks.push(AudioTrack {
                        enabled: hidden != "audio" && hidden != "both",
                        items: self.audio_items(lane)?,
                        ..AudioTrack::default()
                    });
                } else {
                    video_tracks.push(VisualTrack {
                        enabled: hidden != "video" && hidden != "both",
                        items: self.video_items(lane)?,
                        ..VisualTrack::default()
                    });
                }
            }
        }

        Ok(ConvertedSequence {
            video_tracks,
            audio_tracks,
            caption_tracks: Vec::new(),
        })
    }

    fn video_items(
        &mut self,
        playlist: &Element,
    ) -> Result<Vec<VideoItem>, Box<dyn Error + Send + Sync>> {
        let mut cursor = 0_i64;
        let mut items = Vec::new();
        for node in &playlist.children {
            match node.name.as_str() {
                "blank" => cursor += element_duration(node, self.fps)?,
                "entry" => {
                    let duration = element_duration(node, self.fps)?;
                    let producer = self.entry_producer(node)?;
                    items.push(self.video_item(node, producer, cursor, duration)?);
                    cursor += duration;
                }
                _ => {}
            }
        }
        Ok(items)
    }

    fn audio_items(
        &mut self,
        playlist: &Element,
    ) -> Result<Vec<AudioItem>, Box<dyn Error + Send + Sync>> {
        let mut cursor = 0_i64;
        let mut items = Vec::new();
        for node in &playlist.children {
            match node.name.as_str() {
                "blank" => cursor += element_duration(node, self.fps)?,
                "entry" => {
                    let duration = element_duration(node, self.fps)?;
                    let producer = self.entry_producer(node)?;
                    items.push(self.audio_item(node, producer, cursor, duration)?);
                    cursor += duration;
                }
                _ => {}
            }
        }
        Ok(items)
    }

    fn video_item(
        &mut self,
        entry: &Element,
        producer: &Element,
        start_frame: i64,
        duration_frames: i64,
    ) -> Result<VideoItem, Box<dyn Error + Send + Sync>> {
        let source = self.source(producer)?;
        let start = frame_time(start_frame, self.fps);
        let end = frame_time(start_frame + duration_frames, self.fps);
        let mut item = VideoItem::background_item(self.canvas_size, start, end);
        item.time_offset = source_time(entry_in(entry, self.fps)?, source.speed, self.fps);
        item.source_duration = source
            .source_duration
            .unwrap_or_else(|| source_time(duration_frames, source.speed, self.fps));
        item.playback_speed = source.speed;
        item.track_id = source.video_track_id;
        item.file = source.path.into();
        item.source_width = source.width;
        item.source_height = source.height;
        if matches!(&source.visual, VideoItemContent::LayeredImage(_)) {
            item.sample_method =
                TimelineValue::new_const(shrimply_core::VideoSampleMethod::Nearest);
        }
        item.content = source.visual;
        item.transform = shrimply_project::Transform::natural_size(
            self.canvas_size,
            source.width,
            source.height,
        );
        item.transform.rotation_degrees = TimelineValue::new_const(source.rotation_degrees);
        let source_size = Vec2::new(source.width.max(1) as f32, source.height.max(1) as f32);
        let oriented_size = if ((source.rotation_degrees / 90.0).round() as i32).rem_euclid(2) != 0
        {
            Vec2::new(source_size.y, source_size.x)
        } else {
            source_size
        };
        item.transform.scale = TimelineValue::new_const(Vec2::splat(math::fit_scale(
            Vec2::new(
                self.canvas_size.width.max(1) as f32,
                self.canvas_size.height.max(1) as f32,
            ),
            oriented_size,
        )));
        item.default_transform = Some(item.transform.clone());
        self.apply_visual_effects(entry, &mut item)?;
        Ok(item)
    }

    fn audio_item(
        &mut self,
        entry: &Element,
        producer: &Element,
        start_frame: i64,
        duration_frames: i64,
    ) -> Result<AudioItem, Box<dyn Error + Send + Sync>> {
        let source = self.source(producer)?;
        let item = AudioItem::builder(
            frame_time(start_frame, self.fps),
            frame_time(start_frame + duration_frames, self.fps),
        )
        .time_offset(source_time(
            entry_in(entry, self.fps)?,
            source.speed,
            self.fps,
        ))
        .source_duration(
            source
                .source_duration
                .unwrap_or_else(|| source_time(duration_frames, source.speed, self.fps)),
        )
        .playback_speed(source.speed)
        .speed_method(if source.pitch_preserved {
            AudioSpeedMethod::PreservePitch
        } else {
            AudioSpeedMethod::Naive
        })
        .track_id(source.audio_track_id)
        .file(source.path)
        .source(source.audio)
        .build();
        self.apply_audio_effects(entry, item)
    }

    fn source(&mut self, producer: &Element) -> Result<Source, Box<dyn Error + Send + Sync>> {
        if producer.name == "tractor" {
            let sequence_id = parse_uuid(
                producer
                    .property("kdenlive:uuid")
                    .ok_or_else(|| invalid("nested sequence has no UUID"))?,
            )?;
            let reference = SequenceReference {
                sequence_id,
                instance_id: Uuid::new_v4(),
            };
            return Ok(Source {
                path: PathBuf::new(),
                visual: VideoItemContent::FoldedSequence(reference),
                audio: AudioSource::FoldedSequence(reference),
                width: self.canvas_size.width,
                height: self.canvas_size.height,
                speed: Fraction::from(1_u64),
                pitch_preserved: true,
                video_track_id: 0,
                audio_track_id: 0,
                source_duration: Some(frame_time(sequence_duration(producer, self.fps)?, self.fps)),
                rotation_degrees: 0.0,
            });
        }

        let service = producer.property("mlt_service").unwrap_or_default();
        if service == "color" {
            return Ok(Source {
                path: PathBuf::new(),
                visual: VideoItemContent::Background(Box::default()),
                audio: AudioSource::Media,
                width: self.canvas_size.width,
                height: self.canvas_size.height,
                speed: Fraction::from(1_u64),
                pitch_preserved: true,
                video_track_id: 0,
                audio_track_id: 0,
                source_duration: None,
                rotation_degrees: 0.0,
            });
        }

        let resource = producer
            .property("kdenlive:originalurl")
            .or_else(|| producer.property("warp_resource"))
            .or_else(|| producer.property("resource"))
            .ok_or_else(|| invalid("media producer has no resource"))?;
        let path = resolve_path(&self.root_dir, resource);
        let speed = if service == "timewarp" {
            parse_fraction(producer.property("warp_speed").unwrap_or("1"))?
        } else {
            Fraction::from(1_u64)
        };
        let pitch_preserved = producer.property("warp_pitch") == Some("1");
        let source_duration = if let Some(length) = producer.property("length") {
            Some(source_time(
                math::parse_frame(length, self.fps).map_err(invalid)?,
                speed,
                self.fps,
            ))
        } else if let Some(out) = producer.attribute("out") {
            Some(source_time(
                math::parse_frame(out, self.fps).map_err(invalid)? + 1,
                speed,
                self.fps,
            ))
        } else {
            None
        };
        let video_index = producer.property("video_index").unwrap_or("0");
        let rotation_degrees = if producer.property("autorotate") == Some("0")
            || producer.property("disable_exif") == Some("1")
        {
            0.0
        } else {
            producer
                .property(&format!("meta.media.{video_index}.codec.rotate"))
                .and_then(|value| value.parse().ok())
                .unwrap_or(0.0)
        };
        let width = property_u32(producer, &["meta.media.width", "meta.media.0.codec.width"])
            .unwrap_or(self.canvas_size.width);
        let height = property_u32(
            producer,
            &["meta.media.height", "meta.media.0.codec.height"],
        )
        .unwrap_or(self.canvas_size.height);
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let (width, height) = if extension == "pdf" {
            let bytes = std::fs::read(&path).map_err(|error| invalid(error.to_string()))?;
            let page = shrimply_pdf::page_sizes(bytes)
                .map_err(invalid)?
                .into_iter()
                .next()
                .expect("PDF inspection requires at least one page");
            (page.width, page.height)
        } else {
            (width, height)
        };
        let visual = match extension.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" => VideoItemContent::Image,
            "gif" => VideoItemContent::Gif,
            "svg" => VideoItemContent::Svg,
            "pdf" => VideoItemContent::Pdf(Box::default()),
            "kra" | "psd" => {
                let image = shrimply_layered_image::load(&path)?;
                let layers = image
                    .layers
                    .iter()
                    .map(|layer| LayerVisibility {
                        id: Uuid::new_v4(),
                        path: layer.path.clone(),
                        visibility: None,
                    })
                    .collect();
                VideoItemContent::LayeredImage(Box::new(LayeredImageItem { layers }))
            }
            _ => VideoItemContent::Media,
        };
        Ok(Source {
            path,
            visual,
            audio: AudioSource::Media,
            width,
            height,
            speed,
            pitch_preserved,
            video_track_id: producer
                .property("vstream")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            audio_track_id: producer
                .property("astream")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            source_duration,
            rotation_degrees,
        })
    }

    fn entry_producer(&self, entry: &Element) -> Result<&'a Element, Box<dyn Error + Send + Sync>> {
        entry
            .attribute("producer")
            .and_then(|id| self.index.get(id).copied())
            .ok_or_else(|| invalid("timeline entry references a missing producer"))
    }

    fn collect_reachable(&self, tractor: &Element, sequences: &mut HashSet<Uuid>) {
        let Some(uuid) = tractor
            .property("kdenlive:uuid")
            .and_then(|value| parse_uuid(value).ok())
        else {
            return;
        };
        if !sequences.insert(uuid) {
            return;
        }
        for lane in tractor
            .children_named("track")
            .filter_map(|track| track.attribute("producer"))
            .filter_map(|id| self.index.get(id).copied())
            .flat_map(|track| track.children_named("track"))
            .filter_map(|track| track.attribute("producer"))
            .filter_map(|id| self.index.get(id).copied())
        {
            for nested in lane
                .children_named("entry")
                .filter_map(|entry| entry.attribute("producer"))
                .filter_map(|id| self.index.get(id).copied())
                .filter(|producer| producer.name == "tractor")
            {
                self.collect_reachable(nested, sequences);
            }
        }
    }

    fn caption_tracks(
        &mut self,
        tractor: &Element,
    ) -> Result<Vec<CaptionTrack>, Box<dyn Error + Send + Sync>> {
        let Some(list) = tractor
            .property("kdenlive:sequenceproperties.subtitlesList")
            .or_else(|| tractor.property("kdenlive:subtitlesList"))
        else {
            return Ok(Vec::new());
        };
        let disabled = tractor.property("kdenlive:sequenceproperties.hidesubtitle") == Some("1")
            || tractor.children_named("filter").any(|filter| {
                matches!(
                    filter.property("mlt_service"),
                    Some("avfilter.ass" | "avfilter.subtitles")
                ) && filter.property("disable") == Some("1")
            });
        let value: Value = serde_json::from_str(list)?;
        let files = value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.get("file").and_then(Value::as_str));
        let mut tracks = Vec::new();
        for file in files {
            tracks.push(CaptionTrack {
                enabled: !disabled,
                items: ass::read(&resolve_path(&self.root_dir, file)).map_err(invalid)?,
                ..CaptionTrack::default()
            });
            self.warnings.insert(
                "ASS subtitle fonts, outlines, and positioning were approximated with Shrimply caption styling."
                    .to_owned(),
            );
        }
        Ok(tracks)
    }
}

struct Source {
    path: PathBuf,
    visual: VideoItemContent,
    audio: AudioSource,
    width: u32,
    height: u32,
    speed: Fraction,
    pitch_preserved: bool,
    video_track_id: u32,
    audio_track_id: u32,
    source_duration: Option<Time>,
    rotation_degrees: f32,
}

fn sequence_by_uuid(root: &Element, uuid: Uuid) -> Option<&Element> {
    root.children_named("tractor").find(|tractor| {
        tractor
            .property("kdenlive:uuid")
            .and_then(|value| parse_uuid(value).ok())
            == Some(uuid)
    })
}

fn sequence_duration(
    tractor: &Element,
    fps: Fraction,
) -> Result<i64, Box<dyn Error + Send + Sync>> {
    if let Some(duration) = tractor.property("kdenlive:maxduration") {
        return math::parse_frame(duration, fps).map_err(invalid);
    }
    element_duration(tractor, fps)
}

fn element_duration(node: &Element, fps: Fraction) -> Result<i64, Box<dyn Error + Send + Sync>> {
    if let Some(length) = node.attribute("length") {
        return math::parse_frame(length, fps).map_err(invalid);
    }
    let start = math::parse_frame(node.attribute("in").unwrap_or("0"), fps).map_err(invalid)?;
    let end_value = node
        .attribute("out")
        .ok_or_else(|| invalid("timeline element has no out point"))?;
    let end = math::parse_frame(end_value, fps).map_err(invalid)?;
    Ok(end - start + 1)
}

fn entry_in(entry: &Element, fps: Fraction) -> Result<i64, Box<dyn Error + Send + Sync>> {
    math::parse_frame(entry.attribute("in").unwrap_or("0"), fps).map_err(invalid)
}

fn frame_time(frame: i64, fps: Fraction) -> Time {
    source_time(frame, Fraction::from(1_u64), fps)
}

fn source_time(frame: i64, speed: Fraction, fps: Fraction) -> Time {
    let magnitude = Fraction::from(frame.unsigned_abs()) * speed / fps;
    Time {
        seconds: if frame < 0 { -magnitude } else { magnitude },
    }
}

fn parse_fraction(value: &str) -> Result<Fraction, Box<dyn Error + Send + Sync>> {
    let value = value.trim();
    if let Some((whole, fractional)) = value.split_once('.') {
        let denominator = 10_u64.pow(fractional.len() as u32);
        let numerator = whole.parse::<u64>()? * denominator + fractional.parse::<u64>()?;
        Ok(Fraction::new(numerator, denominator))
    } else if let Some((numerator, denominator)) = value.split_once('/') {
        Ok(Fraction::new(
            numerator.parse::<u64>()?,
            denominator.parse::<u64>()?,
        ))
    } else {
        Ok(Fraction::from(value.parse::<u64>()?))
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, Box<dyn Error + Send + Sync>> {
    Ok(Uuid::parse_str(value.trim_matches(['{', '}']))?)
}

fn resolve_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn property_u32(element: &Element, names: &[&str]) -> Option<u32> {
    names
        .iter()
        .find_map(|name| element.property(name).and_then(|value| value.parse().ok()))
}

fn positive_u64(value: Option<&str>, name: &str) -> Result<u64, Box<dyn Error + Send + Sync>> {
    let value = value
        .ok_or_else(|| invalid(format!("missing {name}")))?
        .parse::<u64>()?;
    if value == 0 {
        return Err(invalid(format!("{name} must be positive")));
    }
    Ok(value)
}

fn positive_u32(value: Option<&str>, name: &str) -> Result<u32, Box<dyn Error + Send + Sync>> {
    let value = positive_u64(value, name)?;
    Ok(value.try_into()?)
}

fn invalid(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    Box::new(io::Error::new(io::ErrorKind::InvalidData, message.into()))
}
