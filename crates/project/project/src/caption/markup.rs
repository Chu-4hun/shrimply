#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub start_millis: u32,
    pub ruby: Option<Ruby>,
}

#[derive(Clone, Debug)]
pub struct Ruby {
    pub base: String,
    pub annotation: String,
}

#[derive(Clone, Copy)]
enum Marker {
    Bold,
    Underline,
    Italic,
}

pub fn parse(source: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut text = String::new();
    let (mut bold, mut italic, mut underline, mut start_millis) = (false, false, false, 0);
    let mut rest = source;
    while !rest.is_empty() {
        let marker = [
            ("**", Marker::Bold),
            ("__", Marker::Underline),
            ("*", Marker::Italic),
        ]
        .into_iter()
        .find(|(token, _)| rest.starts_with(token));
        if let Some((token, marker)) = marker {
            push(
                &mut spans,
                &mut text,
                bold,
                italic,
                underline,
                start_millis,
                None,
            );
            match marker {
                Marker::Bold => bold = !bold,
                Marker::Underline => underline = !underline,
                Marker::Italic => italic = !italic,
            }
            rest = &rest[token.len()..];
            continue;
        }
        if let Some(after) = rest.strip_prefix('{')
            && let Some((value, tail)) = after.split_once('}')
            && let Ok(millis) = value.parse::<u32>()
        {
            push(
                &mut spans,
                &mut text,
                bold,
                italic,
                underline,
                start_millis,
                None,
            );
            start_millis = millis;
            rest = tail;
            continue;
        }
        if let Some(after) = rest.strip_prefix('[')
            && let Some((value, tail)) = after.split_once(']')
            && let Some((base, annotation)) = value.split_once('/')
        {
            push(
                &mut spans,
                &mut text,
                bold,
                italic,
                underline,
                start_millis,
                None,
            );
            push(
                &mut spans,
                &mut String::new(),
                bold,
                italic,
                underline,
                start_millis,
                Some(Ruby {
                    base: base.to_string(),
                    annotation: annotation.to_string(),
                }),
            );
            rest = tail;
            continue;
        }
        let character = rest.chars().next().unwrap();
        text.push(character);
        rest = &rest[character.len_utf8()..];
    }
    push(
        &mut spans,
        &mut text,
        bold,
        italic,
        underline,
        start_millis,
        None,
    );
    spans
}

pub fn plain_text(source: &str) -> String {
    parse(source)
        .into_iter()
        .map(|span| span.ruby.map_or(span.text, |ruby| ruby.base))
        .collect()
}

pub fn visible_text(source: &str, millis: u32) -> String {
    let mut output = String::new();
    for span in parse(source)
        .into_iter()
        .filter(|span| span.start_millis <= millis)
    {
        if span.bold {
            output.push_str("**");
        }
        if span.italic {
            output.push('*');
        }
        if span.underline {
            output.push_str("__");
        }
        if let Some(ruby) = span.ruby {
            output.push_str(&ruby.base);
            output.push('(');
            output.push_str(&ruby.annotation);
            output.push(')');
        } else {
            output.push_str(&span.text);
        }
        if span.underline {
            output.push_str("__");
        }
        if span.italic {
            output.push('*');
        }
        if span.bold {
            output.push_str("**");
        }
    }
    output
}

pub fn plain_text_byte_at_visible_byte(
    source: &str,
    millis: u32,
    visible_byte: usize,
) -> Option<usize> {
    let mut source_byte = 0;
    let mut rendered_byte = 0;
    for span in parse(source) {
        let source_len = span
            .ruby
            .as_ref()
            .map_or(span.text.len(), |ruby| ruby.base.len());
        if span.start_millis <= millis {
            let rendered_len = span.ruby.as_ref().map_or(span.text.len(), |ruby| {
                ruby.base.len() + ruby.annotation.len() + 2
            });
            if visible_byte <= rendered_byte + rendered_len {
                let local = visible_byte.saturating_sub(rendered_byte);
                let local = span
                    .ruby
                    .as_ref()
                    .map_or(local, |ruby| local.min(ruby.base.len()));
                return Some(source_byte + local.min(source_len));
            }
            rendered_byte += rendered_len;
        }
        source_byte += source_len;
    }
    None
}

pub fn split_at_plain_text_byte(source: &str, byte: usize) -> Option<(String, String)> {
    let spans = parse(source);
    let plain_len = spans
        .iter()
        .map(|span| {
            span.ruby
                .as_ref()
                .map_or(span.text.len(), |ruby| ruby.base.len())
        })
        .sum::<usize>();
    if byte == 0 || byte >= plain_len {
        return None;
    }

    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut offset = 0;
    for span in spans {
        let text = span
            .ruby
            .as_ref()
            .map_or(span.text.as_str(), |ruby| ruby.base.as_str());
        let end = offset + text.len();
        if byte <= offset {
            right.push(span);
        } else if byte >= end {
            left.push(span);
        } else {
            let split = byte - offset;
            if !text.is_char_boundary(split) {
                return None;
            }
            let left_text = text[..split].to_string();
            let right_text = text[split..].to_string();
            let mut left_span = span.clone();
            left_span.text = left_text;
            left_span.ruby = None;
            let mut right_span = span;
            right_span.text = right_text;
            right_span.ruby = None;
            left.push(left_span);
            right.push(right_span);
        }
        offset = end;
    }

    let encode = |spans: Vec<Span>| {
        use std::fmt::Write;

        let mut output = String::new();
        for span in spans {
            if span.start_millis > 0 {
                write!(output, "{{{}}}", span.start_millis)
                    .expect("writing to a string cannot fail");
            }
            if span.bold {
                output.push_str("**");
            }
            if span.italic {
                output.push('*');
            }
            if span.underline {
                output.push_str("__");
            }
            if let Some(ruby) = span.ruby {
                write!(output, "[{}/{}]", ruby.base, ruby.annotation)
                    .expect("writing to a string cannot fail");
            } else {
                output.push_str(&span.text);
            }
            if span.underline {
                output.push_str("__");
            }
            if span.italic {
                output.push('*');
            }
            if span.bold {
                output.push_str("**");
            }
        }
        output
    };
    let left = encode(left);
    let right = encode(right);
    (!plain_text(&left).trim().is_empty() && !plain_text(&right).trim().is_empty())
        .then_some((left, right))
}

fn push(
    spans: &mut Vec<Span>,
    text: &mut String,
    bold: bool,
    italic: bool,
    underline: bool,
    start_millis: u32,
    ruby: Option<Ruby>,
) {
    if text.is_empty() && ruby.is_none() {
        return;
    }
    spans.push(Span {
        text: std::mem::take(text),
        bold,
        italic,
        underline,
        start_millis,
        ruby,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markdown_karaoke_and_ruby() {
        let spans = parse("**Bold** *italic* __under__ {1200}[漢/かん]字");
        assert!(spans.iter().any(|span| span.bold && span.text == "Bold"));
        assert!(
            spans
                .iter()
                .any(|span| span.italic && span.text == "italic")
        );
        assert!(
            spans
                .iter()
                .any(|span| span.underline && span.text == "under")
        );
        let ruby = spans.iter().find_map(|span| span.ruby.as_ref()).unwrap();
        assert_eq!(ruby.base, "漢");
        assert_eq!(ruby.annotation, "かん");
        assert_eq!(spans.last().unwrap().start_millis, 1200);
        assert_eq!(plain_text("[漢/かん]字"), "漢字");
        assert_eq!(
            crate::caption::clean_text_for_speech("**Hello** {500}*world*\n[漢/かん]字"),
            "Hello world かん字"
        );
    }
}
