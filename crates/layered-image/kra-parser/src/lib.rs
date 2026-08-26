use std::io::{BufRead, Cursor, Read};

use glam::{I64Vec2, IVec2, UVec2};
use half::f16;
use moxcms::{ColorProfile, Layout, TransformOptions};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use rayon::prelude::*;
use skia_safe::{AlphaType, Color, ColorType, FontMgr, ImageInfo, surfaces, svg};
use zip::ZipArchive;

const TILE_WIDTH: usize = 64;
const TILE_HEIGHT: usize = 64;
const MAX_CANVAS_PIXELS: usize = 268_435_456;

pub struct Document {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub groups: Vec<Group>,
    pub nodes: Vec<Node>,
}

pub struct Layer {
    pub name: String,
    pub parent: Option<u32>,
    pub rgba: Vec<u8>,
    pub visible: bool,
    pub opacity: u8,
    pub blend_mode: String,
}

pub struct Group {
    pub id: u32,
    pub name: String,
    pub parent: Option<u32>,
    pub visible: bool,
    pub opacity: u8,
}

#[derive(Clone, Copy)]
pub enum Node {
    Layer(usize),
    Group(u32),
}

struct LayerMetadata {
    name: String,
    filename: String,
    vector: bool,
    color_space: String,
    profile_name: Option<String>,
    position: IVec2,
    parent: Option<u32>,
    visible: bool,
    opacity: u8,
    blend_mode: String,
}

struct XmlDocument {
    width: u32,
    height: u32,
    image_name: String,
    profile_name: Option<String>,
    layers: Vec<LayerMetadata>,
    groups: Vec<Group>,
    nodes: Vec<Node>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Model {
    Rgb,
    Gray,
    Cmyk,
    Lab,
    Xyz,
    YCbCr,
    Alpha,
}

#[derive(Clone, Copy)]
enum Depth {
    U8,
    U16,
    F16,
    F32,
}

#[derive(Clone, Copy)]
struct PixelSpec {
    model: Model,
    depth: Depth,
    channels: usize,
    blue_first: bool,
}

impl PixelSpec {
    fn parse(name: &str) -> Result<Self, String> {
        let (model, depth, channels, blue_first) = match name {
            "RGBA" => (Model::Rgb, Depth::U8, 4, true),
            "RGBA16" => (Model::Rgb, Depth::U16, 4, true),
            "RGBAF16" | "RgbAF16" => (Model::Rgb, Depth::F16, 4, false),
            "RGBAF32" | "RgbAF32" => (Model::Rgb, Depth::F32, 4, false),
            "GRAYA" | "Grayscale + Alpha" => (Model::Gray, Depth::U8, 2, false),
            "GRAYAU16" | "GRAYA16" => (Model::Gray, Depth::U16, 2, false),
            "GRAYAF16" => (Model::Gray, Depth::F16, 2, false),
            "GRAYAF32" | "GrayF32" => (Model::Gray, Depth::F32, 2, false),
            "CMYK" => (Model::Cmyk, Depth::U8, 5, false),
            "CMYKAU16" | "CMYKA16" => (Model::Cmyk, Depth::U16, 5, false),
            "CMYKAF32" => (Model::Cmyk, Depth::F32, 5, false),
            "LABAU8" => (Model::Lab, Depth::U8, 4, false),
            "LABA" => (Model::Lab, Depth::U16, 4, false),
            "LABAF32" => (Model::Lab, Depth::F32, 4, false),
            "XYZA8" => (Model::Xyz, Depth::U8, 4, false),
            "XYZA16" => (Model::Xyz, Depth::U16, 4, false),
            "XYZAF16" | "XyzAF16" => (Model::Xyz, Depth::F16, 4, false),
            "XYZAF32" | "XyzAF32" => (Model::Xyz, Depth::F32, 4, false),
            "YCBCRA8" | "YCbCrA" => (Model::YCbCr, Depth::U8, 4, false),
            "YCBCRAU16" | "YCbCrAU16" => (Model::YCbCr, Depth::U16, 4, false),
            "YCBCRF32" => (Model::YCbCr, Depth::F32, 4, false),
            "ALPHA" => (Model::Alpha, Depth::U8, 1, false),
            "ALPHAU16" => (Model::Alpha, Depth::U16, 1, false),
            "ALPHAF16" => (Model::Alpha, Depth::F16, 1, false),
            "ALPHAF32" => (Model::Alpha, Depth::F32, 1, false),
            _ => return Err(format!("unsupported Krita color space {name}")),
        };
        Ok(Self {
            model,
            depth,
            channels,
            blue_first,
        })
    }

    fn bytes_per_channel(self) -> usize {
        match self.depth {
            Depth::U8 => 1,
            Depth::U16 | Depth::F16 => 2,
            Depth::F32 => 4,
        }
    }

    fn pixel_size(self) -> usize {
        self.channels * self.bytes_per_channel()
    }
}

pub fn parse(bytes: &[u8]) -> Result<Document, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let maindoc = read_entry(&mut archive, "maindoc.xml")?;
    let xml = parse_xml(&maindoc)?;
    let pixels = (xml.width as usize)
        .checked_mul(xml.height as usize)
        .filter(|pixels| *pixels <= MAX_CANVAS_PIXELS)
        .ok_or_else(|| format!("Krita canvas {}x{} is too large", xml.width, xml.height))?;
    let document_profile =
        read_optional_entry(&mut archive, &format!("{}/annotations/icc", xml.image_name))?;
    let mut layers = Vec::with_capacity(xml.layers.len());
    for metadata in xml.layers {
        let base = format!("{}/layers/{}", xml.image_name, metadata.filename);
        let rgba = if metadata.vector {
            let svg = read_entry(&mut archive, &format!("{base}.shapelayer/content.svg"))?;
            render_vector_layer(&svg, xml.width, xml.height, pixels)?
        } else {
            let spec = PixelSpec::parse(&metadata.color_space)?;
            let stream = read_entry(&mut archive, &base)?;
            let default_pixel = read_optional_entry(&mut archive, &format!("{base}.defaultpixel"))?
                .unwrap_or_else(|| vec![0; spec.pixel_size()]);
            if default_pixel.len() != spec.pixel_size() {
                return Err(format!(
                    "{} has a {} byte default pixel, expected {}",
                    metadata.name,
                    default_pixel.len(),
                    spec.pixel_size(),
                ));
            }
            let native = decode_tiles(
                &stream,
                UVec2::new(xml.width, xml.height),
                metadata.position,
                spec,
                &default_pixel,
                pixels,
            )?;
            let layer_profile = read_optional_entry(&mut archive, &format!("{base}.icc"))?;
            let profile = layer_profile.as_deref().or(document_profile.as_deref());
            convert_to_rgba8(
                &native,
                spec,
                profile,
                metadata
                    .profile_name
                    .as_deref()
                    .or(xml.profile_name.as_deref()),
            )?
        };
        layers.push(Layer {
            name: metadata.name,
            parent: metadata.parent,
            rgba,
            visible: metadata.visible,
            opacity: metadata.opacity,
            blend_mode: metadata.blend_mode,
        });
    }
    Ok(Document {
        width: xml.width,
        height: xml.height,
        layers,
        groups: xml.groups,
        nodes: xml.nodes,
    })
}

fn read_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|error| format!("missing {name}: {error}"))?;
    let capacity = usize::try_from(file.size()).map_err(|_| format!("{name} is too large"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("could not read {name}: {error}"))?;
    Ok(bytes)
}

fn read_optional_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<Option<Vec<u8>>, String> {
    match archive.by_name(name) {
        Ok(mut file) => {
            let capacity =
                usize::try_from(file.size()).map_err(|_| format!("{name} is too large"))?;
            let mut bytes = Vec::with_capacity(capacity);
            file.read_to_end(&mut bytes)
                .map_err(|error| format!("could not read {name}: {error}"))?;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(format!("could not open {name}: {error}")),
    }
}

fn parse_xml(bytes: &[u8]) -> Result<XmlDocument, String> {
    let xml = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut width = None;
    let mut height = None;
    let mut image_name = None;
    let mut document_color_space = "RGBA".to_string();
    let mut profile_name = None;
    let mut layers = Vec::new();
    let mut groups = Vec::new();
    let mut nodes = Vec::new();
    let mut scopes = Vec::<Option<u32>>::new();
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(element) => match element.local_name().as_ref() {
                "IMAGE" => {
                    width = Some(parse_attribute(&element, "width")?);
                    height = Some(parse_attribute(&element, "height")?);
                    image_name = Some(required_attribute(&element, "name")?);
                    if let Some(value) = attribute(&element, "colorspacename")? {
                        document_color_space = value;
                    }
                    profile_name = attribute(&element, "profile")?;
                }
                "layer" => {
                    let scope = parse_layer(
                        &element,
                        &document_color_space,
                        &scopes,
                        &mut layers,
                        &mut groups,
                        &mut nodes,
                    )?;
                    scopes.push(scope);
                }
                _ => {}
            },
            Event::Empty(element) if element.local_name().as_ref() == "layer" => {
                parse_layer(
                    &element,
                    &document_color_space,
                    &scopes,
                    &mut layers,
                    &mut groups,
                    &mut nodes,
                )?;
            }
            Event::End(element) if element.local_name().as_ref() == "layer" => {
                scopes.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(XmlDocument {
        width: width.ok_or_else(|| "Krita document has no width".to_string())?,
        height: height.ok_or_else(|| "Krita document has no height".to_string())?,
        image_name: image_name.ok_or_else(|| "Krita document has no image name".to_string())?,
        profile_name,
        layers,
        groups,
        nodes,
    })
}

fn parse_layer(
    element: &BytesStart<'_>,
    document_color_space: &str,
    scopes: &[Option<u32>],
    layers: &mut Vec<LayerMetadata>,
    groups: &mut Vec<Group>,
    nodes: &mut Vec<Node>,
) -> Result<Option<u32>, String> {
    let node_type = required_attribute(element, "nodetype")?;
    let parent = scopes.iter().rev().find_map(|scope| *scope);
    if node_type == "grouplayer" {
        let id = groups.len() as u32;
        groups.push(Group {
            id,
            name: required_attribute(element, "name")?,
            parent,
            visible: bool_attribute(element, "visible", true)?,
            opacity: u8_attribute(element, "opacity", u8::MAX)?,
        });
        nodes.push(Node::Group(id));
        return Ok(Some(id));
    }
    if matches!(node_type.as_str(), "paintlayer" | "shapelayer") {
        let index = layers.len();
        layers.push(LayerMetadata {
            name: required_attribute(element, "name")?,
            filename: required_attribute(element, "filename")?,
            vector: node_type == "shapelayer",
            color_space: attribute(element, "colorspacename")?
                .unwrap_or_else(|| document_color_space.to_string()),
            profile_name: attribute(element, "profile")?,
            position: IVec2::new(
                parse_attribute_or(element, "x", 0)?,
                parse_attribute_or(element, "y", 0)?,
            ),
            parent,
            visible: bool_attribute(element, "visible", true)?,
            opacity: u8_attribute(element, "opacity", u8::MAX)?,
            blend_mode: attribute(element, "compositeop")?.unwrap_or_else(|| "normal".to_string()),
        });
        nodes.push(Node::Layer(index));
    }
    Ok(None)
}

fn render_vector_layer(
    bytes: &[u8],
    width: u32,
    height: u32,
    pixels: usize,
) -> Result<Vec<u8>, String> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(vec![0; pixels * 4]);
    }
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("Krita vector layer is not valid UTF-8: {error}"))?;
    let dom = svg::Dom::from_str(source, FontMgr::new())
        .map_err(|error| format!("could not parse Krita vector layer SVG: {error}"))?;
    let width_i32 = i32::try_from(width).map_err(|_| "Krita vector layer is too wide")?;
    let height_i32 = i32::try_from(height).map_err(|_| "Krita vector layer is too tall")?;
    let mut root = dom.root();
    root.set_width(svg::Length::new(width as f32, svg::LengthUnit::PX));
    root.set_height(svg::Length::new(height as f32, svg::LengthUnit::PX));
    let mut surface = surfaces::raster_n32_premul((width_i32, height_i32))
        .ok_or_else(|| "could not allocate Krita vector layer surface".to_string())?;
    surface.canvas().clear(Color::TRANSPARENT);
    dom.render(surface.canvas());

    let row_bytes = width as usize * 4;
    let mut rgba = vec![0; pixels * 4];
    let image_info = ImageInfo::new(
        (width_i32, height_i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    if !surface.read_pixels(&image_info, &mut rgba, row_bytes, (0, 0)) {
        return Err("could not read Krita vector layer pixels".to_string());
    }
    Ok(rgba)
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Result<Option<String>, String> {
    for attribute in element.attributes().with_checks(false) {
        let attribute = attribute.map_err(|error| error.to_string())?;
        if attribute.key.local_name().as_ref() == key {
            return attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map(|value| Some(value.into_owned()))
                .map_err(|error| error.to_string());
        }
    }
    Ok(None)
}

fn required_attribute(element: &BytesStart<'_>, key: &str) -> Result<String, String> {
    attribute(element, key)?.ok_or_else(|| {
        format!(
            "Krita XML element {} has no {} attribute",
            element.local_name().as_ref(),
            key,
        )
    })
}

fn parse_attribute<T: std::str::FromStr>(element: &BytesStart<'_>, key: &str) -> Result<T, String> {
    required_attribute(element, key)?
        .parse()
        .map_err(|_| format!("invalid {key} attribute"))
}

fn parse_attribute_or<T: std::str::FromStr>(
    element: &BytesStart<'_>,
    key: &str,
    default: T,
) -> Result<T, String> {
    attribute(element, key)?.map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| format!("invalid {key} attribute"))
    })
}

fn bool_attribute(element: &BytesStart<'_>, key: &str, default: bool) -> Result<bool, String> {
    Ok(attribute(element, key)?
        .map(|value| value != "0" && value != "false")
        .unwrap_or(default))
}

fn u8_attribute(element: &BytesStart<'_>, key: &str, default: u8) -> Result<u8, String> {
    attribute(element, key)?.map_or(Ok(default), |value| {
        value
            .parse::<u8>()
            .map_err(|_| format!("invalid {key} attribute"))
    })
}

fn decode_tiles(
    bytes: &[u8],
    size: UVec2,
    offset: IVec2,
    spec: PixelSpec,
    default_pixel: &[u8],
    pixels: usize,
) -> Result<Vec<u8>, String> {
    let pixel_size = spec.pixel_size();
    let mut native = Vec::with_capacity(
        pixels
            .checked_mul(pixel_size)
            .ok_or_else(|| "Krita layer is too large".to_string())?,
    );
    for _ in 0..pixels {
        native.extend_from_slice(default_pixel);
    }
    let mut cursor = Cursor::new(bytes);
    expect_header(&mut cursor, "VERSION", "2")?;
    expect_header(&mut cursor, "TILEWIDTH", &TILE_WIDTH.to_string())?;
    expect_header(&mut cursor, "TILEHEIGHT", &TILE_HEIGHT.to_string())?;
    expect_header(&mut cursor, "PIXELSIZE", &pixel_size.to_string())?;
    let tile_count = read_header(&mut cursor, "DATA")?
        .parse::<usize>()
        .map_err(|_| "invalid Krita tile count".to_string())?;
    let tile_bytes = TILE_WIDTH * TILE_HEIGHT * pixel_size;
    for _ in 0..tile_count {
        let mut header = String::new();
        cursor
            .read_line(&mut header)
            .map_err(|error| error.to_string())?;
        let fields = header.trim_end().split(',').collect::<Vec<_>>();
        if fields.len() != 4 || fields[2] != "LZF" {
            return Err(format!("invalid Krita tile header {header:?}"));
        }
        let tile_position = IVec2::new(
            fields[0]
                .parse::<i32>()
                .map_err(|_| "invalid Krita tile x coordinate".to_string())?,
            fields[1]
                .parse::<i32>()
                .map_err(|_| "invalid Krita tile y coordinate".to_string())?,
        );
        let payload_size = fields[3]
            .parse::<usize>()
            .map_err(|_| "invalid Krita tile size".to_string())?;
        if payload_size == 0 || payload_size > tile_bytes + 1 {
            return Err(format!("invalid Krita tile payload size {payload_size}"));
        }
        let mut payload = vec![0; payload_size];
        cursor
            .read_exact(&mut payload)
            .map_err(|error| format!("truncated Krita tile: {error}"))?;
        let tile = match payload[0] {
            0 if payload.len() == tile_bytes + 1 => payload[1..].to_vec(),
            0 => return Err("raw Krita tile has the wrong size".to_string()),
            1 => {
                let linear = lzf::decompress(&payload[1..], tile_bytes)
                    .map_err(|error| format!("could not decompress Krita tile: {error:?}"))?;
                delinearize(&linear, pixel_size)
            }
            flag => return Err(format!("unknown Krita tile compression flag {flag}")),
        };
        copy_tile(
            &mut native,
            size,
            pixel_size,
            tile_position.as_i64vec2() + offset.as_i64vec2(),
            &tile,
        );
    }
    Ok(native)
}

fn expect_header(cursor: &mut Cursor<&[u8]>, key: &str, expected: &str) -> Result<(), String> {
    let actual = read_header(cursor, key)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("unsupported Krita {key} {actual}"))
    }
}

fn read_header(cursor: &mut Cursor<&[u8]>, key: &str) -> Result<String, String> {
    let mut line = String::new();
    cursor
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    let (actual_key, value) = line
        .trim_end()
        .split_once(' ')
        .ok_or_else(|| format!("invalid Krita layer header {line:?}"))?;
    if actual_key != key {
        return Err(format!("expected Krita {key} header, found {actual_key}"));
    }
    Ok(value.to_string())
}

fn delinearize(linear: &[u8], pixel_size: usize) -> Vec<u8> {
    let stride = linear.len() / pixel_size;
    let mut pixels = vec![0; linear.len()];
    for pixel in 0..stride {
        for byte in 0..pixel_size {
            pixels[pixel * pixel_size + byte] = linear[byte * stride + pixel];
        }
    }
    pixels
}

fn copy_tile(
    canvas: &mut [u8],
    canvas_size: UVec2,
    pixel_size: usize,
    tile_position: I64Vec2,
    tile: &[u8],
) {
    for row in 0..TILE_HEIGHT {
        let y = tile_position.y + row as i64;
        if !(0..i64::from(canvas_size.y)).contains(&y) {
            continue;
        }
        for column in 0..TILE_WIDTH {
            let x = tile_position.x + column as i64;
            if !(0..i64::from(canvas_size.x)).contains(&x) {
                continue;
            }
            let source = (row * TILE_WIDTH + column) * pixel_size;
            let destination = (y as usize * canvas_size.x as usize + x as usize) * pixel_size;
            canvas[destination..destination + pixel_size]
                .copy_from_slice(&tile[source..source + pixel_size]);
        }
    }
}

fn convert_to_rgba8(
    native: &[u8],
    spec: PixelSpec,
    profile: Option<&[u8]>,
    profile_name: Option<&str>,
) -> Result<Vec<u8>, String> {
    let source = normalize_u8(native, spec)?;
    if spec.model == Model::Alpha {
        return Ok(source
            .into_iter()
            .flat_map(|gray| [gray, gray, gray, u8::MAX])
            .collect());
    }
    let source_profile = if let Some(profile) = profile {
        ColorProfile::new_from_slice(profile)
            .map_err(|error| format!("could not parse embedded ICC profile: {error:?}"))?
    } else if spec.model == Model::Rgb
        && profile_name
            .is_none_or(|name| name.is_empty() || name.to_ascii_lowercase().contains("srgb"))
    {
        ColorProfile::new_srgb()
    } else {
        return Err(format!(
            "{} pixels require an embedded ICC profile",
            model_name(spec.model),
        ));
    };
    let destination_profile = ColorProfile::new_srgb();
    let layout = match (spec.model, spec.channels) {
        (Model::Gray, 2) => Layout::GrayAlpha,
        (Model::Cmyk, 5) => Layout::Cmyka,
        (_, 4) => Layout::Rgba,
        _ => return Err("unsupported Krita channel layout".to_string()),
    };
    let transform = source_profile
        .create_transform_8bit(
            layout,
            &destination_profile,
            Layout::Rgba,
            TransformOptions::default(),
        )
        .map_err(|error| format!("could not create ICC transform: {error:?}"))?;
    let pixel_count = native.len() / spec.pixel_size();
    let mut rgba = vec![0; pixel_count * 4];
    transform
        .transform(&source, &mut rgba)
        .map_err(|error| format!("ICC transform failed: {error:?}"))?;
    Ok(rgba)
}

fn normalize_u8(native: &[u8], spec: PixelSpec) -> Result<Vec<u8>, String> {
    if !native.len().is_multiple_of(spec.pixel_size()) {
        return Err("Krita pixel buffer has the wrong size".to_string());
    }
    let mut output = vec![0; native.len() / spec.bytes_per_channel()];
    output
        .par_chunks_mut(spec.channels)
        .zip(native.par_chunks_exact(spec.pixel_size()))
        .for_each(|(channels, pixel)| {
            for (channel, output) in channels.iter_mut().enumerate() {
                let offset = channel * spec.bytes_per_channel();
                *output = match spec.depth {
                    Depth::U8 => pixel[offset],
                    Depth::U16 => {
                        let value = u16::from_le_bytes([pixel[offset], pixel[offset + 1]]);
                        (value >> 8) as u8
                    }
                    Depth::F16 => {
                        let value = f16::from_le_bytes([pixel[offset], pixel[offset + 1]]).to_f32();
                        unit_float_to_u8(value)
                    }
                    Depth::F32 => {
                        let value = f32::from_le_bytes([
                            pixel[offset],
                            pixel[offset + 1],
                            pixel[offset + 2],
                            pixel[offset + 3],
                        ]);
                        unit_float_to_u8(value)
                    }
                };
            }
            if spec.blue_first {
                channels.swap(0, 2);
            }
        });
    Ok(output)
}

fn unit_float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn model_name(model: Model) -> &'static str {
    match model {
        Model::Rgb => "RGB",
        Model::Gray => "grayscale",
        Model::Cmyk => "CMYK",
        Model::Lab => "Lab",
        Model::Xyz => "XYZ",
        Model::YCbCr => "YCbCr",
        Model::Alpha => "alpha",
    }
}
