use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 15;
const TARGET_VERSION: u32 = 16;

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
        migrate_grid_padding(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_grid_padding(value: &mut Value) {
    match value {
        Value::Map(map) => {
            if let Some(Value::Map(generator)) = map.get_mut(&key("generator")) {
                migrate_generator(generator);
            }
            for value in map.values_mut() {
                migrate_grid_padding(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_grid_padding(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_grid_padding(value),
        _ => {}
    }
}

fn migrate_generator(generator: &mut BTreeMap<Value, Value>) {
    if generator.get(&key("kind")) != Some(&Value::String("grid".to_string())) {
        return;
    }
    let enabled = generator
        .get(&key("middle_gap"))
        .is_some_and(timeline_const_true);
    let padding = generator.remove(&key("gap_min_size"));
    for field in ["middle_gap", "gap_max_size", "gap_interval", "gap_curve"] {
        generator.remove(&key(field));
    }
    if enabled
        && !generator.contains_key(&key("middle_padding"))
        && let Some(padding) = padding
    {
        generator.insert(key("middle_padding"), padding);
    }
}

fn timeline_const_true(value: &Value) -> bool {
    if value == &Value::Bool(true) || value == &Value::String("true".to_string()) {
        return true;
    }
    let Value::Map(value) = value else {
        return false;
    };
    let Some(Value::Map(base)) = value.get(&key("base")) else {
        return false;
    };
    match base.get(&key("const")) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value == "true",
        _ => false,
    }
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
