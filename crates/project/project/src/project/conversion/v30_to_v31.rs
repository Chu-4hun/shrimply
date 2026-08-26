use serde_value::Value;

use super::{ProjectVersionConverter, ensure_project_version, set_project_version};

pub(super) struct Converter;

impl ProjectVersionConverter for Converter {
    fn source_version(&self) -> u32 {
        30
    }

    fn target_version(&self) -> u32 {
        31
    }

    fn convert(&self, mut project: Value) -> Result<Value, String> {
        ensure_project_version(&project, self.source_version())?;
        set_project_version(&mut project, self.target_version())?;
        Ok(project)
    }
}
