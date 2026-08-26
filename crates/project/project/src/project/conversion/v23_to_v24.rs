use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 23;
const TARGET_VERSION: u32 = 24;

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
        migrate_visual_clip_transitions(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_visual_clip_transitions(value: &mut Value) {
    match value {
        Value::Map(map) => {
            if is_visual_clip_transition(map) {
                migrate_visual_clip_transition(map);
            }
            for value in map.values_mut() {
                migrate_visual_clip_transitions(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_visual_clip_transitions(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => {
            migrate_visual_clip_transitions(value)
        }
        _ => {}
    }
}

fn is_visual_clip_transition(map: &BTreeMap<Value, Value>) -> bool {
    map.contains_key(&key("target_item_id"))
        && map.contains_key(&key("duration"))
        && matches!(
            map.get(&key("kind")),
            Some(Value::String(kind))
                if matches!(
                    kind.as_str(),
                    "cross_fade" | "fade_through_white" | "wipe" | "morph"
                )
        )
}

fn migrate_visual_clip_transition(map: &mut BTreeMap<Value, Value>) {
    let kind = match map.get_mut(&key("kind")) {
        Some(Value::String(kind)) => {
            if kind == "fade_through_white" {
                *kind = "fade_through_color".to_string();
            }
            kind.clone()
        }
        _ => return,
    };
    map.insert(
        key("interpolation"),
        Value::String(
            if kind == "morph" {
                "manim_smooth"
            } else {
                "linear"
            }
            .to_string(),
        ),
    );
    map.insert(key("direction_degrees"), Value::F32(0.0));
    map.insert(key("softness"), Value::F32(0.05));
    map.insert(
        key("center"),
        Value::Seq(vec![Value::F32(0.5), Value::F32(0.5)]),
    );
    map.insert(key("iris_from_inside"), Value::Bool(true));
    map.insert(key("clockwise"), Value::Bool(true));
    map.insert(
        key("fade_color"),
        Value::Map(BTreeMap::from([
            (key("r"), Value::U8(u8::MAX)),
            (key("g"), Value::U8(u8::MAX)),
            (key("b"), Value::U8(u8::MAX)),
            (key("a"), Value::U8(u8::MAX)),
        ])),
    );
    map.insert(key("dissolve_grain_size"), Value::U32(4));
    map.insert(key("zoom_start_scale"), Value::F32(0.0));
    map.insert(key("fade_opacity"), Value::Bool(false));
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
