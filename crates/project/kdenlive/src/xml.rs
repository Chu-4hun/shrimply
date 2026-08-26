use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Debug)]
pub struct Element {
    pub name: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children.iter().filter(move |child| child.name == name)
    }

    pub fn property(&self, name: &str) -> Option<&str> {
        self.children_named("property")
            .find(|property| property.attribute("name") == Some(name))
            .map(|property| property.text.as_str())
    }
}

pub fn parse(path: &Path) -> Result<Element, String> {
    let file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack = Vec::new();
    let mut root = None;
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?
        {
            Event::Start(start) => stack.push(element(&start)?),
            Event::Empty(start) => {
                let child = element(&start)?;
                stack
                    .last_mut()
                    .ok_or_else(|| "XML element appears outside the document root".to_string())?
                    .children
                    .push(child);
            }
            Event::Text(value) => {
                let content = value.xml_content(quick_xml::XmlVersion::Implicit1_0);
                let text =
                    quick_xml::escape::unescape(&content).map_err(|error| error.to_string())?;
                stack
                    .last_mut()
                    .ok_or_else(|| "XML text appears outside the document root".to_string())?
                    .text
                    .push_str(&text);
            }
            Event::CData(value) => {
                stack
                    .last_mut()
                    .ok_or_else(|| "XML text appears outside the document root".to_string())?
                    .text
                    .push_str(&value.xml_content(quick_xml::XmlVersion::Implicit1_0));
            }
            Event::End(_) => {
                let element = stack
                    .pop()
                    .ok_or_else(|| "XML has an unmatched closing element".to_string())?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(element);
                } else if root.replace(element).is_some() {
                    return Err("XML contains more than one root element".to_string());
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if !stack.is_empty() {
        return Err("XML ended before all elements were closed".to_string());
    }
    root.ok_or_else(|| "XML document is empty".to_string())
}

fn element(start: &quick_xml::events::BytesStart<'_>) -> Result<Element, String> {
    let name = start.name().as_ref().to_owned();
    let attributes = start
        .attributes()
        .map(|attribute| {
            let attribute = attribute.map_err(|error| error.to_string())?;
            let name = attribute.key.as_ref().to_owned();
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| error.to_string())?
                .into_owned();
            Ok((name, value))
        })
        .collect::<Result<_, String>>()?;
    Ok(Element {
        name,
        attributes,
        children: Vec::new(),
        text: String::new(),
    })
}
