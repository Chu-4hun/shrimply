use crate::project::{Color, SvgColorOverride, SvgPaintKind};

pub const SVG_COLOR_LIMIT: usize = 32;
const DEFAULT_FILL: Color<u8> = Color::<u8>::BLACK;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgPaintColor {
    pub kind: SvgPaintKind,
    pub color: Color<u8>,
}

pub fn paint_colors(svg: &str) -> Vec<SvgPaintColor> {
    let mut colors = Vec::new();
    collect_attr_colors(svg, "fill", SvgPaintKind::Fill, &mut colors);
    collect_attr_colors(svg, "stroke", SvgPaintKind::Stroke, &mut colors);
    collect_style_colors(svg, &mut colors);
    if uses_default_fill(svg) {
        push_unique(
            &mut colors,
            SvgPaintColor {
                kind: SvgPaintKind::Fill,
                color: DEFAULT_FILL,
            },
        );
    }
    colors
}

pub fn apply_overrides(svg: &str, overrides: &[SvgColorOverride]) -> String {
    if overrides.is_empty() {
        return svg.to_string();
    }

    let default_fill = uses_default_fill(svg)
        .then(|| override_color(overrides, SvgPaintKind::Fill, DEFAULT_FILL))
        .flatten();
    let mut output = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('>') else {
            output.push_str(rest);
            return output;
        };
        output.push_str(&rewrite_tag(&rest[..=end], overrides, default_fill));
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

fn collect_attr_colors(
    svg: &str,
    attr: &'static str,
    kind: SvgPaintKind,
    colors: &mut Vec<SvgPaintColor>,
) {
    let mut rest = svg;
    while colors.len() < SVG_COLOR_LIMIT {
        let Some((_, value, next)) = find_attr(rest, attr) else {
            break;
        };
        if let Some(color) = parse_svg_color(value) {
            push_unique(colors, SvgPaintColor { kind, color });
        }
        rest = next;
    }
}

fn collect_style_colors(svg: &str, colors: &mut Vec<SvgPaintColor>) {
    let mut rest = svg;
    while colors.len() < SVG_COLOR_LIMIT {
        let Some((_, style, next)) = find_attr(rest, "style") else {
            break;
        };
        for declaration in style.split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            let kind = match name.trim() {
                "fill" => SvgPaintKind::Fill,
                "stroke" => SvgPaintKind::Stroke,
                _ => continue,
            };
            if let Some(color) = parse_svg_color(value) {
                push_unique(colors, SvgPaintColor { kind, color });
            }
            if colors.len() >= SVG_COLOR_LIMIT {
                break;
            }
        }
        rest = next;
    }
}

fn rewrite_tag(
    tag: &str,
    overrides: &[SvgColorOverride],
    default_fill: Option<Color<u8>>,
) -> String {
    let with_fill = rewrite_attr(tag, "fill", SvgPaintKind::Fill, overrides);
    let with_stroke = rewrite_attr(&with_fill, "stroke", SvgPaintKind::Stroke, overrides);
    let rewritten = rewrite_style_attr(&with_stroke, overrides);
    if let Some(default_fill) = default_fill
        && tag_name(&rewritten) == Some("path")
    {
        let end = rewritten.len() - if rewritten.ends_with("/>") { 2 } else { 1 };
        return format!(
            "{} fill=\"{}\"{}",
            &rewritten[..end],
            svg_color_string(default_fill),
            &rewritten[end..]
        );
    }
    rewritten
}

fn uses_default_fill(svg: &str) -> bool {
    find_attr(svg, "fill").is_none() && !svg.contains("fill:") && svg.contains("<path")
}

fn tag_name(tag: &str) -> Option<&str> {
    let tag = tag.trim_start().strip_prefix('<')?.trim_start();
    let tag = tag.strip_prefix('/').unwrap_or(tag);
    if tag.starts_with(['!', '?']) {
        return None;
    }
    tag.split(|c: char| c.is_ascii_whitespace() || matches!(c, '/' | '>'))
        .next()
}

fn rewrite_attr(
    tag: &str,
    attr: &'static str,
    kind: SvgPaintKind,
    overrides: &[SvgColorOverride],
) -> String {
    let mut output = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some((prefix_len, value, next)) = find_attr(rest, attr) {
        output.push_str(&rest[..prefix_len]);
        let quote = rest[prefix_len..].chars().next().unwrap_or('"');
        output.push(quote);
        if let Some(color) =
            parse_svg_color(value).and_then(|color| override_color(overrides, kind, color))
        {
            output.push_str(&svg_color_string(color));
        } else {
            output.push_str(value);
        }
        output.push(quote);
        rest = next;
    }
    output.push_str(rest);
    output
}

fn rewrite_style_attr(tag: &str, overrides: &[SvgColorOverride]) -> String {
    let mut output = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some((prefix_len, value, next)) = find_attr(rest, "style") {
        output.push_str(&rest[..prefix_len]);
        let quote = rest[prefix_len..].chars().next().unwrap_or('"');
        output.push(quote);
        output.push_str(&rewrite_style(value, overrides));
        output.push(quote);
        rest = next;
    }
    output.push_str(rest);
    output
}

fn rewrite_style(style: &str, overrides: &[SvgColorOverride]) -> String {
    let mut output = String::with_capacity(style.len());
    for (index, declaration) in style.split(';').enumerate() {
        if index > 0 {
            output.push(';');
        }
        let Some((name, value)) = declaration.split_once(':') else {
            output.push_str(declaration);
            continue;
        };
        let kind = match name.trim() {
            "fill" => SvgPaintKind::Fill,
            "stroke" => SvgPaintKind::Stroke,
            _ => {
                output.push_str(declaration);
                continue;
            }
        };
        output.push_str(name);
        output.push(':');
        if let Some(color) =
            parse_svg_color(value).and_then(|color| override_color(overrides, kind, color))
        {
            output.push_str(&svg_color_string(color));
        } else {
            output.push_str(value);
        }
    }
    output
}

fn find_attr<'a>(input: &'a str, attr: &str) -> Option<(usize, &'a str, &'a str)> {
    let mut rest = input;
    let mut offset = 0;
    loop {
        let index = rest.find(attr)?;
        let before = rest[..index].chars().next_back();
        let after = rest[index + attr.len()..].chars().next();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            || after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            offset += index + attr.len();
            rest = &rest[index + attr.len()..];
            continue;
        }

        let tail = &rest[index + attr.len()..];
        let tail = tail.trim_start();
        let skipped_before_equals = rest[index + attr.len()..].len() - tail.len();
        if !tail.starts_with('=') {
            offset += index + attr.len();
            rest = &rest[index + attr.len()..];
            continue;
        }
        let value = tail[1..].trim_start();
        let skipped_after_equals = tail[1..].len() - value.len();
        let quote = value.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let value = &value[quote.len_utf8()..];
        let end = value.find(quote)?;
        let quote_index =
            offset + index + attr.len() + skipped_before_equals + 1 + skipped_after_equals;
        let value_start = quote_index + quote.len_utf8();
        let value_end = value_start + end;
        let next_start = value_end + quote.len_utf8();
        return Some((
            quote_index,
            &input[value_start..value_end],
            &input[next_start..],
        ));
    }
}

fn push_unique(colors: &mut Vec<SvgPaintColor>, color: SvgPaintColor) {
    if colors.len() >= SVG_COLOR_LIMIT || colors.contains(&color) {
        return;
    }
    colors.push(color);
}

fn override_color(
    overrides: &[SvgColorOverride],
    kind: SvgPaintKind,
    original: Color<u8>,
) -> Option<Color<u8>> {
    overrides
        .iter()
        .find(|override_color| override_color.kind == kind && override_color.original == original)
        .map(|override_color| override_color.replacement)
}

fn parse_svg_color(value: &str) -> Option<Color<u8>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("currentColor")
        || value.starts_with("url(")
    {
        return None;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    parse_rgb_function(value)
}

fn parse_hex_color(hex: &str) -> Option<Color<u8>> {
    match hex.len() {
        3 => Some(Color::<u8>::from_rgb(
            repeated_hex(hex, 0)?,
            repeated_hex(hex, 1)?,
            repeated_hex(hex, 2)?,
        )),
        4 => Some(Color::<u8>::from_rgba(
            repeated_hex(hex, 0)?,
            repeated_hex(hex, 1)?,
            repeated_hex(hex, 2)?,
            repeated_hex(hex, 3)?,
        )),
        6 => Some(Color::<u8>::from_rgb(
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        )),
        8 => Some(Color::<u8>::from_rgba(
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        )),
        _ => None,
    }
}

fn repeated_hex(hex: &str, index: usize) -> Option<u8> {
    let digit = hex.as_bytes().get(index).copied()? as char;
    let value = digit.to_digit(16)? as u8;
    Some(value * 17)
}

fn parse_rgb_function(value: &str) -> Option<Color<u8>> {
    let (alpha, body) = if let Some(body) = value
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
    {
        (true, body)
    } else {
        (
            false,
            value
                .strip_prefix("rgb(")
                .and_then(|value| value.strip_suffix(')'))?,
        )
    };
    let parts: Vec<&str> = body.split(',').map(str::trim).collect();
    if parts.len() != if alpha { 4 } else { 3 } {
        return None;
    }
    Some(Color::<u8>::from_rgba(
        parse_color_channel(parts[0])?,
        parse_color_channel(parts[1])?,
        parse_color_channel(parts[2])?,
        if alpha {
            parse_alpha_channel(parts[3])?
        } else {
            u8::MAX
        },
    ))
}

fn parse_color_channel(value: &str) -> Option<u8> {
    if let Some(percent) = value.strip_suffix('%') {
        let percent: f32 = percent.parse().ok()?;
        return Some((percent.clamp(0.0, 100.0) * 2.55).round() as u8);
    }
    let value: f32 = value.parse().ok()?;
    Some(value.round().clamp(0.0, 255.0) as u8)
}

fn parse_alpha_channel(value: &str) -> Option<u8> {
    if let Some(percent) = value.strip_suffix('%') {
        let percent: f32 = percent.parse().ok()?;
        return Some((percent.clamp(0.0, 100.0) * 2.55).round() as u8);
    }
    let value: f32 = value.parse().ok()?;
    Some((value.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn svg_color_string(color: Color<u8>) -> String {
    if color.a == 255 {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.r, color.g, color.b, color.a
        )
    }
}
