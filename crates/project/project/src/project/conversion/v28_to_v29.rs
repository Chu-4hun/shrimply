use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 28;
const TARGET_VERSION: u32 = 29;

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
        remove_layer_source_indices(&mut project);
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn remove_layer_source_indices(value: &mut Value) {
    match value {
        Value::Map(map) => {
            map.remove(&Value::String("source_index".to_string()));
            for value in map.values_mut() {
                remove_layer_source_indices(value);
            }
        }
        Value::Seq(values) => {
            for value in values {
                remove_layer_source_indices(value);
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => remove_layer_source_indices(value),
        _ => {}
    }
}
