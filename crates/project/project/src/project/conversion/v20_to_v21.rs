use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 20;
const TARGET_VERSION: u32 = 21;

pub(super) struct Converter;

impl ProjectVersionConverter for Converter {
    fn source_version(&self) -> u32 {
        SOURCE_VERSION
    }

    fn target_version(&self) -> u32 {
        TARGET_VERSION
    }

    fn convert(&self, mut project: Value) -> Result<Value, String> {
        ensure_project_version(&project, SOURCE_VERSION)?;
        migrate_paint_items(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_paint_items(value: &mut Value) {
    match value {
        Value::Map(map) => {
            if map.get(&key("kind")) == Some(&Value::String("paint".to_string())) {
                migrate_paint(map);
            }
            for value in map.values_mut() {
                migrate_paint_items(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_paint_items(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_paint_items(value),
        _ => {}
    }
}

fn migrate_paint(paint: &mut BTreeMap<Value, Value>) {
    let stroke_texture = take_texture(paint, "stroke");
    let fill_texture = take_texture(paint, "fill");
    let Some(Value::Seq(colors)) = paint.remove(&key("palette")) else {
        return;
    };
    let color_count = colors.len();
    let separate_fill_textures = stroke_texture != fill_texture;
    let mut entries: Vec<_> = colors
        .iter()
        .cloned()
        .map(|color| texture_entry(color, stroke_texture.clone()))
        .collect();
    if separate_fill_textures {
        entries.extend(
            colors
                .into_iter()
                .map(|color| texture_entry(color, fill_texture.clone())),
        );
        if let Some(fills) = sequence_field(paint, "fills") {
            for fill in fills {
                let Some(fill) = value_map(fill) else {
                    continue;
                };
                let Some(index) = fill.remove(&key("color_index")) else {
                    continue;
                };
                fill.insert(key("color_index"), add_index(index, color_count));
            }
        }
    }
    paint.insert(key("palette"), Value::Seq(entries));
}

fn take_texture(paint: &mut BTreeMap<Value, Value>, field: &str) -> Value {
    map_field(paint, field)
        .and_then(|options| options.remove(&key("texture")))
        .unwrap_or(Value::Option(None))
}

fn texture_entry(color: Value, texture: Value) -> Value {
    Value::Map(BTreeMap::from([
        (key("color"), color),
        (key("texture"), texture),
    ]))
}

fn add_index(index: Value, offset: usize) -> Value {
    match index {
        Value::U8(value) => Value::U64(value as u64 + offset as u64),
        Value::U16(value) => Value::U64(value as u64 + offset as u64),
        Value::U32(value) => Value::U64(value as u64 + offset as u64),
        Value::U64(value) => Value::U64(value + offset as u64),
        value => value,
    }
}

fn map_field<'a>(
    map: &'a mut BTreeMap<Value, Value>,
    field: &str,
) -> Option<&'a mut BTreeMap<Value, Value>> {
    value_map(map.get_mut(&key(field))?)
}

fn sequence_field<'a>(
    map: &'a mut BTreeMap<Value, Value>,
    field: &str,
) -> Option<&'a mut Vec<Value>> {
    value_sequence(map.get_mut(&key(field))?)
}

fn value_map(value: &mut Value) -> Option<&mut BTreeMap<Value, Value>> {
    match value {
        Value::Map(map) => Some(map),
        Value::Option(Some(value)) | Value::Newtype(value) => value_map(value),
        _ => None,
    }
}

fn value_sequence(value: &mut Value) -> Option<&mut Vec<Value>> {
    match value {
        Value::Seq(values) => Some(values),
        Value::Option(Some(value)) | Value::Newtype(value) => value_sequence(value),
        _ => None,
    }
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
