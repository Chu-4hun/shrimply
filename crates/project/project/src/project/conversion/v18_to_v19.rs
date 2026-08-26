use std::collections::BTreeMap;

use serde_value::Value;
use shrimply_paint_model::DEFAULT_STROKE_WIDTH;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 18;
const TARGET_VERSION: u32 = 19;

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
    if let Some(stroke) = map_field(paint, "stroke") {
        for field in [
            "width",
            "thinning",
            "smoothing",
            "streamline",
            "simplification_tolerance",
            "maximum_subdivision_spacing",
        ] {
            wrap_timeline_field(stroke, field);
        }
        for field in ["start", "end"] {
            if let Some(end) = map_field(stroke, field) {
                migrate_stroke_end(end);
            }
        }
        if let Some(texture) = map_field(stroke, "texture") {
            migrate_texture(texture);
        }
    }
    if let Some(fill) = map_field(paint, "fill") {
        wrap_timeline_field(fill, "closure_tolerance");
        if let Some(texture) = map_field(fill, "texture") {
            migrate_texture(texture);
        }
    }
}

fn migrate_stroke_end(end: &mut BTreeMap<Value, Value>) {
    let cap_key = key("cap");
    if let Some(cap) = end.remove(&cap_key) {
        let cap = match cap {
            Value::Bool(cap) => Value::String(if cap { "true" } else { "false" }.to_string()),
            cap => cap,
        };
        end.insert(
            cap_key,
            if is_timeline_value(&cap) {
                cap
            } else {
                timeline_constant(cap)
            },
        );
    }

    let taper_key = key("taper");
    let taper = end.remove(&taper_key);
    let (taper, distance) = match taper {
        Some(Value::Map(mut taper)) if !taper.contains_key(&key("base")) => {
            let distance = taper
                .remove(&key("distance"))
                .unwrap_or(Value::F32(DEFAULT_STROKE_WIDTH));
            (Value::String("distance".to_string()), distance)
        }
        Some(taper) => (taper, Value::F32(DEFAULT_STROKE_WIDTH)),
        None => (
            Value::String("none".to_string()),
            Value::F32(DEFAULT_STROKE_WIDTH),
        ),
    };
    end.insert(
        taper_key,
        if is_timeline_value(&taper) {
            taper
        } else {
            timeline_constant(taper)
        },
    );
    end.entry(key("taper_distance"))
        .or_insert_with(|| timeline_constant(distance));
}

fn migrate_texture(texture: &mut BTreeMap<Value, Value>) {
    wrap_timeline_field(texture, "repeat_scale");
    wrap_timeline_field(texture, "rotation_degrees");
}

fn wrap_timeline_field(map: &mut BTreeMap<Value, Value>, field: &str) {
    let field = key(field);
    let Some(value) = map.remove(&field) else {
        return;
    };
    map.insert(
        field,
        if is_timeline_value(&value) {
            value
        } else {
            timeline_constant(value)
        },
    );
}

fn timeline_constant(value: Value) -> Value {
    Value::Map(BTreeMap::from([(
        key("base"),
        Value::Map(BTreeMap::from([(key("const"), value)])),
    )]))
}

fn is_timeline_value(value: &Value) -> bool {
    matches!(value, Value::Map(map) if map.contains_key(&key("base")))
}

fn map_field<'a>(
    map: &'a mut BTreeMap<Value, Value>,
    field: &str,
) -> Option<&'a mut BTreeMap<Value, Value>> {
    value_map(map.get_mut(&key(field))?)
}

fn value_map(value: &mut Value) -> Option<&mut BTreeMap<Value, Value>> {
    match value {
        Value::Map(map) => Some(map),
        Value::Option(Some(value)) | Value::Newtype(value) => value_map(value),
        _ => None,
    }
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
