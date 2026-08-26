use std::collections::BTreeMap;

use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

const SOURCE_VERSION: u32 = 12;
const TARGET_VERSION: u32 = 13;
const FONT_FAMILIES_KEY: &str = "font_families";
const GOOGLE_FONT_FAMILIES_KEY: &str = "google_font_families";

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
        merge_font_families(&mut project)?;
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn merge_font_families(value: &mut Value) -> Result<(), String> {
    match value {
        Value::Map(map) => {
            let font_key = Value::String(FONT_FAMILIES_KEY.to_string());
            let google_key = Value::String(GOOGLE_FONT_FAMILIES_KEY.to_string());
            if map.contains_key(&font_key) || map.contains_key(&google_key) {
                let families = string_sequence(
                    map.remove(&font_key)
                        .unwrap_or_else(|| Value::Seq(Vec::new())),
                    FONT_FAMILIES_KEY,
                )?;
                let google = string_sequence(
                    map.remove(&google_key)
                        .unwrap_or_else(|| Value::Seq(Vec::new())),
                    GOOGLE_FONT_FAMILIES_KEY,
                )?;
                let mut merged = families
                    .iter()
                    .map(|family| {
                        font_family_value(
                            if google
                                .iter()
                                .any(|google| google.eq_ignore_ascii_case(family))
                            {
                                "google_fonts"
                            } else {
                                "local"
                            },
                            family,
                        )
                    })
                    .collect::<Vec<_>>();
                merged.extend(
                    google
                        .iter()
                        .filter(|google| {
                            !families
                                .iter()
                                .any(|family| family.eq_ignore_ascii_case(google))
                        })
                        .map(|family| font_family_value("google_fonts", family)),
                );
                map.insert(font_key, Value::Seq(merged));
            }
            for value in map.values_mut() {
                merge_font_families(value)?;
            }
        }
        Value::Seq(values) => {
            for value in values {
                merge_font_families(value)?;
            }
        }
        Value::Option(Some(value)) | Value::Newtype(value) => merge_font_families(value)?,
        _ => {}
    }
    Ok(())
}

fn string_sequence(value: Value, field: &str) -> Result<Vec<String>, String> {
    let Value::Seq(values) = value else {
        return Err(format!("project {field} must be a sequence"));
    };
    values
        .into_iter()
        .map(|value| match value {
            Value::String(value) => Ok(value),
            _ => Err(format!("project {field} entries must be strings")),
        })
        .collect()
}

fn font_family_value(source: &str, family: &str) -> Value {
    Value::Map(BTreeMap::from([(
        Value::String(source.to_string()),
        Value::Map(BTreeMap::from([(
            Value::String("name".to_string()),
            Value::String(family.to_string()),
        )])),
    )]))
}
