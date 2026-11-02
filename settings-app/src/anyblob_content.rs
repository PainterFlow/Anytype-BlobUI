//! Authorized content operations. Routes come exclusively from the shared catalog.
use crate::{codex::Permission, kanban_api::Client};
use blobui_mcp::content::{validate_schema, ContentRequest};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};
const MAX_UPLOAD: u64 = 25 * 1024 * 1024;
pub struct Prepared {
    pub request: ContentRequest,
    file: Option<File>,
    path: Option<PathBuf>,
    size: u64,
    name: String,
    hash: String,
    pub files: bool,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        self.file.take();
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}
pub fn permission(
    config: &crate::codex::Configuration,
    scope: Option<&crate::anyblob_scope::Scope>,
    request: &ContentRequest,
    files: bool,
) -> Result<Permission, String> {
    let permission = if let Some(scope) = scope {
        scope.content_permission(request, files)
    } else {
        config
            .spaces
            .get(
                request.arguments["space_id"]
                    .as_str()
                    .ok_or("Missing space ID.")?,
            )
            .cloned()
    };
    permission.filter(|p|p.permits(request.capability())&&(!files||p.files_enabled)).ok_or_else(||"Access denied by saved BlobUI content permissions. Save the required access level; file actions also require the file switch.".into())
}
pub fn needs_files(value: &Value) -> bool {
    match value {
        Value::Object(o) => o.iter().any(|(k, v)| {
            matches!(
                k.as_str(),
                "attachments" | "icon_image" | "file" | "file_id"
            ) || (k == "markdown" && v.as_str().is_some_and(|text| text.contains("![")))
                || (matches!(k.as_str(), "type" | "format")
                    && matches!(
                        v.as_str(),
                        Some("file" | "files" | "image" | "audio" | "video" | "pdf")
                    ))
                || needs_files(v)
        }),
        Value::Array(a) => a.iter().any(needs_files),
        _ => false,
    }
}
fn normalize_property(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn response(value: Value) -> Result<Value, String> {
    if value.get("_http_error").is_some() {
        Err(format!("Anytype confirmed refusal. {}", value))
    } else {
        Ok(value)
    }
}
fn request(
    client: &Client,
    method: &str,
    path: &str,
    body: Option<&Value>,
    etag: Option<&str>,
    key: Option<&str>,
) -> Result<Value, String> {
    response(client.content_request(method, path, body, etag, key, None)?)
}
fn route(req: &ContentRequest, dry: bool) -> Result<String, String> {
    let spec = req.specification()?;
    let mut route = spec["route"].as_str().ok_or("Missing route.")?.to_string();
    for (key, value) in req.arguments.as_object().unwrap() {
        if let Some(value) = value.as_str() {
            route = route.replace(&format!("{{{key}}}"), &crate::api::encode_segment(value));
        }
    }
    if route.contains(['{', '}']) {
        return Err("Missing route identifier.".into());
    }
    let mut query = Vec::new();
    for key in spec["query"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
    {
        if key == "ids" {
            query.push("ids=full".into());
            continue;
        }
        if key == "dry_run" {
            if dry || req.arguments[key] == true {
                query.push("dry_run=true".into());
            }
            continue;
        }
        if let Some(value) = req.arguments.get(key) {
            let text = value
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| value.to_string());
            query.push(format!("{}={}", key, crate::api::encode_segment(&text)));
        }
    }
    if !query.is_empty() {
        route.push('?');
        route.push_str(&query.join("&"));
    }
    Ok(route)
}
impl Prepared {
    pub fn prompt(&self) -> blobui_mcp::Operation {
        let mut content = self.request.clone();
        if self.request.irreversible() {
            content.arguments["irreversible_effect"]=json!("This action cannot be restored from Bin. Deleting a message can also delete its attachment content.");
        }
        if self.file.is_some() {
            content.arguments["upload_snapshot"] =
                json!({"name":self.name,"bytes":self.size,"sha256":self.hash});
        }
        blobui_mcp::Operation::Content { content }
    }
}
pub fn prepare(
    _client: &Client,
    req: &ContentRequest,
    config: &crate::codex::Configuration,
    scope: Option<&crate::anyblob_scope::Scope>,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Prepared, String> {
    req.validate()?;
    let files = req.tool == "upload_file" || needs_files(&req.arguments["body"]);
    permission(config, scope, req, files)?;
    check()?;
    let mut prepared = Prepared {
        request: req.clone(),
        file: None,
        path: None,
        size: 0,
        name: String::new(),
        hash: String::new(),
        files,
    };
    if let Some(local) = req.arguments["local_path"].as_str() {
        let path = PathBuf::from(local);
        if !path.is_absolute()
            || local.starts_with("\\\\")
            || local.starts_with("//")
            || local.contains(':') && local.chars().filter(|c| *c == ':').count() != 1
        {
            return Err("Upload requires an absolute regular local path without device, network or alternate stream syntax.".into());
        }
        for component in path.components() {
            let text = component.as_os_str().to_string_lossy();
            let stem = text.split('.').next().unwrap_or("").to_ascii_uppercase();
            if [
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9",
            ]
            .contains(&stem.as_str())
            {
                return Err("Device filenames cannot be uploaded.".into());
            }
        }
        crate::reject_reparse_component(&path)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1).custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
        }
        let mut source = options
            .open(&path)
            .map_err(|_| "Could not open the selected upload file.")?;
        let meta = source
            .metadata()
            .map_err(|_| "Could not inspect upload file.")?;
        if !meta.is_file() || meta.len() > MAX_UPLOAD {
            return Err("Upload requires a regular file no larger than 25 MiB.".into());
        }
        let directory = std::env::current_exe()
            .map_err(|_| "Could not locate upload staging directory.")?
            .parent()
            .ok_or("No staging parent.")?
            .join("anyblob-upload-staging");
        crate::reject_reparse_component(&directory)?;
        std::fs::create_dir_all(&directory)
            .map_err(|_| "Could not create upload staging directory.")?;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| "Could not create upload snapshot ID.")?;
        let name = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let snapshot = directory.join(name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        prepared.file = Some(
            options
                .open(&snapshot)
                .map_err(|_| "Could not stage upload snapshot.")?,
        );
        prepared.path = Some(snapshot);
        prepared.name = req.arguments["name"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        let target = prepared.file.as_mut().unwrap();
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            check()?;
            let n = source
                .read(&mut buffer)
                .map_err(|_| "Could not read upload file.")?;
            if n == 0 {
                break;
            }
            prepared.size += n as u64;
            if prepared.size > MAX_UPLOAD {
                return Err("Upload exceeds 25 MiB.".into());
            }
            digest.update(&buffer[..n]);
            target
                .write_all(&buffer[..n])
                .map_err(|_| "Could not write upload snapshot.")?;
        }
        target
            .flush()
            .map_err(|_| "Could not finish upload snapshot.")?;
        target
            .seek(SeekFrom::Start(0))
            .map_err(|_| "Could not rewind upload snapshot.")?;
        prepared.hash = format!("{:x}", digest.finalize());
    }
    check()?;
    Ok(prepared)
}
pub fn inspect(
    client: &Client,
    prepared: &mut Prepared,
    config: &crate::codex::Configuration,
    scope: Option<&crate::anyblob_scope::Scope>,
    check: &impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    let req = &prepared.request;
    let mut files = prepared.files;
    if req.mutates() && matches!(req.tool.as_str(), "patch_object" | "delete_object") {
        let path = format!(
            "/v2/spaces/{}/objects/{}?ids=full",
            crate::api::encode_segment(req.arguments["space_id"].as_str().unwrap()),
            crate::api::encode_segment(req.arguments["object_id"].as_str().unwrap())
        );
        check()?;
        let doc = request(client, "GET", &path, None, None, None)?;
        if matches!(
            doc["kind"].as_str(),
            Some("file" | "files" | "image" | "audio" | "video" | "pdf")
        ) || matches!(
            doc["type"].as_str(),
            Some("file" | "files" | "image" | "audio" | "video" | "pdf")
        ) || matches!(
            doc["layout"].as_str(),
            Some("file" | "files" | "image" | "audio" | "video" | "pdf")
        ) {
            files = true;
        }
        if matches!(
            doc["kind"].as_str(),
            Some("objectType" | "property" | "type")
        ) {
            let mut wide = req.clone(); // Schema documents affect the space, regardless of their object membership.
            wide.tool = if req.capability() == "createDelete" {
                "create_object"
            } else {
                "update_property"
            }
            .into();
            wide.arguments["type"] = json!("permission-check");
            permission(config, scope, &wide, files)?;
        }
    }
    if req.mutates() && req.arguments.get("body").is_some() {
        // Custom file-valued properties can have arbitrary names. Resolve their format internally.
        let body = &req.arguments["body"];
        let sets = body
            .get("properties")
            .into_iter()
            .chain(
                body["ops"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|op| op["op"] == "set_properties")
                    .filter_map(|op| op.get("set")),
            )
            .collect::<Vec<_>>();
        if !sets.is_empty() {
            let path = format!(
                "/v2/spaces/{}/properties?ids=full",
                crate::api::encode_segment(req.arguments["space_id"].as_str().unwrap())
            );
            check()?;
            let properties = client.pages("GET", &path, None)?;
            for set in sets {
                if let Some(set) = set.as_object() {
                    for prop in &properties {
                        if matches!(prop["format"].as_str(), Some("file" | "files"))
                            && ["key", "name"].iter().any(|key| {
                                prop[*key].as_str().is_some_and(|key| {
                                    set.keys().any(|candidate| {
                                        normalize_property(candidate) == normalize_property(key)
                                    })
                                })
                            })
                        {
                            files = true;
                        }
                    }
                }
            }
        }
    }
    permission(config, scope, req, files)?;

    if req.tool == "delete_chat_message" {
        // Message deletion can destroy attachment content. Inspect before committing, and
        // fail closed if pagination cannot establish the target's attachments.
        let base = format!(
            "/v2/spaces/{}/chats/{}/messages?limit=100",
            crate::api::encode_segment(req.arguments["space_id"].as_str().unwrap()),
            crate::api::encode_segment(req.arguments["chat_id"].as_str().unwrap())
        );
        let mut before = String::new();
        let mut found = false;
        for _ in 0..100 {
            check()?;
            let page = request(client, "GET", &format!("{base}{before}"), None, None, None)?;
            let messages = page["messages"].as_array().ok_or("Missing message page.")?;
            if let Some(message) = messages
                .iter()
                .find(|m| m["id"] == req.arguments["message_id"])
            {
                if message["attachments"]
                    .as_array()
                    .is_some_and(|a| !a.is_empty())
                {
                    files = true;
                }
                found = true;
                break;
            }
            if page["has_more"] == false {
                break;
            }
            let cursor = page["next_before"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Missing message cursor.")?;
            let next = format!("&before={}", crate::api::encode_segment(cursor));
            if next == before {
                return Err("Message pagination did not advance.".into());
            }
            before = next;
        }
        if !found {
            return Err("Could not establish the target message's attachment access. No deletion was dispatched.".into());
        }
    }
    permission(config, scope, req, files)?;
    prepared.files = files;
    check()?;
    Ok(())
}
pub fn execute(
    client: &Client,
    prepared: &mut Prepared,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    let req = &prepared.request;
    let spec = req.specification()?;
    let method = spec["method"].as_str().unwrap();
    let body = req.arguments.get("body");
    let etag = req.arguments["etag"].as_str();
    // Discover authoritative operation shapes before applying a document patch.
    if req.tool == "patch_object" {
        for op in req.arguments["body"]["ops"].as_array().unwrap() {
            check()?;
            let schema = request(
                client,
                "GET",
                &format!(
                    "/v2/schemas/ops/{}",
                    crate::api::encode_segment(op["op"].as_str().unwrap())
                ),
                None,
                None,
                None,
            )?;
            if !schema["schema"].is_object() {
                return Err("Connected Anytype did not supply the operation schema.".into());
            }
            validate_schema(&schema["schema"], op, "body/ops")?;
        }
    }
    if !req.mutates() {
        check()?;
        return request(client, method, &route(req, false)?, body, etag, None);
    }
    let key = req.arguments["idempotency_key"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| {
            let mut random = [0u8; 16];
            if getrandom::fill(&mut random).is_err() {
                return String::new();
            }
            random.iter().map(|b| format!("{b:02x}")).collect()
        });
    if key.is_empty() {
        return Err("Could not generate mutation retry key.".into());
    }
    let url_body = if req.tool == "upload_file" && req.arguments["url"].is_string() {
        let mut body = json!({"url":req.arguments["url"]});
        if req.arguments["name"].is_string() {
            body["name"] = req.arguments["name"].clone();
        }
        Some(body)
    } else {
        None
    };
    let body = url_body.as_ref().or(body);
    let dry = req.arguments["dry_run"] == true;
    // Server dry runs validate current semantics and advertised limits. They do not reserve state.
    if spec["query"]
        .as_array()
        .unwrap()
        .iter()
        .any(|q| q == "dry_run")
    {
        check()?;
        let file = prepared
            .file
            .as_mut()
            .map(|f| (f, prepared.size, prepared.name.as_str()));
        let result = response(client.content_request(
            method,
            &route(req, true)?,
            body,
            etag,
            None,
            file,
        )?)?;
        if dry {
            return Ok(json!({"outcome":"validated","receipt":result,"idempotency_key":key}));
        }
        if let Some(file) = prepared.file.as_mut() {
            file.seek(SeekFrom::Start(0))
                .map_err(|_| "Could not rewind staged upload.")?;
        }
    } else if dry {
        return Err("This operation does not support dry_run.".into());
    }
    check()?;
    let file = prepared
        .file
        .as_mut()
        .map(|f| (f, prepared.size, prepared.name.as_str()));
    let result = client.content_request(method, &route(req, false)?, body, etag, Some(&key), file);
    match result {
        Ok(value) if value.get("_http_error").is_some() => {
            Ok(json!({"outcome":"rejected","receipt":value,"idempotency_key":key}))
        }
        Ok(value) => Ok(json!({"outcome":"confirmed","receipt":value,"idempotency_key":key})),
        Err(error) if error.starts_with("Mutation outcome unknown") => Ok(
            json!({"outcome":"unknown","error":error,"idempotency_key":key,"retry_automatically":false}),
        ),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::TcpListener,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    fn config(access: &str, opt_in: bool, files: bool) -> crate::codex::Configuration {
        crate::codex::normalize(
            &json!({"spaces":{"s":{"access":access,"mutationsEnabled":opt_in,"filesEnabled":files,"approval":"fullAccess"}}}),
        )
    }
    fn content(name: &str, args: Value) -> ContentRequest {
        let blobui_mcp::Operation::Content { content } = blobui_mcp::operation(name, args).unwrap()
        else {
            panic!()
        };
        content
    }
    fn prepared(req: ContentRequest) -> Prepared {
        Prepared {
            request: req,
            file: None,
            path: None,
            size: 0,
            name: String::new(),
            hash: String::new(),
            files: false,
        }
    }
    fn fixture(
        responses: Vec<Value>,
    ) -> (
        Client,
        Arc<Mutex<Vec<Vec<u8>>>>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let capture = requests.clone();
        let server = std::thread::spawn(move || {
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(8);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "Missing fixture request");
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]);
                        let len = header
                            .lines()
                            .find_map(|l| l.strip_prefix("Content-Length: "))
                            .map(|n| n.parse::<usize>().unwrap())
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                capture.lock().unwrap().push(bytes);
                if response == json!("disconnect") {
                    continue;
                }
                let status = response["fixture_status"].as_u64().unwrap_or(200);
                let text = response.to_string();
                write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nETag: \"MixedCaseETag\"\r\nConnection: close\r\n\r\n{text}",text.len()).unwrap();
            }
        });
        (
            Client::with_connection("fixture-key", "127.0.0.1", port),
            requests,
            server,
        )
    }
    #[test]
    fn access_levels_work_without_content_opt_in() {
        for access in ["none", "read", "readWrite", "createDelete"] {
            let old = config(access, false, true);
            assert_eq!(old.spaces["s"].permits("write"), matches!(access, "readWrite" | "createDelete"));
            assert_eq!(old.spaces["s"].permits("createDelete"), access == "createDelete");
            assert_eq!(
                old.spaces["s"].permits("read"),
                matches!(access, "read" | "readWrite" | "createDelete")
            );
        }
        let read = config("read", true, true);
        assert!(!read.spaces["s"].permits("write"));
        let write = config("readWrite", true, true);
        assert!(write.spaces["s"].permits("write"));
        assert!(!write.spaces["s"].permits("createDelete"));
        let full = config("createDelete", true, true);
        for capability in ["read", "write", "createDelete"] {
            assert!(full.spaces["s"].permits(capability));
        }
        let upload = content(
            "upload_file",
            json!({"space_id":"s","url":"https://example.com/fixture"}),
        );
        assert!(permission(&config("createDelete", true, false), None, &upload, true).is_err());
        assert!(permission(&full, None, &upload, true).is_ok());
    }
    #[test]
    fn edits_preserve_etags_preflight_and_receipts() {
        let (client, capture, server) = fixture(vec![
            json!({"schema":{"type":"object","required":["op","set"]}}),
            json!({"dry_run":true}),
            json!({"etag":"new","created_blocks":{"ops[0]":"block"},"warnings":[{"message":"kept"}]}),
        ]);
        let mut prepared = prepared(content(
            "patch_object",
            json!({"space_id":"s","object_id":"o","etag":"\"OriginalETag\"","idempotency_key":"stable-key","body":{"ops":[{"op":"set_properties","set":{"name":"updated"}}]}}),
        ));
        let result = execute(&client, &mut prepared, &|| Ok(())).unwrap();
        server.join().unwrap();
        assert_eq!(result["outcome"], "confirmed");
        assert_eq!(result["receipt"]["warnings"][0]["message"], "kept");
        let captured = capture.lock().unwrap();
        let preflight = String::from_utf8_lossy(&captured[1]);
        let commit = String::from_utf8_lossy(&captured[2]);
        assert!(preflight.contains("dry_run=true"));
        assert!(commit.contains("If-Match: \"OriginalETag\""));
        assert!(commit.contains("Idempotency-Key: stable-key"));
        assert!(!commit.contains("dry_run=true"));
        assert_eq!(captured.len(), 3);
    }
    #[test]
    fn stale_etag_and_cancellation_never_commit() {
        let (client, capture, server) = fixture(vec![
            json!({"schema":{"type":"object"}}),
            json!({"fixture_status":409,"code":"conflict","etag":"current"}),
        ]);
        let mut prepared = prepared(content(
            "patch_object",
            json!({"space_id":"s","object_id":"o","etag":"old","body":{"ops":[{"op":"delete_block","id":"b"}]}}),
        ));
        assert!(execute(&client, &mut prepared, &|| Ok(()))
            .unwrap_err()
            .contains("conflict"));
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
        assert!(execute(&client, &mut prepared, &|| Err("cancelled".into()))
            .unwrap_err()
            .contains("cancelled"));
    }
    #[test]
    fn unknown_sends_are_reported_without_retries() {
        let (client, capture, server) = fixture(vec![json!({"dry_run":true}), json!("disconnect")]);
        let mut prepared = prepared(content(
            "add_chat_message",
            json!({"space_id":"s","chat_id":"c","idempotency_key":"message-key","body":{"text":"fixture"}}),
        ));
        let result = execute(&client, &mut prepared, &|| Ok(())).unwrap();
        server.join().unwrap();
        assert_eq!(result["outcome"], "unknown");
        assert_eq!(result["retry_automatically"], false);
        assert_eq!(result["idempotency_key"], "message-key");
        assert_eq!(capture.lock().unwrap().len(), 2);
    }
    #[test]
    fn upload_snapshot_and_multipart_preserve_unicode_and_bytes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run/anyblob-content-tests");
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("codex-test-1");
        let bytes = (0..1024).map(|i| (i % 256) as u8).collect::<Vec<_>>();
        std::fs::write(&source, &bytes).unwrap();
        let (client, capture, server) =
            fixture(vec![json!({"dry_run":true}), json!({"id":"file-fixture"})]);
        let req = content(
            "upload_file",
            json!({"space_id":"s","local_path":source,"name":"codex-test-1-猫"}),
        );
        let mut staged = prepare(
            &client,
            &req,
            &config("createDelete", true, true),
            None,
            &|| Ok(()),
        )
        .unwrap();
        let snapshot = staged.path.clone().unwrap();
        assert!(snapshot.exists());
        assert_eq!(staged.size, 1024);
        assert_eq!(staged.prompt().name(), "upload_file");
        std::fs::write(&source, b"changed after snapshot").unwrap();
        assert_eq!(
            execute(&client, &mut staged, &|| Ok(())).unwrap()["outcome"],
            "confirmed"
        );
        server.join().unwrap();
        for request in capture.lock().unwrap().iter() {
            assert!(request.windows(bytes.len()).any(|w| w == bytes));
            assert!(String::from_utf8_lossy(request).contains("codex-test-1-猫"));
        }
        drop(staged);
        assert!(!snapshot.exists());
        std::fs::remove_file(source).unwrap();
    }
    #[test]
    fn message_deletion_requires_file_access_for_attachments() {
        let (client, capture, server) = fixture(vec![
            json!({"messages":[{"id":"m","attachments":[{"id":"f"}]}],"has_more":false}),
        ]);
        let mut prepared = prepared(content(
            "delete_chat_message",
            json!({"space_id":"s","chat_id":"c","message_id":"m"}),
        ));
        assert!(inspect(
            &client,
            &mut prepared,
            &config("createDelete", true, false),
            None,
            &|| Ok(())
        )
        .is_err());
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 1);
    }
    #[test]
    fn every_content_family_uses_its_fixed_http_route() {
        for spec in blobui_mcp::content::catalog() {
            let name = spec["name"].as_str().unwrap();
            if matches!(name, "patch_object" | "upload_file") {
                continue;
            }
            let mut args = super::catalog_tests::sample(&spec["schema"]);
            if name == "update_widget" {
                args["body"] = json!({"layout":"link"});
            }
            let req = content(name, args);
            let mutation = req.mutates();
            let preflight = mutation
                && spec["query"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|q| q == "dry_run");
            let count = if preflight { 2 } else { 1 };
            let (client, capture, server) =
                fixture(vec![json!({"id":"fixture","warnings":[]}); count]);
            let expected = format!(
                "{} {} HTTP/1.1",
                spec["method"].as_str().unwrap(),
                route(&req, false).unwrap()
            );
            let result = execute(&client, &mut prepared(req), &|| Ok(())).unwrap();
            server.join().unwrap();
            let capture = capture.lock().unwrap();
            assert_eq!(capture.len(), count, "{name}");
            assert!(
                String::from_utf8_lossy(capture.last().unwrap()).starts_with(&expected),
                "{name}"
            );
            if mutation {
                assert_eq!(result["outcome"], "confirmed");
            }
        }
    }
    #[test]
    fn markdown_images_require_file_permission() {
        assert!(needs_files(&json!({"markdown":"![image](anytype://file)"})));
        assert!(!needs_files(&json!({"markdown":"plain text"})));
    }
    #[test]
    fn custom_file_properties_require_file_permission() {
        let (client, capture, server) = fixture(vec![
            json!({"id":"o","kind":"object","type":"page"}),
            json!({"data":[{"key":"invoice_file","name":"Invoice file","format":"files"}],"has_more":false}),
        ]);
        let mut prepared = prepared(content(
            "patch_object",
            json!({"space_id":"s","object_id":"o","etag":"e","body":{"ops":[{"op":"set_properties","set":{"invoiceFile":["f"]}}]}}),
        ));
        assert!(inspect(
            &client,
            &mut prepared,
            &config("readWrite", true, false),
            None,
            &|| Ok(())
        )
        .is_err());
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
    }
    #[test]
    fn local_upload_rejects_unsafe_sources_before_dispatch() {
        let client = Client::with_connection("fixture-key", "127.0.0.1", 1);
        for path in [
            "relative.txt",
            "\\\\server\\share\\file",
            "\\\\?\\C:\\secret",
            "C:\\file:stream",
            "C:\\NUL.txt",
        ] {
            let req = content("upload_file", json!({"space_id":"s","local_path":path}));
            assert!(prepare(
                &client,
                &req,
                &config("createDelete", true, true),
                None,
                &|| Ok(())
            )
            .is_err());
        }
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    pub(super) fn sample(schema: &Value) -> Value {
        if let Some(value) = schema.get("const") {
            return value.clone();
        }
        if let Some(values) = schema["enum"].as_array() {
            return values[0].clone();
        }
        for key in ["anyOf", "oneOf"] {
            if let Some(values) = schema[key].as_array() {
                return sample(&values[0]);
            }
        }
        match schema["type"].as_str().unwrap_or("object") {
            "string" => json!("fixture"),
            "integer" | "number" => json!(1),
            "boolean" => json!(false),
            "array" => json!([sample(&schema["items"])]),
            _ => {
                let mut body = json!({});
                for key in schema["required"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    body[key] = sample(&schema["properties"][key]);
                }
                body
            }
        }
    }
    #[test]
    fn every_tool_accepts_typed_arguments_and_round_trips_the_bridge_contract() {
        for spec in blobui_mcp::content::catalog() {
            let name = spec["name"].as_str().unwrap();
            let mut args = sample(&spec["schema"]);
            if name == "upload_file" {
                args["url"] = json!("https://example.com/codex-test-1");
            }
            if name == "patch_object" {
                args["body"] = json!({"ops":[{"op":"set_properties","set":{"name":"fixture"}}]});
            }
            if name == "update_widget" {
                args["body"] = json!({"layout":"link"});
            }
            let op = blobui_mcp::operation(name, args.clone())
                .unwrap_or_else(|e| panic!("{name}: {e} with {args}"));
            let blobui_mcp::Operation::Content { content } = op else {
                panic!()
            };
            let bytes = serde_json::to_vec(&blobui_mcp::Operation::Content {
                content: content.clone(),
            })
            .unwrap();
            let decoded: blobui_mcp::Operation = serde_json::from_slice(&bytes).unwrap();
            decoded.validate().unwrap();
            let path = route(&content, false).unwrap();
            assert!(path.starts_with("/v2/"));
            assert!(!path.contains(['{', '}', '\r', '\n']));
            let mut forged = args;
            forged["headers"] = json!({"Authorization":"forged"});
            assert!(blobui_mcp::operation(name, forged).is_err(), "{name}");
        }
    }
}
