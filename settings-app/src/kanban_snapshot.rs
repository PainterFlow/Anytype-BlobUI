//! Saved Kanban result types shared by Settings and installer CSS compilation.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub space_id: String,
    pub object_id: String,
    pub block_id: String,
    pub view_id: String,
}
pub fn validate_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err("A saved view has an invalid identity. Reload the view list.".into());
    }
    Ok(())
}
impl Target {
    pub fn validate(&self) -> Result<(), String> {
        for id in [
            &self.space_id,
            &self.object_id,
            &self.block_id,
            &self.view_id,
        ] {
            validate_id(id)?;
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CategoryResult {
    pub target: Target,
    pub category_id: String,
    pub value: Option<crate::calculator::CalculationValue>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub display: String,
    pub calculated_at: String,
    pub results: Vec<CategoryResult>,
}
