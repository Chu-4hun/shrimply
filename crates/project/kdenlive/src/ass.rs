use std::fs;
use std::path::Path;

use shrimply_math_core::Fraction;
use shrimply_project::{
    CaptionEdgeStyle, CaptionFont, CaptionItem, Color, HorizontalAlign, VerticalAlign,
};

use crate::math;

pub fn read(path: &Path) -> Result<Vec<CaptionItem>, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read subtitles {}: {error}", path.display()))?;
    let mut events = false;
    let mut items = Vec::new();
    for line in source.lines() {
        if line == "[Events]" {
            events = true;
            continue;
        }
        if line.starts_with('[') {
            events = false;
            continue;
        }
        if !events {
            continue;
        }
        let Some(value) = line.strip_prefix("Dialogue: ") else {
            continue;
        };
        let fields = value.splitn(10, ',').collect::<Vec<_>>();
        if fields.len() != 10 {
            return Err(format!("invalid ASS dialogue in {}", path.display()));
        }
        let start = math::time(ass_centiseconds(fields[1])?, Fraction::from(100u32));
        let end = math::time(ass_centiseconds(fields[2])?, Fraction::from(100u32));
        let mut item = CaptionItem::new(
            start,
            end,
            fields[9]
                .replace("\\N", "\n")
                .replace("\\n", "\n")
                .replace("\\h", " "),
        );
        item.styling_enabled = true;
        item.layout_enabled = true;
        item.h_align = HorizontalAlign::Center;
        item.v_align = VerticalAlign::Bottom;
        item.position_x = 50;
        item.position_y = 96;
        item.text_color = Color::<u8>::WHITE;
        item.background_color = Color::<u8>::TRANSPARENT;
        item.edge_color = Color::<u8>::BLACK;
        item.edge_style = CaptionEdgeStyle::Glow;
        item.font = CaptionFont::Roboto;
        item.font_scale = 100;
        items.push(item);
    }
    Ok(items)
}

fn ass_centiseconds(value: &str) -> Result<i64, String> {
    let parts = value.split([':', '.']).collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err(format!("invalid ASS time {value:?}"));
    }
    let values = parts
        .iter()
        .map(|part| {
            part.parse::<i64>()
                .map_err(|_| format!("invalid ASS time {value:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(((values[0] * 60 + values[1]) * 60 + values[2]) * 100 + values[3])
}
