use pest::Parser as _;
use pest::iterators::Pair;
use pest_derive::Parser;

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

enum SyntaxNode {
    Text(String),
    Styled { marker: Marker, children: Vec<Self> },
    Karaoke(u32),
    Ruby(Ruby),
}

#[derive(Parser)]
#[grammar = "caption/markup.pest"]
struct CaptionParser;

pub fn parse(source: &str) -> Vec<Span> {
    let caption = CaptionParser::parse(Rule::caption, source)
        .expect("the caption grammar must accept all input")
        .next()
        .expect("a successful caption parse must have a root node");
    let nodes = syntax_nodes(caption);
    let mut spans = Vec::new();
    flatten(&nodes, &mut spans, Style::default(), &mut 0);
    spans
}

fn syntax_nodes(pair: Pair<'_, Rule>) -> Vec<SyntaxNode> {
    pair.into_inner().filter_map(syntax_node).collect()
}

fn syntax_node(pair: Pair<'_, Rule>) -> Option<SyntaxNode> {
    match pair.as_rule() {
        Rule::bold => Some(SyntaxNode::Styled {
            marker: Marker::Bold,
            children: syntax_nodes(pair),
        }),
        Rule::underline => Some(SyntaxNode::Styled {
            marker: Marker::Underline,
            children: syntax_nodes(pair),
        }),
        Rule::italic => Some(SyntaxNode::Styled {
            marker: Marker::Italic,
            children: syntax_nodes(pair),
        }),
        Rule::karaoke => {
            let source = pair.as_str();
            Some(source[1..source.len() - 1].parse().map_or_else(
                |_| SyntaxNode::Text(source.to_string()),
                SyntaxNode::Karaoke,
            ))
        }
        Rule::ruby => {
            let mut parts = pair.into_inner();
            Some(SyntaxNode::Ruby(Ruby {
                base: parts
                    .next()
                    .expect("ruby syntax must contain a base node")
                    .as_str()
                    .to_string(),
                annotation: parts
                    .next()
                    .expect("ruby syntax must contain an annotation node")
                    .as_str()
                    .to_string(),
            }))
        }
        Rule::escaped => Some(SyntaxNode::Text(pair.as_str()[1..].to_string())),
        Rule::text | Rule::character => Some(SyntaxNode::Text(pair.as_str().to_string())),
        Rule::EOI => None,
        Rule::caption
        | Rule::bold_content
        | Rule::italic_content
        | Rule::italic_close
        | Rule::underline_content
        | Rule::ruby_base
        | Rule::ruby_annotation
        | Rule::node => unreachable!("silent grammar rules cannot produce syntax-tree pairs"),
    }
}

#[derive(Clone, Copy, Default)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
}

fn flatten(nodes: &[SyntaxNode], spans: &mut Vec<Span>, style: Style, start_millis: &mut u32) {
    for node in nodes {
        match node {
            SyntaxNode::Text(text) => push(spans, text.clone(), style, *start_millis, None),
            SyntaxNode::Styled { marker, children } => {
                let mut nested = style;
                match marker {
                    Marker::Bold => nested.bold = true,
                    Marker::Underline => nested.underline = true,
                    Marker::Italic => nested.italic = true,
                }
                flatten(children, spans, nested, start_millis);
            }
            SyntaxNode::Karaoke(millis) => *start_millis = *millis,
            SyntaxNode::Ruby(ruby) => {
                push(
                    spans,
                    String::new(),
                    style,
                    *start_millis,
                    Some(ruby.clone()),
                );
            }
        }
    }
}

pub fn plain_text(source: &str) -> String {
    plain_text_from_spans(&parse(source))
}

pub fn plain_text_from_spans(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|span| {
            span.ruby
                .as_ref()
                .map_or(span.text.as_str(), |ruby| ruby.base.as_str())
        })
        .collect()
}

pub fn visible_spans(source: &str, millis: u32) -> Vec<Span> {
    parse(source)
        .into_iter()
        .filter(|span| span.start_millis <= millis)
        .collect()
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
                for character in span.text.chars() {
                    if matches!(character, '\\' | '*' | '_' | '{' | '[') {
                        output.push('\\');
                    }
                    output.push(character);
                }
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

fn push(spans: &mut Vec<Span>, text: String, style: Style, start_millis: u32, ruby: Option<Ruby>) {
    if text.is_empty() && ruby.is_none() {
        return;
    }
    if ruby.is_none()
        && let Some(previous) = spans.last_mut()
        && previous.ruby.is_none()
        && previous.bold == style.bold
        && previous.italic == style.italic
        && previous.underline == style.underline
        && previous.start_millis == start_millis
    {
        previous.text.push_str(&text);
        return;
    }
    spans.push(Span {
        text,
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        start_millis,
        ruby,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_caption_markup() {
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
