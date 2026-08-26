use std::collections::BTreeMap;

use serde_value::Value;
use shrimply_paint_model::DEFAULT_STROKE_WIDTH_SCALE;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 19;
const TARGET_VERSION: u32 = 20;

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
    let stroke_color = map_field(paint, "stroke").and_then(|stroke| stroke.remove(&key("color")));
    let fill_color = map_field(paint, "fill").and_then(|fill| fill.remove(&key("color")));
    if let (Some(stroke_color), Some(fill_color)) = (stroke_color, fill_color) {
        paint.insert(key("palette"), Value::Seq(vec![stroke_color, fill_color]));
    }
    if let Some(strokes) = sequence_field(paint, "strokes") {
        for stroke in strokes {
            if let Some(stroke) = value_map(stroke) {
                stroke
                    .entry(key("width_scale"))
                    .or_insert(Value::F32(DEFAULT_STROKE_WIDTH_SCALE));
                stroke.entry(key("color_index")).or_insert(Value::U64(0));
            }
        }
    }
    if let Some(fills) = sequence_field(paint, "fills") {
        for fill in fills {
            if let Some(fill) = value_map(fill) {
                fill.entry(key("color_index")).or_insert(Value::U64(1));
            }
        }
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
