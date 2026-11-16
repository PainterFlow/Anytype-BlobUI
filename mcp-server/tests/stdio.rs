use serde_json::{json, Map, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

// Mirror the serializable subset of Codex's JsonSchema. This catalog stays
// under the threshold where Codex would start dropping schema details.
fn codex_parameters(schema: &Value) -> Value {
    let mut result = Map::new();
    for (key, value) in schema.as_object().unwrap() {
        let normalized = match key.as_str() {
            "const" => {
                result.insert("enum".into(), json!([value]));
                continue;
            }
            "properties" | "$defs" | "definitions" => Value::Object(
                value.as_object().unwrap().iter()
                    .map(|(name, schema)| (name.clone(), codex_parameters(schema)))
                    .collect(),
            ),
            "anyOf" | "oneOf" | "allOf" => {
                Value::Array(value.as_array().unwrap().iter().map(codex_parameters).collect())
            }
            "items" => codex_parameters(value),
            "additionalProperties" if value.is_object() => codex_parameters(value),
            "additionalProperties" | "$ref" | "description" | "encrypted" | "enum"
            | "minItems" | "required" | "type" => value.clone(),
            _ => continue,
        };
        result.insert(key.clone(), normalized);
    }
    if !result.contains_key("type")
        && !["$ref", "anyOf", "oneOf", "allOf"].iter().any(|key| result.contains_key(*key))
    {
        let inferred = if ["properties", "required", "additionalProperties"]
            .iter().any(|key| result.contains_key(*key))
        {
            "object"
        } else if result.contains_key("items") {
            "array"
        } else if result.contains_key("enum") || schema.get("format").is_some() {
            "string"
        } else if schema.get("minimum").is_some() || schema.get("maximum").is_some() {
            "number"
        } else {
            return json!({});
        };
        result.insert("type".into(), json!(inferred));
    }
    if result.get("type").is_some_and(|kind| kind == "object") {
        result.entry("properties").or_insert_with(|| json!({}));
    }
    if result.get("type").is_some_and(|kind| kind == "array") {
        result.entry("items").or_insert_with(|| json!({"type":"string"}));
    }
    Value::Object(result)
}

fn verify_codex_catalog_budget(instructions: &str, tools: &[Value]) {
    const TOOL_LIMIT: usize = 8_000;
    const CATALOG_TARGET: usize = 60_000; // Leave room below Codex's 64,000-byte limit.
    let mut count = 0;
    let mut total = 0;
    let mut largest = 0;
    for tool in tools {
        if tool["_meta"]["ui"]["visibility"].as_array()
            .is_some_and(|visibility| !visibility.contains(&json!("model")))
        {
            continue;
        }
        let name = tool["name"].as_str().unwrap();
        let description = tool["description"].as_str().unwrap().trim();
        let period = if description.ends_with('.') { "" } else { "." };
        let description = format!("{description}{period} This tool is part of plugin `Anyblob`.");
        let parameters = codex_parameters(&tool["inputSchema"]);
        assert!(
            serde_json::to_vec(&parameters).unwrap().len() < 5_000,
            "{name} would trigger Codex's schema compaction"
        );
        // Include the legacy namespace prefix for a conservative estimate.
        let spec = json!({
            "type":"namespace", "name":"mcp__anyblob", "description":instructions,
            "tools":[{"type":"function", "name":name, "description":description,
                      "strict":false, "parameters":parameters}]
        });
        let bytes = serde_json::to_vec(&spec).unwrap().len();
        assert!(bytes < TOOL_LIMIT, "{name} occupies {bytes} bytes");
        count += 1;
        total += bytes;
        largest = largest.max(bytes);
    }
    assert_eq!(count, 57);
    assert!(total < CATALOG_TARGET, "Codex tool catalog occupies {total} bytes");
    println!("{count} model tools, {total} bytes, largest tool {largest} bytes");
}

#[test]
fn stdio_initialization_schemas_malformed_unknown_and_no_credentials() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_blobui-mcp"))
        .env("ANYBLOB_BRIDGE_INSTANCE", "fixture-missing-credential")
        .env_remove("ANYBLOB_CREDENTIAL_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut call = |value: Value| {
        writeln!(input, "{value}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).expect("stdout must contain JSON-RPC only")
    };
    assert_eq!(
        call(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))["error"]["code"],
        -32002
    );
    let initialize = call(
            json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})
    );
    assert_eq!(initialize["result"]["serverInfo"]["name"], "anyblob");
    let tools = call(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}));
    let catalog = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(catalog.len(), 58);
    verify_codex_catalog_budget(initialize["result"]["instructions"].as_str().unwrap(), catalog);
    let patch = catalog.iter().find(|tool| tool["name"] == "patch_object").unwrap();
    assert_eq!(patch["annotations"]["readOnlyHint"], false);
    assert!(patch["inputSchema"]["required"].as_array().unwrap().contains(&json!("etag")));
    let mentions=&tools["result"]["tools"][0];
    assert_eq!(mentions["name"],"search_mentions");
    assert_eq!(mentions["_meta"]["ui"]["visibility"],json!(["app"]));
    assert_eq!(mentions["outputSchema"]["properties"]["items"]["maxItems"],20);
    let read_object = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "read_object")
        .unwrap();
    assert_eq!(read_object["outputSchema"]["properties"]["etag"]["type"], "string");
    assert!(read_object["outputSchema"]["required"].as_array().unwrap().contains(&json!("etag")));
    let initialized=call(json!({"jsonrpc":"2.0","id":30,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}));
    let icon = &initialized["result"]["serverInfo"]["icons"][0];
    assert_eq!(icon["src"], include_str!("../src/icon-data-uri.txt"));
    assert_eq!(icon["mimeType"], "image/png");
    assert_eq!(icon["sizes"], json!(["256x256"]));
    assert_eq!(initialized["result"]["capabilities"]["extensions"]["openai/mentions"]["searchTool"],"search_mentions");
    assert_eq!(call(json!({"jsonrpc":"2.0","id":31,"method":"resources/list"}))["result"]["resources"],json!([]));
    let templates = call(json!({"jsonrpc":"2.0","id":36,"method":"resources/templates/list"}));
    assert!(templates["result"]["resourceTemplates"][0]["description"]
        .as_str()
        .unwrap()
        .contains("call read_object as a tool"));
    assert_eq!(call(json!({"jsonrpc":"2.0","id":32,"method":"resources/read","params":{"uri":"anyblob://objects/a/b?consent=true"}}))["error"]["code"],-32602);
    assert_eq!(call(json!({"jsonrpc":"2.0","id":33,"method":"resources/read","params":{"uri":"anyblob://objects/a/b","_meta":{"threadId":"host-chat"}}}))["error"]["code"],-32000);
    assert_eq!(call(json!({"jsonrpc":"2.0","id":37,"method":"resources/read","params":{"uri":"mcp-resource://anyblob/anyblob%3A%2F%2Fobjects%2Fa%2Fb","_meta":{"threadId":"host-chat"}}}))["error"]["code"],-32000);
    let auth_error=call(json!({"jsonrpc":"2.0","id":38,"method":"tools/call","params":{"name":"authenticate_project","arguments":{"name":"Work","key":"secret-value","remote":"true"},"_meta":{"threadId":"host-chat"}}}));
    assert_eq!(auth_error["error"]["code"],-32602);
    let auth_message=auth_error["error"]["message"].as_str().unwrap();
    assert!(auth_message.contains("name:string, key:string, and remote:boolean"));
    assert!(!auth_message.contains("secret-value"));
    let valid_auth=call(json!({"jsonrpc":"2.0","id":39,"method":"tools/call","params":{"name":"authenticate_project","arguments":{"name":"Work","key":"secret-value","remote":true},"_meta":{"threadId":"host-chat"}}}));
    assert_eq!(valid_auth["result"]["isError"],true);
    assert_ne!(valid_auth["result"]["content"][0]["text"],"Unknown tool or invalid arguments.");
    for tool in tools["result"]["tools"].as_array().unwrap() {
        assert!(tool["annotations"]["readOnlyHint"].is_boolean());
    }
    assert_eq!(call(json!({"jsonrpc":"2.0","id":34,"method":"tools/call","params":{"name":"create_object","arguments":{"space_id":"s","body":{"type":"page","name":"fixture"}},"_meta":{"threadId":"host-chat"}}}))["result"]["isError"],true);
    assert_eq!(call(json!({"jsonrpc":"2.0","id":35,"method":"tools/call","params":{"name":"list_objects","arguments":{"space_id":"s","fields":"markdown"}}}))["error"]["code"],-32602);
    for (id, params) in [
        (4, json!({"name":"delete_object","arguments":{}})),
        (
            5,
            json!({"name":"read_object","arguments":{"space_id":"s","object_id":"o","path":"/v1"}}),
        ),
        (
            6,
            json!({"name":"search_objects","arguments":{"space_id":"s","query":"q","offset":-1}}),
        ),
    ] {
        assert_eq!(
            call(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":params}))["error"]
                ["code"],
            -32602
        );
    }
    assert_eq!(
        call(
            json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"list_spaces","arguments":{}}})
        )["result"]["isError"],
        true
    );
    assert_eq!(
        call(json!({"jsonrpc":"2.0","id":8,"method":"unknown"}))["error"]["code"],
        -32601
    );
    writeln!(input, "not json").unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap()["error"]["code"],
        -32700
    );
    drop(input);
    assert!(child.wait().unwrap().success());
}
