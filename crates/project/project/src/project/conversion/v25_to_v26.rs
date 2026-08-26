use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 25;
const TARGET_VERSION: u32 = 26;
const OLD_FIELDS: [&str; 8] = [
    "model",
    "text",
    "language",
    "speaker",
    "instruction",
    "reference_audio",
    "reference_text",
    "audio_only",
];

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
        migrate_tts_settings(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_tts_settings(value: &mut Value) {
    match value {
        Value::Map(map) => {
            if OLD_FIELDS.iter().all(|field| map.contains_key(&key(field))) {
                migrate_tts_map(map);
            }
            for value in map.values_mut() {
                migrate_tts_settings(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_tts_settings(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_tts_settings(value),
        _ => {}
    }
}

fn migrate_tts_map(settings: &mut BTreeMap<Value, Value>) {
    let mut inputs = BTreeMap::new();
    insert_input(&mut inputs, "text", "text", take(settings, "text"));
    if let Some(value) = optional(take(settings, "language")) {
        insert_input(&mut inputs, "language", "select", value);
    }
    if let Some(value) = optional(take(settings, "speaker")) {
        insert_input(&mut inputs, "speaker", "select", value);
    }
    insert_input(
        &mut inputs,
        "instruction",
        "text",
        take(settings, "instruction"),
    );
    if let Some(value) = optional(take(settings, "reference_audio")) {
        insert_input(&mut inputs, "reference_audio", "audio", value);
    }
    insert_input(
        &mut inputs,
        "reference_text",
        "text",
        take(settings, "reference_text"),
    );
    insert_input(
        &mut inputs,
        "audio_only",
        "toggle",
        take(settings, "audio_only"),
    );
    settings.insert(key("inputs"), Value::Map(inputs));
}

fn insert_input(inputs: &mut BTreeMap<Value, Value>, input_key: &str, kind: &str, value: Value) {
    inputs.insert(
        key(input_key),
        Value::Map(BTreeMap::from([
            (key("kind"), Value::String(kind.to_string())),
            (key("value"), value),
        ])),
    );
}

fn take(settings: &mut BTreeMap<Value, Value>, field: &str) -> Value {
    settings
        .remove(&key(field))
        .expect("detected legacy TTS settings must contain every legacy field")
}

fn optional(value: Value) -> Option<Value> {
    match value {
        Value::Option(Some(value)) | Value::Newtype(value) => Some(*value),
        Value::Option(None) | Value::Unit => None,
        value => Some(value),
    }
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
