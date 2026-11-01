//! Shared typed bridge contract. No Anytype credentials or HTTP client belong here.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub mod transport;
pub mod credential;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    ListSpaces {},
    SearchObjects {
        space_id: String,
        query: String,
        #[serde(default = "default_limit")]
        limit: u32,
        #[serde(default)]
        offset: u32,
    },
    ReadObject {
        space_id: String,
        object_id: String,
    },
}
fn default_limit() -> u32 {
    20
}
impl Operation {
    pub fn validate(&self) -> Result<(), String> {
        fn id(value: &str) -> bool {
            !value.is_empty()
                && value.len() <= 512
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                && !value.chars().any(char::is_control)
                && ![".", ".."].contains(&value)
        }
        match self {
            Self::ListSpaces{} => Ok(()),
            Self::SearchObjects { space_id, query, limit, offset } if id(space_id) && query.len() <= 4096 && *limit > 0 && *limit <= 100 && *offset <= i32::MAX as u32 => Ok(()),
            Self::ReadObject { space_id, object_id } if id(space_id) && id(object_id) => Ok(()),
            _ => Err("Invalid arguments. Use explicit IDs, a limit of 1–100, and a nonnegative 32-bit offset.".into()),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::ListSpaces {} => "list_spaces",
            Self::SearchObjects { .. } => "search_objects",
            Self::ReadObject { .. } => "read_object",
        }
    }
    pub fn space(&self) -> Option<&str> {
        match self {
            Self::ListSpaces {} => None,
            Self::SearchObjects { space_id, .. } | Self::ReadObject { space_id, .. } => {
                Some(space_id)
            }
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub token: String,
    pub request: Operation,
}
#[derive(Serialize, Deserialize)]
pub struct Response {
    pub result: Option<Value>,
    pub error: Option<String>,
}
pub fn tools() -> Value {
    let id = json!({"type":"string","minLength":1,"maxLength":512});
    let annotations = json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false});
    json!([
        {"name":"list_spaces","description":"List spaces readable through saved BlobUI permissions.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":annotations},
        {"name":"search_objects","description":"Search one explicit space. BlobUI may require approval.","inputSchema":{"type":"object","properties":{"space_id":id,"query":{"type":"string","maxLength":4096},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20},"offset":{"type":"integer","minimum":0,"maximum":2147483647,"default":0}},"required":["space_id","query"],"additionalProperties":false},"annotations":annotations},
        {"name":"read_object","description":"Read an explicit object as properties and Markdown. Attachments remain references.","inputSchema":{"type":"object","properties":{"space_id":id,"object_id":id},"required":["space_id","object_id"],"additionalProperties":false},"annotations":annotations}
    ])
}
pub fn operation(name: &str, arguments: Value) -> Result<Operation, String> {
    let mut arguments = arguments
        .as_object()
        .cloned()
        .ok_or("Arguments must be an object.")?;
    if arguments.contains_key("operation") {
        return Err("Unknown argument.".into());
    }
    arguments.insert("operation".into(), name.into());
    let op: Operation = serde_json::from_value(Value::Object(arguments))
        .map_err(|_| "Unknown tool or invalid arguments.".to_string())?;
    op.validate()?;
    Ok(op)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_arguments() {
        for name in ["delete_object", "GET", ""] {
            assert!(operation(name, json!({})).is_err());
        }
        for args in [
            json!({"space_id":"x","query":"q","offset":-1}),
            json!({"space_id":"x","query":"q","limit":101}),
            json!({"space_id":"x","query":"q","path":"/v1"}),
            json!({"space_id":"x","query":"q","operation":"list_spaces"}),
        ] {
            assert!(operation("search_objects", args).is_err());
        }
        assert!(matches!(
            operation("search_objects", json!({"space_id":"x","query":""})).unwrap(),
            Operation::SearchObjects {
                limit: 20,
                offset: 0,
                ..
            }
        ));
        assert!(operation("list_spaces", json!({"extra":true})).is_err());
    }
}
