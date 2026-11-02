//! Shared typed bridge contract. No Anytype credentials or HTTP client belong here.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub mod transport;
pub mod approval;
pub mod credential;
pub mod content;
pub mod sources;
pub mod blobui;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Blobui { blobui: blobui::BlobuiRequest },
    Content { content: content::ContentRequest },
    SearchMentions { query: String },
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
            Self::Blobui { blobui } => blobui.validate(),
            Self::Content { content } => content.validate(),
            Self::SearchMentions { query } if query.len() <= 4096 => Ok(()),
            Self::ListSpaces{} => Ok(()),
            Self::SearchObjects { space_id, query, limit, offset } if id(space_id) && query.len() <= 4096 && *limit > 0 && *limit <= 100 && *offset <= i32::MAX as u32 => Ok(()),
            Self::ReadObject { space_id, object_id } if id(space_id) && id(object_id) => Ok(()),
            _ => Err("Invalid arguments. Use explicit IDs, a limit of 1–100, and a nonnegative 32-bit offset.".into()),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Blobui { blobui } => &blobui.tool,
            Self::Content { content } => &content.tool,
            Self::SearchMentions { .. } => "search_mentions",
            Self::ListSpaces {} => "list_spaces",
            Self::SearchObjects { .. } => "search_objects",
            Self::ReadObject { .. } => "read_object",
        }
    }
    pub fn space(&self) -> Option<&str> {
        match self {
            Self::Blobui { blobui } => blobui.arguments["space_id"].as_str(),
            Self::Content { content } => content.arguments["space_id"].as_str(),
            Self::ListSpaces {} | Self::SearchMentions { .. } => None,
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
    #[serde(default)]
    pub thread_id: Option<String>,
    pub request: Operation,
}
/// Host metadata is deliberately separate from model-controlled tool arguments.
pub fn thread_id(params: &Value) -> Result<Option<String>, String> {
    match params.pointer("/_meta/threadId") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(id)) if !id.is_empty() && id.len() <= 128
            && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') => Ok(Some(id.clone())),
        _ => Err("Codex supplied invalid chat identity metadata.".into()),
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub result: Option<Value>,
    pub error: Option<String>,
}
#[cfg(test)]
#[test]
fn bridge_response_rejects_unrecognized_envelopes() {
    assert!(serde_json::from_value::<Response>(json!({
        "frame":"response", "version":2, "response":{"result":{"items":[]},"error":null}
    })).is_err());
    let response: Response = serde_json::from_value(json!({"result":{"items":[]},"error":null})).unwrap();
    assert_eq!(response.result.unwrap(), json!({"items":[]}));
}
pub fn capabilities() -> Value {
    json!({"tools":{"listChanged":false},"resources":{"subscribe":false,"listChanged":false},
        "extensions":{"openai/mentions":{"searchTool":"search_mentions"}},
        "experimental":{"openai/mentions":{"searchTool":"search_mentions"}}})
}
pub fn object_uri(space: &str, object: &str) -> String {
    format!("anyblob://objects/{space}/{object}")
}
/// Parse raw canonical URIs without URL normalization, decoding or model-selected paths.
pub fn resource_operation(params: &Value) -> Result<Operation, String> {
    let params=params.as_object().ok_or("Resource read parameters must be an object.")?;
    if params.keys().any(|key| key!="uri" && key!="_meta") {
        return Err("Unknown resource read argument. Chat identity must come from host metadata.".into());
    }
    let uri=params.get("uri").and_then(Value::as_str).ok_or("Resource URI is required.")?;
    let path=uri.strip_prefix("anyblob://objects/").ok_or("Unsupported Anyblob resource URI.")?;
    let (space,object)=path.split_once('/').ok_or("Invalid Anyblob object resource URI.")?;
    let op=Operation::ReadObject{space_id:space.into(),object_id:object.into()};
    op.validate()?;
    if object_uri(space,object)!=uri {return Err("Invalid Anyblob object resource URI.".into());}
    Ok(op)
}
pub fn tools() -> Value {
    let id = json!({"type":"string","minLength":1,"maxLength":512});
    let annotations = json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false});
    let mut tools = json!([
        {"name":"search_mentions","description":"Find object mentions using saved global filters. Suggestions grant no read permission.","inputSchema":{"type":"object","properties":{"query":{"type":"string","maxLength":4096}},"required":["query"],"additionalProperties":false},"outputSchema":{"type":"object","properties":{"items":{"type":"array","maxItems":20,"items":{"type":"object","properties":{"type":{"const":"resource_link"},"uri":{"type":"string"},"name":{"type":"string"}},"required":["type","uri","name"]}}},"required":["items"],"additionalProperties":false},"_meta":{"ui":{"visibility":["app"]}},"annotations":annotations},
        {"name":"list_spaces","description":"List spaces readable through saved BlobUI permissions.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":annotations},
        {"name":"search_objects","description":"Search one explicit space. BlobUI may require approval.","inputSchema":{"type":"object","properties":{"space_id":id,"query":{"type":"string","maxLength":4096},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20},"offset":{"type":"integer","minimum":0,"maximum":2147483647,"default":0}},"required":["space_id","query"],"additionalProperties":false},"annotations":annotations},
        {"name":"read_object","description":"Read an explicit object as properties and Markdown. Attachments remain references.","inputSchema":{"type":"object","properties":{"space_id":id,"object_id":id},"required":["space_id","object_id"],"additionalProperties":false},"annotations":annotations}
    ]);
    tools.as_array_mut().unwrap().extend(content::tools());
    tools.as_array_mut().unwrap().extend(blobui::tools());
    tools
}
pub fn operation(name: &str, arguments: Value) -> Result<Operation, String> {
    if blobui::NAMES.contains(&name) {
        let blobui = blobui::BlobuiRequest {tool:name.into(), arguments};
        blobui.validate()?;
        return Ok(Operation::Blobui {blobui});
    }
    if content::spec(name).is_some() {
        let content = content::ContentRequest { tool: name.into(), arguments };
        content.validate()?;
        return Ok(Operation::Content { content });
    }
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
    fn resource_uris_are_canonical_and_never_carry_consent_or_identity() {
        assert!(matches!(resource_operation(&json!({"uri":object_uri("space-id","object_id"),"_meta":{"threadId":"host-chat"}})).unwrap(),Operation::ReadObject{..}));
        for uri in ["anyblob://objects/a/b/c","anyblob://objects/a/..","anyblob://objects/a/%62","anyblob://objects/a/b?consent=true","anyblob://objects/a/b#x","anyblob://objects/a/","anyblob://other/a/b","ANYBLOB://objects/a/b","anyblob://objects/a\\b/c","anyblob://objects/a/猫"] {
            assert!(resource_operation(&json!({"uri":uri})).is_err(),"{uri}");
        }
        for key in ["threadId","consent","approval","space_id"] {
            let mut params=json!({"uri":object_uri("a","b")});params[key]=json!("forged");
            assert!(resource_operation(&params).is_err());
        }
    }
    #[test]
    fn chat_identity_comes_only_from_host_metadata() {
        assert_eq!(thread_id(&json!({"_meta":{"threadId":"chat-123"},"arguments":{"threadId":"spoof"}})).unwrap(),Some("chat-123".into()));
        assert_eq!(thread_id(&json!({"arguments":{"threadId":"spoof"}})).unwrap(),None);
        for bad in [json!(""),json!(false),json!("../escape"),json!("x".repeat(129))] {
            assert!(thread_id(&json!({"_meta":{"threadId":bad}})).is_err());
        }
        assert!(operation("list_spaces",json!({"threadId":"spoof"})).is_err());
    }
    #[test]
    fn strict_arguments() {
        assert!(operation("search_mentions",json!({"query":"猫"})).is_ok());
        assert!(operation("search_mentions",json!({"query":""})).is_ok());
        assert!(operation("search_mentions",json!({"query":"x".repeat(4097)})).is_err());
        for extra in [json!({"query":"","threadId":"forged"}),json!({"query":"","consent":true}),json!({"query":"","space_id":"override"})] {
            assert!(operation("search_mentions",extra).is_err());
        }
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
