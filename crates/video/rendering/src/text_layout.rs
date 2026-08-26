use std::{
    cell::{Cell, RefCell},
    path::{Path as FsPath, PathBuf},
    rc::Rc,
    sync::{
        OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use cached::{Cached, stores::LruCache};
use cairo::PathSegment;
use glib::translate::from_glib;
use pango::prelude::FontMapExt;
use pango::{Alignment, FontDescription, Gravity, GravityHint};
use skia_safe::{Path, PathBuilder, Rect};

use shrimply_project::project::{
    FontFamily, TextDirection, TextFontStyle, TextHorizontalAlign, TextItem, VerticalAlign,
};

pub struct TextLayout {
    pub(crate) path: Path,
    pub(crate) subpaths: Vec<Path>,
    pub(crate) word_subpaths: Vec<Path>,
    pub(crate) mask_units: Vec<TextMaskUnit>,
    pub size: glam::Vec2,
}

pub(crate) struct TextMaskUnit {
    pub path: Path,
}

const MAX_LAYOUT_CACHE_ENTRIES: usize = 1024;

static APPLICATION_FONT_REVISION: AtomicU64 = AtomicU64::new(0);
static APPLICATION_FONT_FILES: OnceLock<RwLock<Vec<PathBuf>>> = OnceLock::new();

thread_local! {
    static LAYOUTS: RefCell<LruCache<Key, Rc<TextLayout>>> = RefCell::new(
        LruCache::builder()
            .max_size(MAX_LAYOUT_CACHE_ENTRIES)
            .build()
            .expect("valid Pango text layout cache size"),
    );
    static REGISTERED_FONT_REVISION: Cell<u64> = const { Cell::new(0) };
}

pub fn register_application_font(path: impl AsRef<FsPath>) -> Result<(), String> {
    let path = path.as_ref();
    shrimply_text_3d::register_application_font(path)?;
    pangocairo::FontMap::default()
        .add_font_file(path)
        .map_err(|error| format!("could not register font {}: {error}", path.display()))?;
    let fonts = APPLICATION_FONT_FILES.get_or_init(|| RwLock::new(Vec::new()));
    let mut fonts = fonts
        .write()
        .unwrap_or_else(|_| panic!("application font registry lock died"));
    if fonts.iter().any(|registered| registered == path) {
        return Ok(());
    }
    fonts.push(path.to_path_buf());
    let revision = APPLICATION_FONT_REVISION.fetch_add(1, Ordering::AcqRel) + 1;
    REGISTERED_FONT_REVISION.set(revision);
    LAYOUTS.with(|layouts| layouts.borrow_mut().cache_clear());
    Ok(())
}

pub fn layout(
    text: &TextItem,
    content: &str,
    font_size: f32,
    font_weight: f32,
    tracking: f32,
    line_height: f32,
    time: shrimply_project::project::Time,
) -> Rc<TextLayout> {
    register_pending_application_fonts();
    let key = Key {
        text: content.to_string(),
        h_align: text.h_align.value_at(time),
        v_align: text.v_align.value_at(time),
        direction: text.direction.value_at(time),
        font_families: text.font_families.clone(),
        font_style: text.font_style.value_at(time),
        font_variations: normalized_variations(text),
        font_size: font_size.to_bits(),
        font_weight: font_weight.round().clamp(1.0, 1000.0) as i32,
        tracking: tracking.to_bits(),
        line_height: line_height.to_bits(),
    };
    if let Some(layout) =
        LAYOUTS.with(|layouts| layouts.borrow_mut().cache_get(&key).map(Rc::clone))
    {
        return layout;
    }

    let layout = Rc::new(build(
        text,
        content,
        font_size,
        font_weight,
        tracking,
        line_height,
        time,
    ));
    LAYOUTS.with(|layouts| {
        layouts.borrow_mut().cache_set(key, Rc::clone(&layout));
    });
    layout
}

fn register_pending_application_fonts() {
    let revision = APPLICATION_FONT_REVISION.load(Ordering::Acquire);
    if REGISTERED_FONT_REVISION.get() == revision {
        return;
    }
    let Some(fonts) = APPLICATION_FONT_FILES.get() else {
        REGISTERED_FONT_REVISION.set(revision);
        return;
    };
    let fonts = fonts
        .read()
        .unwrap_or_else(|_| panic!("application font registry lock died"));
    let font_map = pangocairo::FontMap::default();
    for path in fonts.iter() {
        if let Err(error) = font_map.add_font_file(path) {
            tracing::warn!(path = %path.display(), "Could not register application font: {error}");
        }
    }
    REGISTERED_FONT_REVISION.set(revision);
    LAYOUTS.with(|layouts| layouts.borrow_mut().cache_clear());
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct Key {
    text: String,
    h_align: TextHorizontalAlign,
    v_align: VerticalAlign,
    direction: TextDirection,
    font_families: Vec<FontFamily>,
    font_style: TextFontStyle,
    font_variations: Vec<(String, u32)>,
    font_size: u32,
    font_weight: i32,
    tracking: u32,
    line_height: u32,
}

fn build(
    text: &TextItem,
    content: &str,
    font_size: f32,
    font_weight: f32,
    tracking: f32,
    line_height: f32,
    time: shrimply_project::project::Time,
) -> TextLayout {
    let direction = text.direction.value_at(time);
    let h_align = text.h_align.value_at(time);
    let v_align = text.v_align.value_at(time);
    let font_style = text.font_style.value_at(time);
    let surface = cairo::RecordingSurface::create(cairo::Content::Alpha, None)
        .expect("create text path surface");
    let cairo = cairo::Context::new(&surface).expect("create text layout context");
    if direction == TextDirection::Vertical {
        cairo.rotate(std::f64::consts::FRAC_PI_2);
    }

    let layout = pangocairo::functions::create_layout(&cairo);
    let context = layout.context();
    context.set_base_gravity(match direction {
        TextDirection::Horizontal => Gravity::South,
        TextDirection::Vertical => Gravity::East,
    });
    context.set_gravity_hint(GravityHint::Natural);

    let mut font = FontDescription::new();
    let font_families = text
        .font_families
        .iter()
        .map(FontFamily::name)
        .collect::<Vec<_>>();
    if !font_families.is_empty() {
        font.set_family(&font_families.join(", "));
    }
    font.set_style(match font_style {
        TextFontStyle::Normal => pango::Style::Normal,
        TextFontStyle::Italic => pango::Style::Italic,
        TextFontStyle::Oblique => pango::Style::Oblique,
    });
    font.set_absolute_size(font_size.max(1.0) as f64 * pango::SCALE as f64);
    font.set_weight(unsafe { from_glib(font_weight.round().clamp(1.0, 1000.0) as i32) });
    let mut variations = normalized_variations(text)
        .into_iter()
        .map(|(axis, value)| format!("{axis}={}", f32::from_bits(value)))
        .collect::<Vec<_>>();
    variations.push(format!("wght={}", font_weight.clamp(1.0, 1000.0)));
    if font_style == TextFontStyle::Italic
        && !variations
            .iter()
            .any(|variation| variation.starts_with("ital="))
    {
        variations.push("ital=1".to_string());
    }
    font.set_variations(Some(&variations.join(",")));
    layout.set_font_description(Some(&font));
    layout.set_text(if content.is_empty() { " " } else { content });
    if tracking != 0.0 {
        let attrs = pango::AttrList::new();
        attrs.insert(pango::AttrInt::new_letter_spacing(
            (tracking * pango::SCALE as f32).round() as i32,
        ));
        layout.set_attributes(Some(&attrs));
    }
    if line_height != 1.0 {
        layout.set_line_spacing(line_height.max(f32::EPSILON));
    }

    let (_, natural) = layout.extents();
    layout.set_width(natural.width().max(1));
    let alignment = match direction {
        TextDirection::Horizontal => match h_align {
            TextHorizontalAlign::Left | TextHorizontalAlign::Fill => Alignment::Left,
            TextHorizontalAlign::Center => Alignment::Center,
            TextHorizontalAlign::Right => Alignment::Right,
        },
        TextDirection::Vertical => match v_align {
            VerticalAlign::Top => Alignment::Left,
            VerticalAlign::Middle => Alignment::Center,
            VerticalAlign::Bottom => Alignment::Right,
        },
    };
    layout.set_alignment(alignment);
    let justify = direction == TextDirection::Horizontal && h_align == TextHorizontalAlign::Fill;
    layout.set_justify(justify);
    layout.set_justify_last_line(justify);

    let mut word_indices = vec![None; content.len() + 1];
    let mut current_word = None;
    let mut word_count = 0;
    for (offset, character) in content.char_indices() {
        if character.is_whitespace() {
            current_word = None;
        } else {
            let index = *current_word.get_or_insert_with(|| {
                let index = word_count;
                word_count += 1;
                index
            });
            word_indices[offset] = Some(index);
        }
    }

    let mut character_rects = Vec::new();
    let mut iter = layout.iter();
    loop {
        let rect = iter.char_extents();
        if rect.width() > 0 && rect.height() > 0 {
            let word_index = word_indices
                .get(iter.index().max(0) as usize)
                .copied()
                .flatten();
            character_rects.push((rect, word_index));
        }
        if !iter.next_char() {
            break;
        }
    }

    let (_, logical) = layout.extents();
    cairo.move_to(0.0, 0.0);
    pangocairo::functions::layout_path(&cairo, &layout);
    cairo.identity_matrix();
    let (ink_x1, ink_y1, ink_x2, ink_y2) = cairo.path_extents().expect("measure text path");

    let scale = pango::SCALE as f64;
    let x = logical.x() as f64 / scale;
    let y = logical.y() as f64 / scale;
    let width = logical.width() as f64 / scale;
    let height = logical.height() as f64 / scale;
    let (logical_x1, logical_y1, logical_x2, logical_y2) = match direction {
        TextDirection::Horizontal => (x, y, x + width, y + height),
        TextDirection::Vertical => (-(y + height), x, -y, x + width),
    };
    let min_x = logical_x1.min(ink_x1);
    let min_y = logical_y1.min(ink_y1);
    let max_x = logical_x2.max(ink_x2);
    let max_y = logical_y2.max(ink_y2);

    let cairo_path = cairo.copy_path().expect("copy text path");
    let mut path = PathBuilder::new();
    let point = |(x, y): (f64, f64)| ((x - min_x) as f32, (y - min_y) as f32);
    for segment in cairo_path.iter() {
        match segment {
            PathSegment::MoveTo(to) => {
                path.move_to(point(to));
            }
            PathSegment::LineTo(to) => {
                path.line_to(point(to));
            }
            PathSegment::CurveTo(control_1, control_2, to) => {
                path.cubic_to(point(control_1), point(control_2), point(to));
            }
            PathSegment::ClosePath => {
                path.close();
            }
        }
    }

    let path: Path = path.into();
    let scale = pango::SCALE as f32;
    let character_rects = character_rects
        .into_iter()
        .map(|(rect, word_index)| {
            let x = rect.x() as f32 / scale;
            let y = rect.y() as f32 / scale;
            let width = rect.width() as f32 / scale;
            let height = rect.height() as f32 / scale;
            let rect = match direction {
                TextDirection::Horizontal => {
                    Rect::from_xywh(x - min_x as f32, y - min_y as f32, width, height)
                }
                TextDirection::Vertical => {
                    Rect::from_xywh(-y - height - min_x as f32, x - min_y as f32, height, width)
                }
            };
            (rect, word_index)
        })
        .collect::<Vec<_>>();
    let mut subpaths = character_rects
        .iter()
        .map(|_| PathBuilder::new())
        .collect::<Vec<_>>();
    let mut word_subpaths = (0..word_count)
        .map(|_| PathBuilder::new())
        .collect::<Vec<_>>();
    for contour in crate::path_transition::contours(&path) {
        let center = contour.compute_tight_bounds().center();
        let Some((index, _)) =
            character_rects
                .iter()
                .enumerate()
                .min_by(|(_, (left, _)), (_, (right, _))| {
                    let distance = |rect: &Rect| {
                        let dx = center.x - center.x.clamp(rect.left, rect.right);
                        let dy = center.y - center.y.clamp(rect.top, rect.bottom);
                        dx * dx + dy * dy
                    };
                    distance(left).total_cmp(&distance(right))
                })
        else {
            continue;
        };
        subpaths[index].add_path(&contour, None);
        if let Some(word_index) = character_rects[index].1 {
            word_subpaths[word_index].add_path(&contour, None);
        }
    }
    let subpaths = subpaths
        .into_iter()
        .map(|mut builder| builder.detach())
        .collect::<Vec<_>>();
    let mask_units = character_rects
        .iter()
        .zip(&subpaths)
        .map(|(_, path)| TextMaskUnit { path: path.clone() })
        .collect();
    let subpaths = subpaths
        .into_iter()
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    let word_subpaths = word_subpaths
        .into_iter()
        .map(|mut builder| builder.detach())
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();

    TextLayout {
        subpaths: if subpaths.is_empty() {
            vec![path.clone()]
        } else {
            subpaths
        },
        word_subpaths: if word_subpaths.is_empty() {
            vec![path.clone()]
        } else {
            word_subpaths
        },
        mask_units,
        path,
        size: glam::Vec2::new((max_x - min_x) as f32, (max_y - min_y) as f32),
    }
}

fn normalized_variations(text: &TextItem) -> Vec<(String, u32)> {
    let mut variations = text
        .font_variations
        .iter()
        .filter(|variation| {
            variation.value.is_finite()
                && variation.axis.len() == 4
                && variation.axis.bytes().all(|byte| byte.is_ascii_graphic())
                && variation.axis != "wght"
                && variation.axis != "ital"
        })
        .map(|variation| (variation.axis.clone(), variation.value.to_bits()))
        .collect::<Vec<_>>();
    variations.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    variations.dedup_by(|left, right| left.0 == right.0);
    variations
}

pub fn anchor(
    size: glam::Vec2,
    h_align: TextHorizontalAlign,
    v_align: VerticalAlign,
) -> glam::Vec2 {
    glam::Vec2::new(
        match h_align {
            TextHorizontalAlign::Left => 0.0,
            TextHorizontalAlign::Center | TextHorizontalAlign::Fill => size.x * 0.5,
            TextHorizontalAlign::Right => size.x,
        },
        match v_align {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Middle => size.y * 0.5,
            VerticalAlign::Bottom => size.y,
        },
    )
}
