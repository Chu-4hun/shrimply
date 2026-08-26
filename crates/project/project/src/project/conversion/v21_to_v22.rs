use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 21;
const TARGET_VERSION: u32 = 22;

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
    let strokes = paint
        .remove(&key("strokes"))
        .unwrap_or_else(|| Value::Seq(Vec::new()));
    let fills = paint
        .remove(&key("fills"))
        .unwrap_or_else(|| Value::Seq(Vec::new()));
    let drawing = Value::Map(BTreeMap::from([
        (key("strokes"), strokes),
        (key("fills"), fills),
    ]));
    paint.insert(
        key("drawing"),
        Value::Map(BTreeMap::from([(
            key("base"),
            Value::Map(BTreeMap::from([(key("const"), drawing)])),
        )])),
    );
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
