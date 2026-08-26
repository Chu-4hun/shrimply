use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 16;
const TARGET_VERSION: u32 = 17;
const FULL_ELLIPSE_DEGREES: f32 = 360.0;
const SEMICIRCLE_DEGREES: f32 = 180.0;
const FAN_DEGREES: f32 = 90.0;

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
        migrate_ellipse_shapes(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_ellipse_shapes(value: &mut Value) {
    match value {
        Value::Map(map) => {
            if map.get(&key("kind")) == Some(&Value::String("shape".to_string())) {
                migrate_shape(map);
            }
            for value in map.values_mut() {
                migrate_ellipse_shapes(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_ellipse_shapes(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_ellipse_shapes(value),
        _ => {}
    }
}

fn migrate_shape(shape: &mut BTreeMap<Value, Value>) {
    if let Some(inner_radius) = shape.remove(&key("fan_inner_radius_percent")) {
        shape
            .entry(key("ellipse_inner_radius_percent"))
            .or_insert(inner_radius);
    }
    let completion = match shape.get(&key("shape")) {
        Some(Value::String(kind)) if kind == "semicircle" => SEMICIRCLE_DEGREES,
        Some(Value::String(kind)) if kind == "fan" => FAN_DEGREES,
        Some(Value::String(kind)) if kind == "ellipse" => FULL_ELLIPSE_DEGREES,
        _ => return,
    };
    shape.insert(key("shape"), Value::String("ellipse".to_string()));
    shape
        .entry(key("ellipse_completion_degrees"))
        .or_insert_with(|| timeline_constant(completion));
}

fn timeline_constant(value: f32) -> Value {
    Value::Map(BTreeMap::from([(
        key("base"),
        Value::Map(BTreeMap::from([(key("const"), Value::F32(value))])),
    )]))
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
