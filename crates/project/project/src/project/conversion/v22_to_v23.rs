use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 22;
const TARGET_VERSION: u32 = 23;

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
        migrate_gaussian_blur(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_gaussian_blur(value: &mut Value) {
    match value {
        Value::Map(map) => {
            if map.get(&key("kind")) == Some(&Value::String("gaussian_blur".to_string())) {
                migrate_config(map);
            }
            for value in map.values_mut() {
                migrate_gaussian_blur(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_gaussian_blur(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_gaussian_blur(value),
        _ => {}
    }
}

fn migrate_config(effect: &mut BTreeMap<Value, Value>) {
    let Some(Value::Map(config)) = effect.get_mut(&key("config")) else {
        return;
    };
    let Some(radius) = config.get_mut(&key("radius")) else {
        return;
    };
    scalar_timeline_to_vector(radius);
}

fn scalar_timeline_to_vector(value: &mut Value) {
    let Value::Map(timeline) = value else {
        duplicate_scalar(value);
        return;
    };
    let Some(Value::Map(base)) = timeline.get_mut(&key("base")) else {
        return;
    };
    if let Some(value) = base.get_mut(&key("const")) {
        duplicate_scalar(value);
    }
    let Some(Value::Seq(keyframes)) = base.get_mut(&key("keyframes")) else {
        return;
    };
    for keyframe in keyframes {
        let Value::Map(keyframe) = keyframe else {
            continue;
        };
        if let Some(value) = keyframe.get_mut(&key("value")) {
            duplicate_scalar(value);
        }
    }
}

fn duplicate_scalar(value: &mut Value) {
    if matches!(
        value,
        Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::F32(_)
            | Value::F64(_)
    ) {
        let scalar = value.clone();
        *value = Value::Seq(vec![scalar.clone(), scalar]);
    }
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
