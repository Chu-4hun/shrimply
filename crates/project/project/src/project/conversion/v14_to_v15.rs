use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 14;
const TARGET_VERSION: u32 = 15;
const NANOS_PER_SECOND: f32 = 1_000_000_000.0;

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
        migrate_backgrounds(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_backgrounds(value: &mut Value) {
    match value {
        Value::Map(map) => {
            if let Some(generator) = map.get_mut(&key("generator")) {
                migrate_generator(generator);
            }
            for value in map.values_mut() {
                migrate_backgrounds(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_backgrounds(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_backgrounds(value),
        _ => {}
    }
}

fn migrate_generator(value: &mut Value) {
    let Value::Map(generator) = value else {
        return;
    };
    let Some(Value::String(kind)) = generator.get(&key("kind")) else {
        return;
    };
    let kind = kind.clone();
    match kind.as_str() {
        "color_gradient" => {
            rename_field(generator, "drift", "position");
            rename_field(generator, "cycle_speed", "cycle_position");
        }
        "grid" => {
            rename_field(generator, "speed", "position");
            rename_field(generator, "dash_speed", "dash_position");
            rename_field(generator, "wobble_speed", "wobble_position");
        }
        "perlin_noise" => {
            rename_field(generator, "drift", "position");
            rename_field(generator, "evolution_speed", "evolution");
        }
        "rainbow" => {
            rename_field(generator, "drift", "position");
            rename_field(generator, "hue_speed", "hue_position");
        }
        "checkerboard" => rename_field(generator, "speed", "position"),
        "voronoi" => {
            rename_field(generator, "drift", "position");
            rename_field(generator, "motion_speed", "motion_position");
        }
        _ => {}
    }
    let fields: &[&str] = match kind.as_str() {
        "color_gradient" => &[
            "color_a",
            "color_b",
            "center",
            "angle_degrees",
            "scale",
            "position",
            "cycle_position",
        ],
        "grid" => &[
            "background_color",
            "horizontal_color",
            "vertical_color",
            "spacing",
            "line_width",
            "position",
            "rotation_degrees",
            "dash_length",
            "dash_gap",
            "dash_position",
            "wobble_amount",
            "wobble_scale",
            "wobble_position",
            "middle_gap",
            "gap_min_size",
            "gap_max_size",
            "gap_interval",
            "seed",
        ],
        "white_noise" => &[
            "color_a",
            "color_b",
            "pixel_size",
            "brightness",
            "contrast",
            "animated",
            "refresh_interval",
            "seed",
        ],
        "perlin_noise" => &[
            "color_a",
            "color_b",
            "scale",
            "octaves",
            "lacunarity",
            "persistence",
            "contrast",
            "position",
            "evolution",
            "warp_amount",
            "warp_scale",
            "seed",
        ],
        "rainbow" => &[
            "band_count",
            "center",
            "angle_degrees",
            "scale",
            "saturation",
            "brightness",
            "alpha",
            "position",
            "hue_position",
        ],
        "checkerboard" => &[
            "color_a",
            "color_b",
            "cell_size",
            "edge_softness",
            "position",
            "rotation_degrees",
        ],
        "voronoi" => &[
            "color_a",
            "color_b",
            "edge_color",
            "cell_size",
            "jitter",
            "edge_width",
            "position",
            "motion_amount",
            "motion_position",
            "seed",
        ],
        _ => return,
    };

    for field in ["middle_gap", "animated"] {
        normalize_bool(generator, field);
    }
    for field in ["gap_interval", "refresh_interval"] {
        normalize_duration(generator, field);
    }
    for field in fields {
        wrap_timeline_value(generator, field);
    }
}

fn rename_field(map: &mut BTreeMap<Value, Value>, old: &str, new: &str) {
    if !map.contains_key(&key(new))
        && let Some(value) = map.remove(&key(old))
    {
        map.insert(key(new), value);
    }
}

fn normalize_bool(map: &mut BTreeMap<Value, Value>, field: &str) {
    let Some(value) = map.get_mut(&key(field)) else {
        return;
    };
    let Value::Bool(boolean) = value else {
        return;
    };
    *value = Value::String(if *boolean { "true" } else { "false" }.to_string());
}

fn normalize_duration(map: &mut BTreeMap<Value, Value>, field: &str) {
    let Some(value) = map.get_mut(&key(field)) else {
        return;
    };
    let Value::Map(duration) = value else {
        return;
    };
    if duration.contains_key(&key("base")) {
        return;
    }
    let Some(seconds) = duration.get(&key("secs")).and_then(number_as_f32) else {
        return;
    };
    let nanos = duration
        .get(&key("nanos"))
        .and_then(number_as_f32)
        .unwrap_or(0.0);
    *value = Value::F32(seconds + nanos / NANOS_PER_SECOND);
}

fn wrap_timeline_value(map: &mut BTreeMap<Value, Value>, field: &str) {
    let Some(value) = map.get_mut(&key(field)) else {
        return;
    };
    if matches!(value, Value::Map(map) if map.contains_key(&key("base"))) {
        return;
    }
    let constant = std::mem::replace(value, Value::Unit);
    *value = Value::Map(BTreeMap::from([(
        key("base"),
        Value::Map(BTreeMap::from([(key("const"), constant)])),
    )]));
}

fn number_as_f32(value: &Value) -> Option<f32> {
    match value {
        Value::U8(value) => Some(f32::from(*value)),
        Value::U16(value) => Some(f32::from(*value)),
        Value::U32(value) => Some(*value as f32),
        Value::U64(value) => Some(*value as f32),
        Value::I8(value) => Some(f32::from(*value)),
        Value::I16(value) => Some(f32::from(*value)),
        Value::I32(value) => Some(*value as f32),
        Value::I64(value) => Some(*value as f32),
        Value::F32(value) => Some(*value),
        Value::F64(value) => Some(*value as f32),
        _ => None,
    }
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
