use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
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
    assert_eq!(
        call(
            json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})
        )["result"]["serverInfo"]["name"],
        "anyblob"
    );
    let tools = call(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}));
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 45);
    let mentions=&tools["result"]["tools"][0];
    assert_eq!(mentions["name"],"search_mentions");
    assert_eq!(mentions["_meta"]["ui"]["visibility"],json!(["app"]));
    assert_eq!(mentions["outputSchema"]["properties"]["items"]["maxItems"],20);
    let initialized=call(json!({"jsonrpc":"2.0","id":30,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}));
    let icon = &initialized["result"]["serverInfo"]["icons"][0];
    assert_eq!(icon["src"], include_str!("../src/icon-data-uri.txt"));
    assert_eq!(icon["mimeType"], "image/png");
    assert_eq!(icon["sizes"], json!(["256x256"]));
    assert_eq!(initialized["result"]["capabilities"]["extensions"]["openai/mentions"]["searchTool"],"search_mentions");
    assert_eq!(call(json!({"jsonrpc":"2.0","id":31,"method":"resources/list"}))["result"]["resources"],json!([]));
    assert_eq!(call(json!({"jsonrpc":"2.0","id":32,"method":"resources/read","params":{"uri":"anyblob://objects/a/b?consent=true"}}))["error"]["code"],-32602);
    assert_eq!(call(json!({"jsonrpc":"2.0","id":33,"method":"resources/read","params":{"uri":"anyblob://objects/a/b","_meta":{"threadId":"host-chat"}}}))["error"]["code"],-32000);
    for tool in tools["result"]["tools"].as_array().unwrap() {
        assert!(tool["annotations"]["readOnlyHint"].is_boolean());
        if tool["name"]=="patch_object" {assert_eq!(tool["annotations"]["readOnlyHint"],false);assert!(tool["inputSchema"]["required"].as_array().unwrap().contains(&json!("etag")));}
    }
    assert_eq!(call(json!({"jsonrpc":"2.0","id":34,"method":"tools/call","params":{"name":"create_object","arguments":{"space_id":"s","body":{"type":"page","name":"fixture"}},"_meta":{"threadId":"host-chat"}}}))["result"]["isError"],true);
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
