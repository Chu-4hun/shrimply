use crate::project::{CaptionFont, CaptionItem, HorizontalAlign, Project, Time, VerticalAlign};
use crate::timeline::renderer::{Color, Rect, TimelinePainter, Vec2, vec2};
use skia_safe::{
    FontMgr, FontStyle, Point,
    textlayout::{
        FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextAlign, TextDecoration,
        TextStyle,
    },
};

const PREVIEW_CAPTION_SAFE_MARGIN: f32 = 24.0;
const PREVIEW_CAPTION_BOTTOM_PADDING: f32 = 24.0;
const PREVIEW_CAPTION_PADDING_X: f32 = 8.0;
const PREVIEW_CAPTION_PADDING_Y: f32 = 3.0;
const PARAGRAPH_LAYOUT_EPSILON: f32 = 1.0;

thread_local! {
    static CAPTION_FONTS: FontCollection = {
        let mut fonts = FontCollection::new();
        fonts.set_default_font_manager(FontMgr::new(), None);
        fonts
    };
}

#[derive(Clone)]
struct CaptionFontId {
    size: f32,
    font: CaptionFont,
}

trait CaptionPainter {
    fn caption_text(
        &self,
        position: Vec2,
        text: &str,
        font: CaptionFontId,
        color: Color,
        width: f32,
        align: HorizontalAlign,
    );

    fn layout_caption_text(
        &self,
        text: &str,
        font: CaptionFontId,
        color: Color,
        width: f32,
        align: HorizontalAlign,
    ) -> Vec2;
}

impl CaptionPainter for TimelinePainter {
    fn caption_text(
        &self,
        position: Vec2,
        text: &str,
        font: CaptionFontId,
        color: Color,
        width: f32,
        align: HorizontalAlign,
    ) {
        caption_paragraph(text, font, color, width, align)
            .paint(self.canvas(), Point::new(position.x, position.y));
    }

    fn layout_caption_text(
        &self,
        text: &str,
        font: CaptionFontId,
        color: Color,
        width: f32,
        align: HorizontalAlign,
    ) -> Vec2 {
        let paragraph = caption_paragraph(text, font, color, width, align);
        Vec2::new(paragraph.max_width(), paragraph.height())
    }
}

fn caption_paragraph(
    text: &str,
    font: CaptionFontId,
    color: Color,
    width: f32,
    align: HorizontalAlign,
) -> Paragraph {
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(match align {
        HorizontalAlign::Left => TextAlign::Left,
        HorizontalAlign::Center => TextAlign::Center,
        HorizontalAlign::Right => TextAlign::Right,
    });
    let mut builder =
        CAPTION_FONTS.with(|fonts| ParagraphBuilder::new(&paragraph_style, fonts.clone()));
    for span in crate::caption::markup::parse(text) {
        let mut style = TextStyle::new();
        let family = match font.font {
            CaptionFont::Roboto => "Roboto",
            CaptionFont::MonospaceSerif => "Courier New",
            CaptionFont::Serif => "Times New Roman",
            CaptionFont::MonospaceSans => "Lucida Console",
            CaptionFont::Casual => "Comic Sans MS",
            CaptionFont::Cursive => "Monotype Corsiva",
            CaptionFont::SmallCapitals => "Arial",
        };
        style
            .set_color(color)
            .set_font_size(font.size.max(1.0))
            .set_font_families(&[family]);
        style.set_font_style(match (span.bold, span.italic) {
            (true, true) => FontStyle::bold_italic(),
            (true, false) => FontStyle::bold(),
            (false, true) => FontStyle::italic(),
            _ => FontStyle::normal(),
        });
        if span.underline {
            style.set_decoration_type(TextDecoration::UNDERLINE);
        }
        builder
            .push_style(&style)
            .add_text(span.ruby.map_or(span.text, |ruby| ruby.base));
    }
    let mut paragraph = builder.build();
    paragraph.layout(f32::MAX);
    let unwrapped_width = paragraph.max_intrinsic_width() + PARAGRAPH_LAYOUT_EPSILON;
    paragraph.layout(if unwrapped_width <= width {
        unwrapped_width
    } else {
        width.max(1.0)
    });
    paragraph
}

pub(super) fn draw_captions(
    painter: &TimelinePainter,
    project: &Project,
    position: Time,
    preview_rect: Rect,
    caption_font_size: f32,
    caption_bottom_inset: f32,
) {
    if preview_rect.width() <= 1.0 || preview_rect.height() <= 1.0 {
        return;
    }

    let mut active = project
        .caption_tracks
        .iter()
        .rev()
        .filter(|track| track.enabled)
        .flat_map(|track| &track.items)
        .filter(|item| position >= item.start && position < item.end)
        .collect::<Vec<_>>();
    active.sort_by_key(|item| {
        (
            if item.layout_enabled {
                item.position_y
            } else {
                90
            },
            item.start,
        )
    });
    let mut bottom_stack = caption_bottom_inset;
    for item in active {
        let defaults = (!item.styling_enabled || !item.layout_enabled)
            .then(|| CaptionItem::new(item.start, item.end, item.text.clone()));
        let effective_item = defaults.as_ref().map(|defaults| {
            let mut item = item.clone();
            if !item.styling_enabled {
                item.text_color = defaults.text_color;
                item.background_color = defaults.background_color;
                item.edge_color = defaults.edge_color;
                item.edge_style = defaults.edge_style;
                item.font = defaults.font;
                item.font_scale = defaults.font_scale;
            }
            if !item.layout_enabled {
                item.h_align = defaults.h_align;
                item.v_align = defaults.v_align;
                item.position_x = defaults.position_x;
                item.position_y = defaults.position_y;
            }
            item
        });
        let item = effective_item.as_ref().unwrap_or(item);
        let automatic_bottom = item.h_align == HorizontalAlign::Center
            && item.v_align == VerticalAlign::Bottom
            && item.position_x == 50
            && item.position_y == 90;
        let height = draw_caption(
            painter,
            item,
            position,
            preview_rect,
            caption_font_size,
            automatic_bottom,
            if automatic_bottom { bottom_stack } else { 0.0 },
        );
        if automatic_bottom {
            bottom_stack += height + PREVIEW_CAPTION_PADDING_Y * 2.0;
        }
    }
}

fn draw_caption(
    painter: &TimelinePainter,
    item: &CaptionItem,
    playback_position: Time,
    preview_rect: Rect,
    caption_font_size: f32,
    automatic_bottom: bool,
    caption_bottom_inset: f32,
) -> f32 {
    if item.text.trim().is_empty() || item.text_color.a == 0 {
        return 0.0;
    }

    let margin = PREVIEW_CAPTION_SAFE_MARGIN;
    let wrap_width = (preview_rect.width() - margin * 2.0).max(1.0);
    // Font size and decoration are deliberately constant in preview pixels. Only the caption's
    // percentage position follows the preview canvas; neither canvas dimensions nor video bounds
    // may scale these metrics.
    let font_size = caption_font_size * f32::from(item.font_scale) / 100.0;
    let font = CaptionFontId {
        size: font_size,
        font: item.font,
    };
    let text_color = item.text_color.into();
    let elapsed_millis =
        (playback_position.as_nanos_i128() - item.start.as_nanos_i128()).max(0) / 1_000_000;
    let visible_text = crate::caption::markup::visible_text(
        &item.text,
        elapsed_millis.min(i128::from(u32::MAX)) as u32,
    );
    let vertical = item.writing_direction != crate::project::CaptionWritingDirection::Horizontal;
    let text = vertical.then(|| {
        crate::caption::markup::plain_text(&visible_text)
            .chars()
            .flat_map(|character| [character, '\n'])
            .collect::<String>()
    });
    let text = text.as_deref().unwrap_or(&visible_text);
    let text_size =
        painter.layout_caption_text(text, font.clone(), text_color, wrap_width, item.h_align);
    if text_size.x <= 0.0 || text_size.y <= 0.0 {
        return 0.0;
    }
    let text_pos = caption_position(
        item,
        preview_rect,
        margin,
        text_size,
        automatic_bottom,
        caption_bottom_inset,
    );
    let background_rect = Rect::from_min_max(
        vec2(
            text_pos.x - PREVIEW_CAPTION_PADDING_X,
            text_pos.y - PREVIEW_CAPTION_PADDING_Y,
        ),
        vec2(
            text_pos.x + text_size.x + PREVIEW_CAPTION_PADDING_X,
            text_pos.y + text_size.y + PREVIEW_CAPTION_PADDING_Y,
        ),
    );
    if item.background_color.a > 0 {
        painter.rect_filled(background_rect, 0, item.background_color.into());
    }

    draw_caption_edge(painter, item, text_pos, text, font.clone(), wrap_width);
    painter.caption_text(text_pos, text, font, text_color, wrap_width, item.h_align);
    text_size.y
}

fn draw_caption_edge(
    painter: &TimelinePainter,
    item: &CaptionItem,
    position: Vec2,
    text: &str,
    font: CaptionFontId,
    width: f32,
) {
    use crate::project::CaptionEdgeStyle;
    let color = item.edge_color.into();
    let offsets: &[Vec2] = match item.edge_style {
        CaptionEdgeStyle::None => &[],
        CaptionEdgeStyle::HardShadow => &[Vec2::splat(2.0)],
        CaptionEdgeStyle::Bevel => &[Vec2::splat(-1.0), Vec2::ONE],
        CaptionEdgeStyle::Glow => &[Vec2::NEG_X, Vec2::X, Vec2::NEG_Y, Vec2::Y],
        CaptionEdgeStyle::SoftShadow => &[Vec2::ONE, Vec2::splat(2.0)],
    };
    for &offset in offsets {
        painter.caption_text(
            position + offset,
            text,
            font.clone(),
            color,
            width,
            item.h_align,
        );
    }
}

fn caption_position(
    item: &CaptionItem,
    preview_rect: Rect,
    margin: f32,
    size: crate::timeline::renderer::Vec2,
    automatic_bottom: bool,
    bottom_inset: f32,
) -> Vec2 {
    let anchor_x = preview_rect.left() + preview_rect.width() * f32::from(item.position_x) / 100.0;
    let x = match item.h_align {
        HorizontalAlign::Left => anchor_x,
        HorizontalAlign::Center => anchor_x - size.x / 2.0,
        HorizontalAlign::Right => anchor_x - size.x,
    };
    // Default bottom captions use fixed preview-space padding. Percentage positioning belongs to
    // explicit layouts only, so resizing the preview does not change the default bottom gap.
    let y = if automatic_bottom {
        preview_rect.bottom()
            - PREVIEW_CAPTION_BOTTOM_PADDING
            - PREVIEW_CAPTION_PADDING_Y
            - size.y
            - bottom_inset.max(0.0)
    } else {
        let anchor_y =
            preview_rect.top() + preview_rect.height() * f32::from(item.position_y) / 100.0;
        match item.v_align {
            VerticalAlign::Top => anchor_y,
            VerticalAlign::Middle => anchor_y - size.y / 2.0,
            VerticalAlign::Bottom => anchor_y - size.y,
        }
    };

    let min_x = preview_rect.left() + margin;
    let max_x = (preview_rect.right() - margin - size.x).max(min_x);
    let min_y = preview_rect.top() + margin;
    let max_y = (preview_rect.bottom() - margin - bottom_inset.max(0.0) - size.y).max(min_y);
    vec2(x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}
