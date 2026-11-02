#![cfg(windows)]
use blobui_mcp::{
    approval::{Frame, VERSION},
    transport::{Pipe, CALL_TIMEOUT},
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
fn receive(out: &mut impl BufRead) -> Value {
    let mut s = String::new();
    out.read_line(&mut s).unwrap();
    serde_json::from_str(&s).unwrap()
}
fn send(input: &mut impl Write, v: Value) {
    writeln!(input, "{v}").unwrap();
    input.flush().unwrap();
}
#[test]
fn native_forms_fail_closed_and_correlate_concurrent_calls() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.test-run/elicitation");
    std::fs::create_dir_all(&root).unwrap();
    let credential = root.join("fixture.dpapi");
    std::fs::write(
        &credential,
        blobui_mcp::credential::encrypt(&"a".repeat(64)).unwrap(),
    )
    .unwrap();
    let instance = format!("fixture-forms-{}", std::process::id());
    std::env::set_var("ANYBLOB_BRIDGE_INSTANCE", &instance);
    let mut listener = Pipe::listen(true).unwrap();
    let bridge = std::thread::spawn(move || {
        let mut workers = Vec::new();
        for _ in 0..10 {
            let deadline = Instant::now() + CALL_TIMEOUT;
            while !listener.accept().unwrap() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            let pipe = std::mem::replace(&mut listener, Pipe::listen(false).unwrap());
            workers.push(std::thread::spawn(move||{
 let cancel=AtomicBool::new(false);
 let envelope:Value=serde_json::from_slice(&pipe.receive(&cancel,deadline).unwrap()).unwrap();assert_eq!(envelope["version"],VERSION);
 let id=envelope["request"]["id"].as_str().unwrap().to_string();let aid=format!("approval-{id}");
 let question=Frame::ApprovalRequest{version:VERSION,operation_id:id.clone(),approval_id:aid.clone(),message:format!("Allow fixture operation {id}?")};
 pipe.send(&serde_json::to_vec(&question).unwrap(),&cancel,deadline).unwrap();
 let accepted=pipe.receive(&cancel,deadline).ok().and_then(|b|serde_json::from_slice::<Frame>(&b).ok()).is_some_and(|f|matches!(f,Frame::ApprovalDecision{version,operation_id,approval_id,accepted:true} if version==VERSION && operation_id==id && approval_id==aid));
 let response=Frame::Response{version:VERSION,response:blobui_mcp::Response{result:Some(json!({"executions":if accepted{1}else{0}})),error:None}};
 let _=pipe.send(&serde_json::to_vec(&response).unwrap(),&cancel,deadline);accepted
 }));
        }
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .filter(|v| *v)
            .count()
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_blobui-mcp"))
        .env("ANYBLOB_BRIDGE_INSTANCE", &instance)
        .env("ANYBLOB_CREDENTIAL_FILE", credential)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{"elicitation":{"form":{}}}}}),
    );
    receive(&mut out);
    for (i, result) in [
        json!({"action":"accept","content":{"confirm":true}}),
        json!({"action":"accept","content":{"confirm":false}}),
        json!({"action":"decline","content":{"confirm":true}}),
        json!({"action":"cancel"}),
        json!({"action":"accept","content":{"confirm":"true"}}),
        json!({"action":"accept","content":{"confirm":true,"extra":true}}),
    ]
    .into_iter()
    .enumerate()
    {
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":i+10,"method":"tools/call","params":{"name":"read_object","arguments":{"space_id":"s","object_id":"o"}}}),
        );
        let question = receive(&mut out);
        assert_eq!(question["method"], "elicitation/create");
        assert_eq!(
            question["params"]["requestedSchema"]["properties"]["confirm"]["default"],
            false
        );
        // An unrelated response cannot answer the pending question.
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":"stale","result":{"action":"accept","content":{"confirm":true}}}),
        );
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":question["id"],"result":result}),
        );
        let reply = receive(&mut out);
        assert_eq!(reply["id"], i + 10);
        assert_eq!(
            reply["result"]["structuredContent"]["executions"],
            if i == 0 { 1 } else { 0 }
        );
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":question["id"],"result":{"action":"accept","content":{"confirm":true}}}),
        );
    }
    // Two simultaneous calls have independent server request IDs. Answer in reverse order.
    for id in [30, 31] {
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"read_object","arguments":{"space_id":"s","object_id":"o"}}}),
        );
    }
    let first = receive(&mut out);
    let second = receive(&mut out);
    assert_ne!(first["id"], second["id"]);
    for question in [second, first] {
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":question["id"],"result":{"action":"accept","content":{"confirm":true}}}),
        );
    }
    for _ in 0..2 {
        assert_eq!(
            receive(&mut out)["result"]["structuredContent"]["executions"],
            1
        );
    }
    // Cancellation closes the pending bridge without a decision.
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":40,"method":"tools/call","params":{"name":"read_object","arguments":{"space_id":"s","object_id":"o"}}}),
    );
    let question = receive(&mut out);
    assert_eq!(question["method"], "elicitation/create");
    send(
        &mut input,
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":40}}),
    );
    let closed = receive(&mut out);
    assert_eq!(closed["method"], "notifications/cancelled");
    assert_eq!(receive(&mut out)["error"]["code"], -32800);
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":50,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}),
    );
    receive(&mut out);
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":51,"method":"tools/call","params":{"name":"read_object","arguments":{"space_id":"s","object_id":"o"}}}),
    );
    let reply = receive(&mut out);
    assert_eq!(reply["result"]["isError"], true);
    assert!(reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Update Codex"));
    drop(input);
    assert!(child.wait().unwrap().success());
    assert_eq!(bridge.join().unwrap(), 3);
}
