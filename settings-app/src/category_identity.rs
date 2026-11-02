//! Exact option names scoped by the caller to one space and property.
use serde_json::Value;
use std::collections::BTreeSet;
pub(crate) fn validate_names(space: &str, property: &str, options: &[Value]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for option in options {
        let name = option["name"].as_str().ok_or("An option name is missing or malformed.")?;
        if !names.insert(name) {
            return Err(format!("duplicate_option_name: space {space:?}, property {property:?}, name {name:?}. Property excluded."));
        }
    }
    Ok(())
}
pub(crate) fn column_id(format: &str, names: &[String]) -> Result<String, String> {
    if !matches!(format, "select" | "multi_select") { return Err("Unsupported grouping format.".into()); }
    if format == "select" && names.len() > 1 { return Err("invalid_single_select_category".into()); }
    let mut names = names.to_vec(); names.sort(); names.dedup();
    serde_json::to_string(&names).map_err(|e| e.to_string())
}
#[cfg(test)] mod tests {
    use super::*; use serde_json::json;
    #[test] fn exact_names_and_combinations() {
        assert!(validate_names("s", "p", &[json!({"name":"A"}),json!({"name":"A"})]).unwrap_err().contains("Property excluded"));
        assert!(validate_names("s", "p", &[json!({"name":"A"}),json!({"name":"a"}),json!({"name":" A"})]).is_ok());
        assert_eq!(column_id("multi_select", &["B".into(),"A".into(),"A".into()]).unwrap(), r#"["A","B"]"#);
        assert_ne!(column_id("multi_select", &["A,B".into()]).unwrap(), column_id("multi_select", &["A".into(),"B".into()]).unwrap());
    }
}
