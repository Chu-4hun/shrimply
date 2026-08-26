use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 26;
const TARGET_VERSION: u32 = 27;

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
        migrate_reverb_modes(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_reverb_modes(value: &mut Value) {
    match value {
        Value::Map(map) => {
            let kind = Value::String("kind".to_string());
            let config = Value::String("config".to_string());
            let mode = Value::String("mode".to_string());
            if map.get(&kind) == Some(&Value::String("reverb".to_string()))
                && let Some(Value::Map(config)) = map.get_mut(&config)
            {
                config
                    .entry(mode)
                    .or_insert_with(|| Value::String("classic".to_string()));
            }
            for value in map.values_mut() {
                migrate_reverb_modes(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_reverb_modes(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_reverb_modes(value),
        _ => {}
    }
}
