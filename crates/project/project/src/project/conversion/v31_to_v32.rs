use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, project_map, set_project_version};

const SOURCE_VERSION: u32 = 31;
const TARGET_VERSION: u32 = 32;

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
        reverse_tracks(&mut project)?;
        set_project_version(&mut project, TARGET_VERSION)?;
        Ok(project)
    }
}

fn reverse_tracks(project: &mut Value) -> Result<(), String> {
    let project = project_map(project)?;
    reverse_sequence(project.get_mut(&key("caption_tracks")), "caption_tracks")?;
    reverse_sequence(project.get_mut(&key("visual_tracks")), "visual_tracks")?;

    let Some(folded_sequences) = project.get_mut(&key("folded_sequences")) else {
        return Ok(());
    };
    let Value::Seq(folded_sequences) = folded_sequences else {
        return Err("project folded_sequences must be a sequence".to_string());
    };
    for folded_sequence in folded_sequences {
        let Value::Map(folded_sequence) = folded_sequence else {
            return Err("project folded_sequences entries must be maps".to_string());
        };
        reverse_sequence(
            folded_sequence.get_mut(&key("video_tracks")),
            "folded sequence video_tracks",
        )?;
    }
    Ok(())
}

fn reverse_sequence(value: Option<&mut Value>, field: &str) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let Value::Seq(values) = value else {
        return Err(format!("project {field} must be a sequence"));
    };
    values.reverse();
    Ok(())
}

fn key(value: &str) -> Value {
    Value::String(value.to_string())
}
