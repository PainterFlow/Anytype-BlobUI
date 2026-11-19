//! Shared typed bridge contract. No Anytype credentials or HTTP client belong here.
extern crate self as blobui_mcp;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub mod transport;
pub mod approval;
pub mod credential;
pub mod content;
pub mod sources;
pub mod blobui;
pub mod server;

pub fn serve_bridge_stdio() {
    server::serve_stdio(credential::read, |request, cancel, approve| {
        transport::call_with_approval(&request, cancel, &|message, remaining| {
            approve(message, remaining)
        })
    });
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Blobui { blobui: blobui::BlobuiRequest },
    Content { content: content::ContentRequest },
    SearchMentions { query: String },
    AuthenticateProject {
        name: String,
        key: String,
        remote: Option<bool>,
    },
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
            Self::AuthenticateProject { name, key, .. }
                if !name.trim().is_empty()
                    && name.trim().len() <= 512
                    && !name.chars().any(char::is_control)
                    && !key.trim().is_empty()
                    && key.len() <= 16 * 1024
                    && !key.chars().any(char::is_control) => Ok(()),
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
            Self::AuthenticateProject { .. } => "authenticate_project",
            Self::ListSpaces {} => "list_spaces",
            Self::SearchObjects { .. } => "search_objects",
            Self::ReadObject { .. } => "read_object",
        }
    }
    pub fn space(&self) -> Option<&str> {
        match self {
            Self::Blobui { blobui } => blobui.arguments["space_id"].as_str(),
            Self::Content { content } => content.arguments["space_id"].as_str(),
            Self::ListSpaces {} | Self::SearchMentions { .. } | Self::AuthenticateProject { .. } => None,
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
    let meta = params.get("_meta");
    let direct = metadata_thread_id(meta.and_then(|meta| meta.get("threadId")))?;
    let turn_metadata = match meta.and_then(|meta| meta.get("x-codex-turn-metadata")) {
        None | Some(Value::Null) => None,
        Some(Value::Object(metadata)) => Some(Value::Object(metadata.clone())),
        Some(Value::String(metadata)) => {
            let metadata: Value = serde_json::from_str(metadata)
                .map_err(|_| "Codex supplied invalid chat identity metadata.")?;
            if !metadata.is_object() {
                return Err("Codex supplied invalid chat identity metadata.".into());
            }
            Some(metadata)
        }
        _ => return Err("Codex supplied invalid chat identity metadata.".into()),
    };
    let turn = metadata_thread_id(turn_metadata.as_ref().and_then(|value| value.get("thread_id")))?;
    match (direct, turn) {
        (Some(direct), Some(turn)) if direct != turn => Err("Codex supplied conflicting chat identity metadata.".into()),
        (Some(direct), _) => Ok(Some(direct)),
        (None, turn) => Ok(turn),
    }
}
fn metadata_thread_id(value: Option<&Value>) -> Result<Option<String>, String> {
    match value {
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
fn decode_percent_encoded_resource(value: &str) -> Result<String, String> {
    let bytes=value.as_bytes();
    let mut decoded=Vec::with_capacity(bytes.len());
    let mut index=0;
    while index<bytes.len() {
        if bytes[index]==b'%' {
            if index+2>=bytes.len() { return Err("Invalid wrapped Anyblob resource URI.".into()); }
            let digit=|byte:u8| match byte {
                b'0'..=b'9'=>Some(byte-b'0'),b'a'..=b'f'=>Some(byte-b'a'+10),b'A'..=b'F'=>Some(byte-b'A'+10),_=>None
            };
            let high=digit(bytes[index+1]).ok_or("Invalid wrapped Anyblob resource URI.")?;
            let low=digit(bytes[index+2]).ok_or("Invalid wrapped Anyblob resource URI.")?;
            decoded.push((high<<4)|low);
            index+=3;
        } else {
            decoded.push(bytes[index]);
            index+=1;
        }
    }
    String::from_utf8(decoded).map_err(|_|"Invalid wrapped Anyblob resource URI.".into())
}
fn json_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null=>"null",
        Value::Bool(_)=>"boolean",
        Value::Number(_)=>"number",
        Value::String(_)=>"string",
        Value::Array(_)=>"array",
        Value::Object(_)=>"object",
    }
}
fn authenticate_project_argument_error(arguments: &Value) -> String {
    let Some(fields)=arguments.as_object() else {
        return format!("Invalid authenticate_project arguments. Expected name:string, key:string, and optional remote:boolean in an object. Received {}. Keep chat identity in MCP metadata.",json_value_kind(arguments));
    };
    let expected=[("name","string"),("key","string")];
    let missing=expected.iter().filter(|(key,_)|!fields.contains_key(*key)).map(|(key,_)|*key).collect::<Vec<_>>();
    let mut wrong=expected.iter().filter_map(|(key,kind)|fields.get(*key).filter(|value|json_value_kind(value)!=*kind).map(|value|format!("{key}:{}",json_value_kind(value)))).collect::<Vec<_>>();
    if let Some(value)=fields.get("remote").filter(|value|!value.is_boolean()) {
        wrong.push(format!("remote:{}",json_value_kind(value)));
    }
    let unexpected=fields.keys().filter(|key|!expected.iter().any(|(known,_)|*known==key.as_str()) && key.as_str()!="remote").count();
    format!("Invalid authenticate_project arguments. Expected name:string, key:string, and optional remote:boolean. Missing fields [{}], wrong types [{}], unexpected field count {}. Keep chat identity in MCP metadata.",if missing.is_empty(){"none".into()}else{missing.join(", ")},if wrong.is_empty(){"none".into()}else{wrong.join(", ")},unexpected)
}
/// Parse canonical URIs, or the exact single-segment Codex wrapper around one.
pub fn resource_operation(params: &Value) -> Result<Operation, String> {
    let params=params.as_object().ok_or("Resource read parameters must be an object.")?;
    if params.keys().any(|key| key!="uri" && key!="_meta") {
        return Err("Unknown resource read argument. Chat identity must come from host metadata.".into());
    }
    let uri=params.get("uri").and_then(Value::as_str).ok_or("Resource URI is required.")?;
    let owned;
    let canonical=if uri.starts_with("anyblob://objects/") {
        uri
    } else {
        let wrapped=uri.strip_prefix("mcp-resource://anyblob/").ok_or("Unsupported Anyblob resource URI.")?;
        if wrapped.is_empty() || wrapped.bytes().any(|byte| matches!(byte,b'/'|b'?'|b'#')) {
            return Err("Invalid wrapped Anyblob resource URI.".into());
        }
        owned=decode_percent_encoded_resource(wrapped)?;
        if !owned.starts_with("anyblob://objects/") {
            return Err("Invalid wrapped Anyblob resource URI.".into());
        }
        &owned
    };
    let path=canonical.strip_prefix("anyblob://objects/").ok_or("Unsupported Anyblob resource URI.")?;
    let (space,object)=path.split_once('/').ok_or("Invalid Anyblob object resource URI.")?;
    let op=Operation::ReadObject{space_id:space.into(),object_id:object.into()};
    op.validate()?;
    if object_uri(space,object)!=canonical {return Err("Invalid Anyblob object resource URI.".into());}
    Ok(op)
}
pub fn tools() -> Value {
    let id = json!({"type":"string","minLength":1,"maxLength":512});
    let annotations = json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false});
    let mut tools = json!([
        {"name":"search_mentions","description":"Find object mentions using saved global filters. Suggestions grant no read permission.","inputSchema":{"type":"object","properties":{"query":{"type":"string","maxLength":4096}},"required":["query"],"additionalProperties":false},"outputSchema":{"type":"object","properties":{"items":{"type":"array","maxItems":20,"items":{"type":"object","properties":{"type":{"const":"resource_link"},"uri":{"type":"string"},"name":{"type":"string"}},"required":["type","uri","name"]}}},"required":["items"],"additionalProperties":false},"_meta":{"ui":{"visibility":["app"]}},"annotations":annotations},
        {"name":"authenticate_project","description":"Call once per project chat with only name and key from anyblob.config.toml. Omit legacy remote. If sent, authentication succeeds with a removal notice. Never send the whole file.","inputSchema":{"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":512},"key":{"type":"string","minLength":1,"maxLength":16384},"remote":{"type":"boolean"}},"required":["name","key"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}},
        {"name":"list_spaces","description":"List spaces readable through saved BlobUI permissions.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":annotations},
        {"name":"search_objects","description":"Search one explicit space. BlobUI may require approval.","inputSchema":{"type":"object","properties":{"space_id":id,"query":{"type":"string","maxLength":4096},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20},"offset":{"type":"integer","minimum":0,"maximum":2147483647,"default":0}},"required":["space_id","query"],"additionalProperties":false},"annotations":annotations},
        {"name":"read_object","description":"Read an explicit object as properties, Markdown, and its ETag. Attachments remain references. Use the exact returned ETag with patch_object. If patch_object reports a stale ETag, re-read the object and rebuild the patch from the current document.","inputSchema":{"type":"object","properties":{"space_id":id,"object_id":id},"required":["space_id","object_id"],"additionalProperties":false},"outputSchema":{"type":"object","properties":{"etag":{"type":"string","minLength":1}},"required":["etag"],"additionalProperties":true},"annotations":annotations}
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
    let auth_parse_error=if name=="authenticate_project" {
        Some(authenticate_project_argument_error(&Value::Object(arguments.clone())))
    } else {
        None
    };
    arguments.insert("operation".into(), name.into());
    let op: Operation = serde_json::from_value(Value::Object(arguments))
        .map_err(|_| auth_parse_error.unwrap_or_else(||"Unknown tool or invalid arguments.".into()))?;
    op.validate()?;
    Ok(op)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_uris_are_canonical_and_never_carry_consent_or_identity() {
        assert!(matches!(resource_operation(&json!({"uri":object_uri("space-id","object_id"),"_meta":{"threadId":"host-chat"}})).unwrap(),Operation::ReadObject{..}));
        let wrapped="mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fspace-id%2Fobject_id";
        assert!(matches!(resource_operation(&json!({"uri":wrapped,"_meta":{"threadId":"host-chat"}})).unwrap(),Operation::ReadObject{space_id,object_id} if space_id=="space-id"&&object_id=="object_id"));
        for uri in ["anyblob://objects/a/b/c","anyblob://objects/a/..","anyblob://objects/a/%62","anyblob://objects/a/b?consent=true","anyblob://objects/a/b#x","anyblob://objects/a/","anyblob://other/a/b","ANYBLOB://objects/a/b","anyblob://objects/a\\b/c","anyblob://objects/a/猫"] {
            assert!(resource_operation(&json!({"uri":uri})).is_err(),"{uri}");
        }
        for uri in [
            "mcp-resource://other/anyblob%3A%2F%2Fobjects%2Fa%2Fb",
            "mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fa%2Fb/extra",
            "mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fa%2Fb?consent=true",
            "mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fa%2Fb#fragment",
            "mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fa%2Fb%3Fconsent%3Dtrue",
            "mcp-resource://anyblob/anyblob%253A%252F%252Fobjects%252Fa%252Fb",
            "mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fa%2F..",
            "mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fa%2Fb%ZZ",
        ] {
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
        assert_eq!(thread_id(&json!({"_meta":{"x-codex-turn-metadata":{"thread_id":"chat-456"}},"arguments":{"thread_id":"spoof"}})).unwrap(),Some("chat-456".into()));
        assert_eq!(thread_id(&json!({"_meta":{"x-codex-turn-metadata":"{\"thread_id\":\"chat-789\"}"}})).unwrap(),Some("chat-789".into()));
        assert!(thread_id(&json!({"_meta":{"threadId":"chat-123","x-codex-turn-metadata":{"thread_id":"chat-456"}}})).is_err());
        assert!(thread_id(&json!({"_meta":{"x-codex-turn-metadata":"invalid json"}})).is_err());
        assert!(thread_id(&json!({"_meta":{"x-codex-turn-metadata":{"thread_id":"../escape"}}})).is_err());
        assert_eq!(thread_id(&json!({"arguments":{"threadId":"spoof"}})).unwrap(),None);
        for bad in [json!(""),json!(false),json!("../escape"),json!("x".repeat(129))] {
            assert!(thread_id(&json!({"_meta":{"threadId":bad}})).is_err());
        }
        assert!(operation("list_spaces",json!({"threadId":"spoof"})).is_err());
        for key in ["host_id", "project_id", "remoteMembership", "remote_membership"] {
            let mut arguments = json!({"query":"task"});
            arguments[key] = json!("spoof");
            assert!(operation("search_mentions", arguments).is_err(), "{key}");
        }
        let request: Request = serde_json::from_value(json!({
            "id":"request",
            "token":"a".repeat(64),
            "thread_id":"chat-123",
            "request":{"operation":"list_spaces"},
        })).unwrap();
        assert_eq!(request.thread_id.as_deref(), Some("chat-123"));
        let envelope = serde_json::to_value(&request).unwrap();
        assert_eq!(envelope["thread_id"], "chat-123");
        assert!(envelope.get("remoteMembership").is_none());
        assert!(serde_json::from_value::<Request>(json!({
            "id":"request",
            "token":"a".repeat(64),
            "request":{"operation":"list_spaces"},
            "remoteMembership":{"hostId":"host_1","projectId":null}
        })).is_err());
    }
    #[test]
    fn strict_arguments() {
        assert!(operation("search_mentions",json!({"query":"猫"})).is_ok());
        assert!(matches!(operation("authenticate_project",json!({"name":"Work","key":"key"})).unwrap(), Operation::AuthenticateProject { name, key, remote: None } if name == "Work" && key == "key"));
        assert!(matches!(operation("authenticate_project",json!({"name":"Work","key":"key","remote":false})).unwrap(), Operation::AuthenticateProject { remote: Some(false), .. }));
        assert!(matches!(operation("authenticate_project",json!({"name":"Work","key":"key","remote":true})).unwrap(), Operation::AuthenticateProject { remote: Some(true), .. }));
        assert!(operation("authenticate_project",json!({"config":"name = 'Work'\nkey = 'key'\nremote = false"})).is_err());
        let auth_error=operation("authenticate_project",json!({"name":"Work","key":"secret-value","remote":"false","unexpected":true})).unwrap_err();
        assert!(auth_error.contains("Missing fields [none], wrong types [remote:string], unexpected field count 1"));
        assert!(!auth_error.contains("secret-value"));
        assert!(operation("authenticate_project",json!({"name":"Work","key":""})).is_err());
        assert!(operation("authenticate_project",json!({"name":"Work","key":"x".repeat(16 * 1024 + 1)})).is_err());
        let tool_list=tools();
        let auth=tool_list.as_array().unwrap().iter().find(|tool|tool["name"]=="authenticate_project").unwrap();
        assert_eq!(auth["inputSchema"]["required"],json!(["name","key"]));
        assert_eq!(auth["inputSchema"]["properties"]["remote"]["type"],"boolean");
        assert!(auth["description"].as_str().unwrap().contains("Omit legacy remote"));
        assert!(auth["inputSchema"]["properties"].get("config").is_none());
        assert!(operation("search_mentions",json!({"query":""})).is_ok());
        assert!(operation("search_mentions",json!({"query":"x".repeat(4097)})).is_err());
        for extra in [json!({"query":"","threadId":"forged"}),json!({"query":"","host_id":"forged"}),json!({"query":"","project_id":"forged"}),json!({"query":"","remoteMembership":{"hostId":"forged","projectId":"forged"}}),json!({"query":"","remote_membership":{"host_id":"forged","project_id":"forged"}}),json!({"query":"","consent":true}),json!({"query":"","space_id":"override"})] {
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
