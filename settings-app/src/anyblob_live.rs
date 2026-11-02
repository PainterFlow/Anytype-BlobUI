//! Explicitly invoked live validation. Never runs in the default test suite.
use super::*;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
};
struct Runner {
    client: crate::kanban_api::Client,
    config: crate::codex::Configuration,
    space: String,
    path: PathBuf,
    ledger: Value,
}
impl Runner {
    fn save(&self) -> Result<(), String> {
        std::fs::write(&self.path, serde_json::to_vec_pretty(&self.ledger).unwrap())
            .map_err(|_| "Could not persist the live-test ledger.".into())
    }
    fn call(&mut self, name: &str, mut args: Value) -> Result<Value, String> {
        if args
            .get("space_id")
            .is_some_and(|id| id != &json!(self.space))
        {
            return Err("Live runner refused a different space.".into());
        }
        args["space_id"] = json!(self.space);
        if name == "upload_file" {
            let attempts = self.ledger["upload_attempts"]
                .as_u64()
                .ok_or("Invalid upload ledger.")?;
            if attempts + 2 > 10 {
                return Err("The persistent 10-attempt upload limit has been reached.".into());
            }
            let number = self.ledger["next_upload_num"]
                .as_u64()
                .ok_or("Invalid upload number.")?;
            let expected = format!("codex-test-{number}");
            if args["name"] != expected {
                return Err("Unexpected live upload filename.".into());
            }
            if let Some(path) = args["local_path"].as_str() {
                let meta = std::fs::metadata(path).map_err(|_| "Missing upload fixture.")?;
                if meta.len() >= 50_000 {
                    return Err("Live upload fixture must be under 50,000 bytes.".into());
                }
            }
            self.ledger["upload_attempts"] = json!(attempts + 2);
            self.ledger["next_upload_num"] = json!(number + 1);
            self.save()?; // Reserve both preflight and commit before either can reach Anytype.
        }
        if name.starts_with("delete_") {
            let id = args["object_id"]
                .as_str()
                .or_else(|| args["key"].as_str())
                .or_else(|| args["type"].as_str())
                .or_else(|| args["widget_id"].as_str())
                .or_else(|| args["message_id"].as_str())
                .ok_or("Missing deletion target.")?;
            if !self.ledger["created"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == id)
            {
                return Err("Live runner refused deletion of an unrecorded fixture.".into());
            }
        }
        let op = blobui_mcp::operation(name, args)?;
        let token = "a".repeat(64);
        let result = dispatch_resolved_with(
            Request {
                id: format!("live-{name}"),
                token: token.clone(),
                thread_id: None,
                request: op,
            },
            || true,
            || Ok(json!({"enabled":true,"verifier":digest(&token)})),
            || Ok(self.config.clone()),
            || Ok(self.client.clone_for_fixture()),
            |_, check| check(),
            || Ok(None),
        );
        self.ledger["calls"].as_array_mut().unwrap().push(json!({"tool":name,"outcome":result.as_ref().ok().and_then(|v|v["outcome"].as_str()).unwrap_or(if result.is_ok(){"read"}else{"failed"})}));
        self.save()?;
        let result = result?;
        if result.get("outcome").is_some() {
            if result["outcome"] != "confirmed" && result["outcome"] != "validated" {
                return Err(format!("Live {name} returned {}", result["outcome"]));
            }
            Ok(result["receipt"].clone())
        } else {
            Ok(result)
        }
    }
    fn track(&mut self, id: &str, kind: &str) -> Result<(), String> {
        self.ledger["created"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":id,"kind":kind,"cleaned":false}));
        self.save()
    }
    fn create(&mut self, name: &str, body: Value, kind: &str) -> Result<String, String> {
        let value = self.call(name, json!({"body":body}))?;
        let id = value["id"]
            .as_str()
            .ok_or("Create returned no fixture ID.")?
            .to_string();
        self.track(&id, kind)?;
        Ok(id)
    }
    fn patch(&mut self, id: &str, ops: Value) -> Result<Value, String> {
        let doc = self.call("read_document", json!({"object_id":id}))?;
        let etag = doc["etag"]
            .as_str()
            .ok_or("Document did not return an ETag.")?;
        self.call(
            "patch_object",
            json!({"object_id":id,"etag":etag,"body":{"ops":ops}}),
        )
    }
    fn cleanup(&mut self) -> Result<(), String> {
        let rows = self.ledger["created"].as_array().unwrap().clone();
        let mut errors = Vec::new();
        for (i, row) in rows.iter().enumerate().rev() {
            if row["cleaned"] == true {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            let result = match row["kind"].as_str().unwrap() {
                "object" => self.call("delete_object", json!({"object_id":id})),
                "property" => self.call("delete_property", json!({"key":id})),
                "type" => self.call("delete_type", json!({"type":id})),
                "widget" => self.call("delete_widget", json!({"widget_id":id})),
                "message" => continue,
                _ => continue,
            };
            match result {
                Ok(_) => {
                    self.ledger["created"][i]["cleaned"] = json!(true);
                    self.save()?;
                }
                Err(e) => errors.push(format!("{id}: {e}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Recorded fixture cleanup failures: {}",
                errors.join("; ")
            ))
        }
    }
}
#[test]
#[ignore = "Requires explicit Codex Test live validation authorization and local API connectivity"]
fn codex_test_content_live() -> Result<(), String> {
    if std::env::var("BLOBUI_ANYBLOB_LIVE_TEST").as_deref() != Ok("1") {
        return Err("Live validation requires its explicit environment opt-in.".into());
    }
    let client = crate::kanban_api::Client::connect()?;
    let spaces = client.pages("GET", "/v2/spaces?ids=full", None)?;
    let matches = spaces
        .iter()
        .filter(|s| s["name"] == "Codex Test")
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("Codex Test is missing or ambiguous. No mutation was dispatched.".into());
    }
    let space = matches[0]["id"]
        .as_str()
        .ok_or("Missing full space ID.")?
        .to_string();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".test-run/anyblob-live-content");
    crate::reject_reparse_component(&root)?;
    std::fs::create_dir_all(&root).map_err(|_| "Could not create workspace fixture directory.")?;
    let path = root.join("ledger.json");
    crate::reject_reparse_component(&path)?;
    let ledger = if path.exists() {
        serde_json::from_slice::<Value>(
            &std::fs::read(&path).map_err(|_| "Could not read persistent upload ledger.")?,
        )
        .map_err(|_| "Invalid persistent upload ledger.")?
    } else {
        json!({"space_id":space,"upload_attempts":0,"next_upload_num":1,"created":[],"calls":[]})
    };
    if ledger["space_id"] != space {
        return Err("The persisted Codex Test space ID changed. Refusing mutations.".into());
    }
    let config = crate::codex::normalize(
        &json!({"spaces":{space.clone():{"access":"createDelete","mutationsEnabled":true,"filesEnabled":true,"approval":"fullAccess"}}}),
    );
    let mut runner = Runner {
        client,
        config,
        space,
        path,
        ledger,
    };
    runner.save()?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let result = (|| {
        runner.call("list_schemas", json!({}))?;
        let object = runner.create(
            "create_object",
            json!({"type":"page","name":format!("codex-test-object-{nonce}")}),
            "object",
        )?;
        runner.patch(&object,json!([{"op":"set_properties","set":{"name":format!("codex-test-edited-{nonce}")}},{"op":"insert_blocks","markdown":"Codex isolated validation fixture."}]))?;
        let collection = runner.create(
            "create_collection",
            json!({"name":format!("codex-test-collection-{nonce}"),"items":[object]}),
            "object",
        )?;
        runner.call(
            "get_collection_objects",
            json!({"collection_id":collection}),
        )?;
        runner.call("get_collection_views", json!({"collection_id":collection}))?;
        let property_key = format!("codex_test_{nonce}");
        runner.call("create_property",json!({"body":{"key":property_key,"name":format!("Codex test {nonce}"),"format":"text"}}))?;
        runner.track(&property_key, "property")?;
        runner.call(
            "update_property",
            json!({"key":property_key,"body":{"name":format!("Codex test edited {nonce}")}}),
        )?;
        let chat = runner.create(
            "create_chat",
            json!({"name":format!("codex-test-chat-{nonce}")}),
            "object",
        )?;
        let message=runner.call("add_chat_message",json!({"chat_id":chat,"idempotency_key":format!("codex-message-{nonce}"),"body":{"text":"Codex Test validation message."}}))?;
        let message_id = message["id"]
            .as_str()
            .ok_or("Missing test message ID.")?
            .to_string();
        runner.track(&message_id, "message")?;
        runner.call("edit_chat_message",json!({"chat_id":chat,"message_id":message_id,"body":{"text":"Edited Codex Test validation message."}}))?;
        runner.call(
            "toggle_chat_reaction",
            json!({"chat_id":chat,"message_id":message_id,"body":{"emoji":"👍"}}),
        )?;
        runner.call("get_chat_messages", json!({"chat_id":chat,"limit":10}))?;
        runner.call(
            "delete_chat_message",
            json!({"chat_id":chat,"message_id":message_id}),
        )?;
        for remote in [false, true] {
            let number = runner.ledger["next_upload_num"].as_u64().unwrap();
            let name = format!("codex-test-{number}");
            let bytes = vec![b'x'; 1024];
            let mut server = None;
            let args = if remote {
                let listener = TcpListener::bind("127.0.0.1:0")
                    .map_err(|_| "Could not bind controlled URL fixture.")?;
                listener.set_nonblocking(true).unwrap();
                let port = listener.local_addr().unwrap().port();
                server = Some(std::thread::spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(20);
                    let mut served = 0;
                    while served < 2 && Instant::now() < deadline {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                                let mut buffer = [0u8; 4096];
                                let _ = stream.read(&mut buffer);
                                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 1024\r\nConnection: close\r\n\r\n");
                                let _ = stream.write_all(&bytes);
                                served += 1;
                            }
                            Err(_) => std::thread::sleep(Duration::from_millis(10)),
                        }
                    }
                }));
                json!({"url":format!("http://127.0.0.1:{port}/{name}"),"name":name})
            } else {
                let path = root.join(&name);
                std::fs::write(&path, bytes).map_err(|_| "Could not write upload fixture.")?;
                json!({"local_path":path,"name":name})
            };
            let uploaded = runner.call("upload_file", args);
            if let Some(server) = server {
                let _ = server.join();
            }
            let uploaded = uploaded?;
            let file = uploaded["id"]
                .as_str()
                .ok_or("Missing uploaded file ID.")?
                .to_string();
            runner.track(&file, "object")?;
            runner.patch(
                &object,
                json!([{"op":"insert_blocks","blocks":[{"type":"file","file":file}]}]),
            )?;
        }
        Ok(())
    })();
    let cleanup = runner.cleanup();
    eprintln!(
        "Codex Test validation reserved {} of 10 upload attempts. Cleanup success={}",
        runner.ledger["upload_attempts"],
        cleanup.is_ok()
    );
    result.and(cleanup)
}
