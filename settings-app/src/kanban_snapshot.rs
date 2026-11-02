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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping_property: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_ids: Vec<String>,
    #[serde(default)]
    pub empty_value: Option<crate::calculator::CalculationValue>,
    pub value: Option<crate::calculator::CalculationValue>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmptyResult {
    pub target: Target,
    pub value: Option<crate::calculator::CalculationValue>,
}
pub fn empty_results(results: &[CategoryResult]) -> Vec<EmptyResult> {
    let mut seen = std::collections::BTreeSet::new();
    results.iter().filter(|r| r.grouping_property.is_some() && seen.insert(r.target.clone()))
        .map(|r| EmptyResult { target: r.target.clone(), value: r.empty_value }).collect()
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub display: String,
    pub calculated_at: String,
    pub results: Vec<CategoryResult>,
    #[serde(default)]
    pub empty_results: Vec<EmptyResult>,
}
