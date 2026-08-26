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
