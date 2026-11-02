//! Allowlisted v2 content operations shared by the MCP host and the BlobUI bridge.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentRequest {
    pub tool: String,
    pub arguments: Value,
}
pub fn catalog() -> &'static Vec<Value> {
    static CATALOG: OnceLock<Vec<Value>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("content-tools.json")).expect("checked content catalog")
    })
}
pub fn spec(name: &str) -> Option<&'static Value> {
    catalog().iter().find(|entry| entry["name"] == name)
}
impl ContentRequest {
    pub fn specification(&self) -> Result<&'static Value, String> {
        spec(&self.tool).ok_or_else(|| "Unknown content tool.".into())
    }
    pub fn mutates(&self) -> bool {
        self.specification()
            .is_ok_and(|s| s["capability"] != "read")
    }
    pub fn target(&self) -> Option<&str> {
        self.specification()
            .ok()?
            .get("target")?
            .as_str()
            .and_then(|key| self.arguments[key].as_str())
    }
    pub fn space_wide(&self) -> bool {
        self.specification().is_ok_and(|s| s["scope"] == "space")
    }
    pub fn irreversible(&self) -> bool {
        self.specification()
            .is_ok_and(|s| s["irreversible"] == true)
    }
    /// Creation, deletion and uploads always need one-shot confirmation.
    pub fn requires_confirmation(&self, approval: &str) -> bool {
        approval == "alwaysAsk" || self.irreversible()
            || (approval == "approveForMe" && self.capability() == "createDelete")
    }
    pub fn capability(&self) -> &str {
        fn contains_views(value: &Value) -> bool {
            match value {
                Value::Object(fields) => fields
                    .iter()
                    .any(|(key, value)| key == "views" || contains_views(value)),
                Value::Array(values) => values.iter().any(contains_views),
                _ => false,
            }
        }
        let base = self
            .specification()
            .ok()
            .and_then(|s| s["capability"].as_str())
            .unwrap_or("none");
        if self.tool == "patch_object"
            && (contains_views(&self.arguments["body"])
                || self.arguments["body"]["ops"].as_array().is_some_and(|ops| {
                    ops.iter()
                        .any(|op| matches!(op["op"].as_str(), Some("insert_view" | "delete_view")))
                }))
        {
            "createDelete"
        } else {
            base
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let spec = self.specification()?;
        validate_schema(&spec["schema"], &self.arguments, "arguments")?;
        if let Some(destination) = self.arguments.get("destination") {
            let id = destination["id"].as_str().ok_or("Missing destination ID.")?;
            if matches!(id, "." | "..") || !id.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c)) {
                return Err("Invalid destination ID.".into());
            }
            if self.arguments["idempotency_key"].as_str().is_none() {
                return Err("Scoped creation requires a caller-supplied idempotency_key.".into());
            }
            if self.arguments["create_missing_options"] == true {
                return Err("Scoped creation cannot create select options.".into());
            }
        }
        if self.arguments.to_string().len() > 11 * 1024 * 1024 {
            return Err("Content request exceeds 11 MiB.".into());
        }
        for key in [
            "space_id",
            "object_id",
            "chat_id",
            "collection_id",
            "query_id",
            "message_id",
            "widget_id",
        ] {
            if let Some(value) = self.arguments.get(key) {
                let id = value.as_str().ok_or("Expected a string ID.")?;
                if id.is_empty()
                    || id.len() > 512
                    || matches!(id, "." | "..")
                    || !id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                {
                    return Err(format!("Invalid {key}."));
                }
            }
        }
        for key in ["etag", "idempotency_key"] {
            if let Some(value) = self.arguments[key].as_str() {
                if value.is_empty()
                    || value.chars().any(char::is_control)
                    || value.len() > 256
                    || value == "*"
                {
                    return Err(format!("Invalid {key}."));
                }
            }
        }
        for key in ["limit", "offset"] {
            if let Some(value) = self.arguments.get(key) {
                let n = value
                    .as_u64()
                    .ok_or("Pagination must be nonnegative integers.")?;
                if (key == "limit" && !(1..=100).contains(&n)) || n > i32::MAX as u64 {
                    return Err("Use a limit of 1–100 and a nonnegative 32-bit offset.".into());
                }
            }
        }
        if self.tool == "upload_file" {
            if self.arguments["local_path"].is_string() == self.arguments["url"].is_string() {
                return Err("Specify exactly one of local_path or url.".into());
            }
            if let Some(url) = self.arguments["url"].as_str() {
                if !(url.starts_with("https://") || url.starts_with("http://"))
                    || url.chars().any(char::is_control)
                {
                    return Err("Upload URL must use HTTP or HTTPS.".into());
                }
            }
        }
        if self.tool == "patch_object" {
            let ops = self.arguments["body"]["ops"]
                .as_array()
                .ok_or("PATCH requires ops.")?;
            for op in ops {
                if ![
                    "set_properties",
                    "set_type",
                    "update_block",
                    "replace_subtree",
                    "insert_blocks",
                    "move_block",
                    "delete_block",
                    "replace_text",
                    "set_cell",
                    "update_view",
                    "insert_view",
                    "move_view",
                    "delete_view",
                    "add_items",
                    "remove_items",
                ]
                .contains(&op["op"].as_str().unwrap_or(""))
                {
                    return Err("Unsupported PATCH operation. Discover the connected operation schema first.".into());
                }
            }
        }
        Ok(())
    }
}
pub fn tools() -> Vec<Value> {
    catalog().iter().map(|s| json!({"name":s["name"],"description":s["description"],"inputSchema":s["schema"],"annotations":{"readOnlyHint":s["capability"]=="read","destructiveHint":s["capability"]!="read","idempotentHint":s["method"]=="GET","openWorldHint":s["name"]=="upload_file"}})).collect()
}
/// Validate structural JSON Schema rules locally. The connected server remains authoritative
/// for semantic rules, patterns, formats and reference resolution, using dry_run before writes.
pub fn validate_schema(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let fail = || {
        Err(format!(
            "Invalid value at {path}. Inspect the connected v2 schema."
        ))
    };
    if schema == &Value::Bool(false) {
        return fail();
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(options) = schema[keyword].as_array() {
            let matches = options
                .iter()
                .filter(|s| validate_schema(s, value, path).is_ok())
                .count();
            if matches == 0 || (keyword == "oneOf" && matches != 1) {
                return fail();
            }
        }
    }
    if let Some(options) = schema["allOf"].as_array() {
        for s in options {
            validate_schema(s, value, path)?;
        }
    }
    if let Some(negated) = schema.get("not") {
        if validate_schema(negated, value, path).is_ok() {
            return fail();
        }
    }
    if let Some(expected) = schema.get("const") {
        if value != expected {
            return fail();
        }
    }
    if let Some(options) = schema["enum"].as_array() {
        if !options.contains(value) {
            return fail();
        }
    }
    let is_type = |kind: &str| match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    };
    if let Some(kind) = schema["type"].as_str() {
        if !is_type(kind) {
            return fail();
        }
    }
    if let Some(kinds) = schema["type"].as_array() {
        if !kinds.iter().any(|k| k.as_str().is_some_and(is_type)) {
            return fail();
        }
    }
    let bounds = |n: f64, min: &str, max: &str| -> Result<(), String> {
        if schema[min].as_f64().is_some_and(|v| n < v)
            || schema[max].as_f64().is_some_and(|v| n > v)
        {
            fail()
        } else {
            Ok(())
        }
    };
    if let Some(n) = value.as_f64() {
        bounds(n, "minimum", "maximum")?;
    }
    if let Some(text) = value.as_str() {
        bounds(text.chars().count() as f64, "minLength", "maxLength")?;
    }
    if let Some(items) = value.as_array() {
        bounds(items.len() as f64, "minItems", "maxItems")?;
        if let Some(s) = schema.get("items") {
            for (i, item) in items.iter().enumerate() {
                validate_schema(s, item, &format!("{path}/{i}"))?;
            }
        }
    }
    if let Some(object) = value.as_object() {
        bounds(object.len() as f64, "minProperties", "maxProperties")?;
        if let Some(required) = schema["required"].as_array() {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return Err(format!("Missing {path}/{key}."));
                }
            }
        }
        for (key, v) in object {
            if let Some(s) = schema["properties"].get(key) {
                validate_schema(s, v, &format!("{path}/{key}"))?;
            } else if schema["additionalProperties"] == false {
                return Err(format!("Unknown field {path}/{key}."));
            } else if schema["additionalProperties"].is_object() {
                validate_schema(&schema["additionalProperties"], v, &format!("{path}/{key}"))?;
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pagination_schema_matches_runtime_bounds() {
        let advertised = tools();
        for spec in catalog() {
            let tool = spec["name"].as_str().unwrap();
            let schema = &advertised.iter().find(|s| s["name"] == tool).unwrap()["inputSchema"];
            for (key, minimum, maximum) in [("limit", 1, 100), ("offset", 0, i32::MAX as i64)] {
                if spec["schema"]["properties"].get(key).is_none() {
                    continue;
                }
                let property = &schema["properties"][key];
                assert_eq!(property["minimum"], minimum, "{tool}/{key}");
                assert_eq!(property["maximum"], maximum, "{tool}/{key}");
                for n in [minimum, maximum] {
                    assert!(validate_schema(property, &json!(n), key).is_ok());
                }
                for value in [json!(minimum - 1), json!(maximum + 1), json!(1.5), json!("25"), Value::Null] {
                    assert!(validate_schema(property, &value, key).is_err(), "{tool}/{key}: {value}");
                }
            }
        }
        for args in [json!({"space_id":"s"}), json!({"space_id":"s","limit":1,"offset":0}), json!({"space_id":"s","limit":100,"offset":2147483647})] {
            assert!(crate::operation("list_properties", args).is_ok());
        }
        for args in [json!({"space_id":"s","limit":101}), json!({"space_id":"s","limit":0}), json!({"space_id":"s","offset":-1}), json!({"space_id":"s","offset":2147483648_i64})] {
            assert!(crate::operation("list_properties", args).is_err());
        }
    }
    #[test]
    fn creation_deletion_and_uploads_always_require_confirmation() {
        for spec in catalog() {
            let request = ContentRequest {
                tool: spec["name"].as_str().unwrap().into(),
                arguments: json!({}),
            };
            assert_eq!(request.requires_confirmation("approveForMe"),
                spec["capability"] == "createDelete" || spec["irreversible"] == true,
                "{}", request.tool);
            assert_eq!(request.requires_confirmation("fullAccess"), spec["irreversible"] == true);
            assert!(request.requires_confirmation("alwaysAsk"));
        }
        for op in ["insert_view", "delete_view"] {
            let request = ContentRequest {
                tool: "patch_object".into(),
                arguments: json!({"body":{"ops":[{"op":op}]}}),
            };
            assert!(request.requires_confirmation("approveForMe"));
        }
    }
    #[test]
    fn catalog_has_no_arbitrary_routes_or_admin_tools() {
        assert_eq!(catalog().len(), 41);
        for spec in catalog() {
            assert!(spec["route"].as_str().unwrap().starts_with("/v2/"));
            assert!(!spec["name"].as_str().unwrap().contains("space"));
            assert_eq!(spec["schema"]["additionalProperties"], false);
        }
    }
    #[test]
    fn nested_view_replacements_require_create_delete() {
        for op in [
            json!({"op":"update_block","id":"b","set":{"views":[]}}),
            json!({"op":"insert_blocks","blocks":[{"type":"dataview","views":[{"name":"new"}]}]}),
        ] {
            let crate::Operation::Content { content } = crate::operation(
                "patch_object",
                json!({"space_id":"s","object_id":"o","etag":"e","body":{"ops":[op]}}),
            )
            .unwrap() else {
                panic!()
            };
            assert_eq!(content.capability(), "createDelete");
        }
    }
    #[test]
    fn mutation_arguments_fail_closed() {
        for args in [
            json!({"space_id":"s","object_id":"o","body":{"ops":[]}}),
            json!({"space_id":"s","object_id":"o","etag":"*","body":{"ops":[{"op":"delete_block"}]}}),
            json!({"space_id":"s","object_id":"../o","etag":"e","body":{"ops":[{"op":"delete_block"}]}}),
            json!({"space_id":"s","object_id":"o","etag":"e\r\nx:y","body":{"ops":[{"op":"delete_block"}]}}),
        ] {
            assert!(crate::operation("patch_object", args).is_err());
        }
        assert!(crate::operation("patch_object",json!({"space_id":"s","object_id":"o","etag":"e","body":{"ops":[{"op":"set_properties","set":{"name":"new"}}]}})).is_ok());
        assert!(crate::operation(
            "upload_file",
            json!({"space_id":"s","url":"file:///secret"})
        )
        .is_err());
        assert!(crate::operation(
            "upload_file",
            json!({"space_id":"s","url":"https://example.com/a","local_path":"C:\\a"})
        )
        .is_err());
        assert!(crate::operation(
            "create_object",
            json!({"space_id":"s","body":{"type":"page"},"path":"/v1/objects"})
        )
        .is_err());
    }
    #[test]
    fn scoped_creation_requires_valid_destination_and_caller_key() {
        let valid = json!({"space_id":"s","destination":{"kind":"collection","id":"saved"},"idempotency_key":"caller-key","body":{"type":"page"}});
        assert!(crate::operation("create_object",valid.clone()).is_ok());
        for (path,value) in [("/destination/id",json!("../foreign")),("/destination/kind",json!("type")),("/idempotency_key",json!("*"))] {
            let mut bad = valid.clone(); *bad.pointer_mut(path).unwrap() = value;
            assert!(crate::operation("create_object",bad).is_err());
        }
        let mut missing = valid.clone();missing.as_object_mut().unwrap().remove("idempotency_key");
        assert!(crate::operation("create_object",missing).is_err());
        let mut options = valid;options["create_missing_options"] = json!(true);
        assert!(crate::operation("create_object",options).is_err());
        let list = crate::operation("list_creation_targets",json!({"space_id":"s"})).unwrap();
        let crate::Operation::Content {content} = list else {panic!()};
        assert!(!content.mutates());
    }
}
