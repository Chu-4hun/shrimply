use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 27;
const TARGET_VERSION: u32 = 28;

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
        migrate_crop_modes(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn migrate_crop_modes(value: &mut Value) {
    match value {
        Value::Map(map) => {
            let kind = Value::String("kind".to_string());
            let config = Value::String("config".to_string());
            let mode = Value::String("mode".to_string());
            let values = Value::String("values".to_string());
            if map.get(&kind) == Some(&Value::String("crop".to_string()))
                && let Some(Value::Map(crop)) = map.get_mut(&config)
                && !crop.contains_key(&values)
            {
                let mode_value = crop
                    .remove(&mode)
                    .unwrap_or_else(|| Value::String("percentage".to_string()));
                let edges = std::mem::take(crop);
                crop.insert(mode, mode_value);
                crop.insert(values, Value::Map(edges));
            }
            for value in map.values_mut() {
                migrate_crop_modes(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                migrate_crop_modes(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => migrate_crop_modes(value),
        _ => {}
    }
}
