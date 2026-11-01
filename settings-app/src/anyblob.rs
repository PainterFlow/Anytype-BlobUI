//! Anyblob's trust boundary. Only saved settings authorize reads.
use blobui_mcp::{Operation, Request, Response};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::Emitter;
static PROMPT: Mutex<()> = Mutex::new(());
static DECISION: Mutex<Option<(String, Option<bool>)>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);
static REVISION: AtomicU64 = AtomicU64::new(0);
pub fn invalidate() {
    REVISION.fetch_add(1, Ordering::AcqRel);
}
pub fn dismiss_pending() {
    if let Ok(mut pending) = DECISION.lock() {
        if let Some((_, decision)) = pending.as_mut() {
            if decision.is_none() {
                *decision = Some(false);
            }
        }
    }
}
fn install() -> Result<PathBuf, String> {
    crate::resolve_install_directory(&crate::app_data_directory())
}
fn bridge_path() -> Result<PathBuf, String> {
    Ok(install()?.join("blob/anyblob.json"))
}
fn state() -> Result<Value, String> {
    let path = bridge_path()?;
    if !path.exists() {
        return Ok(json!({"enabled":false}));
    }
    crate::reject_reparse_component(&path)?;
    crate::read_json_file(&path)
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn authenticate(state: &Value, token: &str) -> Result<(), String> {
    if state["enabled"] != true {
        return Err("Anyblob is disabled. Enable it in BlobUI Integrations > Anyblob.".into());
    }
    let expected = state["verifier"].as_str().unwrap_or("");
    let actual = digest(token);
    // Fixed-size digest comparison without an early mismatch exit.
    let mismatch = expected
        .bytes()
        .zip(actual.bytes())
        .fold(0u8, |a, (b, c)| a | (b ^ c));
    if token.len() != 64 || expected.len() != 64 || mismatch != 0 {
        return Err("Anyblob credential rejected. Disable and enable Anyblob in BlobUI.".into());
    }
    Ok(())
}
// The save guard serializes migration, toggles and permission saves.
fn configure_at(
    path: &std::path::Path,
    credential: &std::path::Path,
    enabled: bool,
) -> Result<(), String> {
    use crate::storage as store;
    blobui_mcp::credential::reject_links(credential)?;
    crate::reject_reparse_component(path)?;
    let before = store::snapshot(path, false)?;
    let secret_before = store::snapshot(credential, true)?;
    let mut bytes = [0u8; 32];
    if enabled {
        getrandom::fill(&mut bytes)
            .map_err(|_| "Windows could not generate an Anyblob credential.")?;
    }
    let token = bytes.iter().map(|v| format!("{v:02x}")).collect::<String>();
    bytes.fill(0);
    let encrypted = if enabled {
        Some(blobui_mcp::credential::encrypt(&token)?)
    } else {
        None
    };
    let state = serde_json::to_vec(
        &json!({"enabled":enabled,"verifier":if enabled {digest(&token)}else{String::new()}}),
    )
    .unwrap();
    let state_temp = store::prepare_temp(path, &state, None, false)?;
    let secret_temp = match encrypted
        .as_ref()
        .map(|data| store::prepare_temp(credential, data, None, true))
        .transpose()
    {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_file(&state_temp);
            return Err(e);
        }
    };
    let result = (|| {
        if let Some(temp) = &secret_temp {
            store::replace_credential(temp, credential)
                .map_err(|_| "Could not save the Anyblob credential.")?;
        } else if credential.exists() {
            std::fs::remove_file(credential)
                .map_err(|_| "Could not remove the Anyblob credential.")?;
        }
        store::replace_credential(&state_temp, path)
            .map_err(|e| format!("Could not save Anyblob state: {e}"))?;
        Ok::<_, String>(())
    })();
    let _ = std::fs::remove_file(state_temp);
    if let Some(p) = secret_temp {
        let _ = std::fs::remove_file(p);
    }
    if let Err(error) = result {
        let rollback = [store::restore(&before), store::restore(&secret_before)];
        invalidate();
        if rollback.iter().any(Result::is_err) {
            return Err(format!(
                "{error} Could not fully restore the previous Anyblob configuration."
            ));
        }
        return Err(error);
    }
    invalidate();
    Ok(())
}
fn migrate() -> Result<bool, String> {
    let _guard = crate::acquire_save_guard("Wait for the current save before changing Anyblob.")?;
    migrate_at(&bridge_path()?, &blobui_mcp::credential::path()?)
}
fn migrate_at(path: &std::path::Path, credential: &std::path::Path) -> Result<bool, String> {
    let value = if path.exists() {
        crate::read_json_file(path)?
    } else {
        json!({"enabled":false})
    };
    let enabled = value["enabled"] == true;
    if enabled
        && blobui_mcp::credential::read_at(credential)
            .ok()
            .is_none_or(|token| authenticate(&value, &token).is_err())
    {
        configure_at(path, credential, true)?;
    } else if !enabled && credential.exists() {
        configure_at(path, credential, false)?;
    }
    Ok(enabled)
}
#[tauri::command]
pub fn anyblob_status() -> Result<Value, String> {
    Ok(json!({"enabled":migrate()?}))
}
#[tauri::command]
pub fn configure_anyblob(enabled: bool) -> Result<Value, String> {
    let _guard = crate::acquire_save_guard("Wait for the current save before changing Anyblob.")?;
    configure_at(&bridge_path()?, &blobui_mcp::credential::path()?, enabled)?;
    Ok(json!({"enabled":enabled}))
}
#[tauri::command]
pub fn approve_anyblob(id: String, allow: bool) -> Result<(), String> {
    let mut pending = DECISION.lock().map_err(|_| "Approval unavailable.")?;
    match pending.as_mut() {
        Some((active, decision)) if *active == id && decision.is_none() => {
            *decision = Some(allow);
            Ok(())
        }
        _ => Err("This request is no longer awaiting approval.".into()),
    }
}
fn saved() -> Result<crate::codex::Configuration, String> {
    let data = crate::read_json_file(&install()?.join("blob/data.json"))?;
    Ok(crate::settings_from_data(&data).codex_integration)
}
pub(crate) fn readable(config: &crate::codex::Configuration, space: &str) -> bool {
    config
        .spaces
        .get(space)
        .is_some_and(|p| matches!(p.access.as_str(), "read" | "readWrite"))
}
fn allowed(config: &crate::codex::Configuration, op: &Operation) -> Result<(), String> {
    // No supported host-owned chat identity reaches this bridge yet. Do not
    // mistake installation credentials or model arguments for project identity.
    if config.projects.values().any(|project| !project.sources.is_empty()) {
        return Err("Anyblob reads are paused because project sources are saved, but Codex has not supplied verified chat identity. Remove every project source selection to use global permissions.".into());
    }
    if op.space().is_some_and(|space| !readable(config, space)) {
        Err("Access denied by saved BlobUI space permissions. Save Read Only or Read-Write access first.".into())
    } else {
        Ok(())
    }
}
fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}
fn identity(row: &Value) -> Result<Value, String> {
    let id = row["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Anytype returned an object without an ID.")?;
    let name = row["name"]
        .as_str()
        .or_else(|| row.pointer("/properties/name").and_then(Value::as_str))
        .ok_or("Anytype returned an invalid object name.")?;
    Ok(json!({"id":id,"name":name,"type":row["type"]}))
}
// This function alone constructs API operations. It never accepts a URL, path or body.
fn execute(
    client: &crate::kanban_api::Client,
    config: &crate::codex::Configuration,
    op: &Operation,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    allowed(config, op)?;
    op.validate()?;
    check()?;
    let unavailable = |_: String| {
        "Anytype API v2 could not complete this read. Check the saved API connection, object ID and Anytype availability.".to_string()
    };
    match op {
        Operation::ListSpaces {} => {
            let rows = client
                .pages("GET", "/v2/spaces?ids=full", None)
                .map_err(unavailable)?;
            let rows=rows.iter().filter(|v|v["id"].as_str().is_some_and(|id|readable(config,id))).map(|v|{
                Ok(json!({"id":v["id"].as_str().ok_or("Invalid space ID.")?,"name":v["name"].as_str().ok_or("Invalid space name.")?}))
            }).collect::<Result<Vec<Value>,String>>()?;
            Ok(json!({"spaces":rows}))
        }
        Operation::SearchObjects {
            space_id,
            query,
            limit,
            offset,
        } => {
            let response = client
                .request(
                    "POST",
                    &format!(
                        "/v2/spaces/{}/search?ids=full&limit={limit}&offset={offset}",
                        enc(space_id)
                    ),
                    Some(&json!({"query":query})),
                )
                .map_err(unavailable)?;
            let rows = response["data"]
                .as_array()
                .ok_or("Anytype returned invalid search results.")?;
            if rows.len() > *limit as usize {
                return Err("Anytype returned too many search results.".into());
            }
            let more = response["has_more"]
                .as_bool()
                .ok_or("Anytype omitted search pagination.")?;
            let total = response["total"]
                .as_u64()
                .ok_or("Anytype omitted search total.")?;
            Ok(
                json!({"space_id":space_id,"objects":rows.iter().map(identity).collect::<Result<Vec<_>,_>>()?,"limit":limit,"offset":offset,"has_more":more,"total":total}),
            )
        }
        Operation::ReadObject {
            space_id,
            object_id,
        } => {
            let path = format!(
                "/v2/spaces/{}/objects/{}?ids=full",
                enc(space_id),
                enc(object_id)
            );
            let document = client.request("GET", &path, None).map_err(unavailable)?;
            let mut result = identity(&document)?;
            if document["id"] != *object_id || !document["properties"].is_object() {
                return Err("Anytype returned an invalid object document.".into());
            }
            check()?;
            let markdown = client
                .request("GET", &format!("{path}&format=md"), None)
                .map_err(unavailable)?;
            let content = markdown["markdown"]
                .as_str()
                .ok_or("Anytype did not return Markdown content.")?;
            if markdown["id"] != *object_id {
                return Err("Anytype returned Markdown for a different object.".into());
            }
            result["space_id"] = json!(space_id);
            result["properties"] = document["properties"].clone();
            result["markdown"] = json!(content);
            // Keep file blocks as references. No content endpoint is ever requested.
            result["attachments"] = json!(document["blocks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|b| ["file", "image", "video", "audio", "pdf"]
                    .contains(&b["type"].as_str().unwrap_or("")))
                .cloned()
                .collect::<Vec<_>>());
            Ok(result)
        }
    }
}
fn approve_with(
    check: impl Fn() -> Result<(), String>,
    timeout: Duration,
    mut decision: impl FnMut() -> Option<bool>,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        check()?;
        match decision() {
            Some(true) => return Ok(()),
            Some(false) => return Err("Read denied in BlobUI.".into()),
            None => {}
        };
        if Instant::now() >= deadline {
            return Err("BlobUI approval timed out. Retry and approve within two minutes.".into());
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}
fn guarded_read(
    client: &crate::kanban_api::Client,
    config: &crate::codex::Configuration,
    op: &Operation,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    let result = execute(client, config, op, check)?;
    check()?;
    Ok(result)
}
fn show_prompt(
    app: &tauri::AppHandle,
    op: &Operation,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let _prompt = acquire_prompt(check, deadline)?;
    check()?;
    let id = format!("anyblob-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    *DECISION.lock().map_err(|_| "Approval unavailable.")? = Some((id.clone(), None));
    crate::show_settings_window(app);
    let emitted = app.emit("anyblob-approval", json!({"id":id,"request":op}));
    let result = if emitted.is_err() {
        Err("Could not show BlobUI approval. Retry the read.".into())
    } else {
        approve_with(
            &check,
            deadline.saturating_duration_since(Instant::now()),
            || {
                DECISION
                    .lock()
                    .ok()
                    .and_then(|v| v.as_ref().and_then(|(_, d)| *d))
            },
        )
    };
    *DECISION.lock().map_err(|_| "Approval unavailable.")? = None;
    let _ = app.emit("anyblob-approval-closed", json!({"id":id}));
    result?;
    check()?;
    Ok(())
}
fn acquire_prompt(
    check: &dyn Fn() -> Result<(), String>,
    deadline: Instant,
) -> Result<std::sync::MutexGuard<'static, ()>, String> {
    loop {
        check()?;
        if Instant::now() >= deadline {
            return Err("Approval queue timed out. Retry the read.".into());
        }
        if let Ok(lock) = PROMPT.try_lock() {
            return Ok(lock);
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}
fn dispatch_with(
    request: Request,
    connected: impl Fn() -> bool,
    load_state: impl Fn() -> Result<Value, String>,
    load_config: impl Fn() -> Result<crate::codex::Configuration, String>,
    connect: impl Fn() -> Result<crate::kanban_api::Client, String>,
    approve: impl Fn(&Operation, &dyn Fn() -> Result<(), String>) -> Result<(), String>,
) -> Result<Value, String> {
    request.request.validate()?;
    let initial = load_state()?;
    authenticate(&initial, &request.token)?;
    let config = load_config()?;
    allowed(&config, &request.request)?;
    let revision = REVISION.load(Ordering::Acquire);
    let check = || {
        if !connected() {
            return Err("MCP request cancelled or disconnected.".into());
        }
        let current = load_state()?;
        authenticate(&current, &request.token)?;
        let current_config = load_config()?;
        if revision != REVISION.load(Ordering::Acquire)
            || current != initial
            || current_config != config
        {
            return Err("BlobUI permissions or integration changed. Retry the request.".into());
        }
        allowed(&current_config, &request.request)
    };
    check()?;
    if request
        .request
        .space()
        .and_then(|s| config.spaces.get(s))
        .is_some_and(|p| p.approval == "alwaysAsk")
    {
        approve(&request.request, &check)?;
    }
    check()?;
    guarded_read(&connect()?, &config, &request.request, &check)
}
fn dispatch(
    app: &tauri::AppHandle,
    request: Request,
    connected: impl Fn() -> bool,
) -> Result<Value, String> {
    dispatch_with(
        request,
        connected,
        state,
        saved,
        || {
            crate::kanban_api::Client::connect().map_err(|_| {
                "Configure the saved Anytype API v2 connection in BlobUI Advanced Settings.".into()
            })
        },
        |op, check| show_prompt(app, op, check),
    )
}
pub fn start(app: tauri::AppHandle) {
    if migrate().is_err() {
        eprintln!("Anyblob credential setup needs attention in Settings.");
    }
    #[cfg(windows)]
    std::thread::spawn(move || {
        use blobui_mcp::transport::{Pipe, CALL_TIMEOUT};
        let mut listener = match Pipe::listen(true) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("Anyblob bridge could not start.");
                return;
            }
        };
        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        loop {
            match listener.accept() {
                Ok(false) => std::thread::sleep(Duration::from_millis(30)),
                Err(_) => break,
                Ok(true) => {
                    let next = match Pipe::listen(false) {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    let pipe = std::mem::replace(&mut listener, next);
                    if active.load(Ordering::Acquire) >= 15 {
                        drop(pipe);
                        continue;
                    }
                    active.fetch_add(1, Ordering::AcqRel);
                    let active = active.clone();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        let cancel = AtomicBool::new(false);
                        let deadline = Instant::now() + CALL_TIMEOUT;
                        let start = Instant::now();
                        if let Ok(bytes) =
                            pipe.receive(&cancel, Instant::now() + Duration::from_secs(5))
                        {
                            if let Ok(request) = serde_json::from_slice::<Request>(&bytes) {
                                let operation = request.request.name();
                                let id = NEXT.fetch_add(1, Ordering::Relaxed);
                                let result = dispatch(&app, request, || {
                                    Instant::now() < deadline && pipe.connected()
                                });
                                eprintln!("anyblob request={id} operation={operation} elapsed_ms={} outcome={}",start.elapsed().as_millis(),if result.is_ok(){"allowed"}else{"denied"});
                                let response = match result {
                                    Ok(value) => Response {
                                        result: Some(value),
                                        error: None,
                                    },
                                    Err(error) => Response {
                                        result: None,
                                        error: Some(error),
                                    },
                                };
                                if let Ok(bytes) = serde_json::to_vec(&response) {
                                    let _ = pipe.send(&bytes, &cancel, deadline);
                                }
                            }
                        }
                        active.fetch_sub(1, Ordering::AcqRel);
                    });
                }
            }
        }
    });
    #[cfg(not(windows))]
    let _ = app;
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
    };
    #[test]
    fn approval_queue_serializes_and_expires() {
        let first = acquire_prompt(&|| Ok(()), Instant::now() + Duration::from_secs(1)).unwrap();
        assert!(acquire_prompt(&|| Ok(()), Instant::now()).is_err());
        assert!(acquire_prompt(
            &|| Err("cancelled".into()),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        let entered = Arc::new(AtomicBool::new(false));
        let signal = entered.clone();
        let next = std::thread::spawn(move || {
            let _second =
                acquire_prompt(&|| Ok(()), Instant::now() + Duration::from_secs(2)).unwrap();
            signal.store(true, Ordering::Release);
        });
        std::thread::sleep(Duration::from_millis(60));
        assert!(!entered.load(Ordering::Acquire));
        drop(first);
        next.join().unwrap();
        assert!(entered.load(Ordering::Acquire));
    }
    fn config(access: &str, approval: &str) -> crate::codex::Configuration {
        crate::codex::normalize(&json!({"spaces":{"space":{"access":access,"approval":approval}}}))
    }
    #[test]
    fn unresolved_chat_identity_denies_every_tool_with_any_saved_selection() {
        let selected=crate::codex::normalize(&json!({"spaces":{"space":{"access":"read"}},"projects":{"missing-project":{"sources":[{"spaceId":"missing-space","kind":"query","id":"missing-source","permission":{"access":"read"}}]}}}));
        for op in [Operation::ListSpaces{},search(),Operation::ReadObject{space_id:"space".into(),object_id:"direct-id".into()}] {
            assert!(allowed(&selected,&op).unwrap_err().contains("verified chat identity"));
            assert!(dispatch_with(request(op),||true,||Ok(enabled()),||Ok(selected.clone()),||panic!("must deny before connecting"),|_,_|panic!("must deny before approval")).is_err());
        }
        let empty=crate::codex::normalize(&json!({"spaces":{"space":{"access":"read"}},"projects":{"saved":{"sources":[]}}}));
        assert!(allowed(&empty,&search()).is_ok());
        assert!(allowed(&config("read","fullAccess"),&Operation::ListSpaces{}).is_ok());
    }
    fn request(op: Operation) -> Request {
        Request {
            id: "fixture".into(),
            token: "a".repeat(64),
            request: op,
        }
    }
    fn enabled() -> Value {
        json!({"enabled":true,"verifier":digest(&"a".repeat(64))})
    }
    fn search() -> Operation {
        Operation::SearchObjects {
            space_id: "space".into(),
            query: "private fixture query".into(),
            limit: 20,
            offset: 7,
        }
    }
    fn mock(
        responses: Vec<(String, Value)>,
        revoke: Option<Arc<AtomicBool>>,
    ) -> (crate::kanban_api::Client, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for (expected, body) in responses {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(_) if Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(e) => panic!("missing fixture request {expected}: {e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buffer = [0; 8192];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buffer[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&data[..end]);
                        let length = header
                            .lines()
                            .find_map(|l| {
                                l.strip_prefix("Content-Length: ")
                                    .and_then(|l| l.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if data.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let text = String::from_utf8(data).unwrap();
                assert!(text.starts_with(&expected), "{text}");
                assert!(expected.contains("/v2/"));
                if expected.starts_with("POST") {
                    assert!(text.ends_with("{\"query\":\"private fixture query\"}"));
                }
                if let Some(flag) = &revoke {
                    flag.store(true, Ordering::Release);
                }
                let status = body["_fixture_status"].as_u64().unwrap_or(200);
                let body = body.to_string();
                write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        (
            crate::kanban_api::Client::with_connection("fixture-api-key", "127.0.0.1", port),
            server,
        )
    }
    #[test]
    fn allowed_v2_reads_and_attachment_references() {
        let (client, server) = mock(
            vec![
                (
                    "GET /v2/spaces?ids=full&limit=100&offset=0 ".into(),
                    json!({"data":[{"id":"space","name":"Work"}],"has_more":true}),
                ),
                (
                    "GET /v2/spaces?ids=full&limit=100&offset=1 ".into(),
                    json!({"data":[{"id":"denied","name":"Private"}],"has_more":false}),
                ),
                (
                    "POST /v2/spaces/space/search?ids=full&limit=20&offset=7 ".into(),
                    json!({"data":[{"id":"object","name":"Note","type":"page"}],"has_more":true,"total":50}),
                ),
                (
                    "GET /v2/spaces/space/objects/object?ids=full ".into(),
                    json!({"id":"object","type":"page","properties":{"name":"Note","link":["other-object"]},"blocks":[{"type":"file","id":"file-ref","target":"attachment"}]}),
                ),
                (
                    "GET /v2/spaces/space/objects/object?ids=full&format=md ".into(),
                    json!({"id":"object","markdown":"# Note\n[file](attachment)"}),
                ),
            ],
            None,
        );
        let config = config("read", "fullAccess");
        assert_eq!(
            guarded_read(&client, &config, &Operation::ListSpaces {}, &|| Ok(())).unwrap()
                ["spaces"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            guarded_read(&client, &config, &search(), &|| Ok(())).unwrap()["has_more"],
            true
        );
        let read = guarded_read(
            &client,
            &config,
            &Operation::ReadObject {
                space_id: "space".into(),
                object_id: "object".into(),
            },
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(read["name"], "Note");
        assert_eq!(read["attachments"][0]["target"], "attachment");
        server.join().unwrap();
    }
    #[test]
    fn revocation_during_api_discards_data_and_malformed_results_fail() {
        let revoked = Arc::new(AtomicBool::new(false));
        let check = || {
            if revoked.load(Ordering::Acquire) {
                Err("revoked".into())
            } else {
                Ok(())
            }
        };
        let (client, server) = mock(
            vec![(
                "POST /v2/spaces/space/search?ids=full&limit=20&offset=7 ".into(),
                json!({"data":[{"id":"secret","name":"Never returned"}],"total":1,"has_more":false}),
            )],
            Some(revoked.clone()),
        );
        assert!(guarded_read(&client, &config("read", "fullAccess"), &search(), &check).is_err());
        server.join().unwrap();
        let (client, server) = mock(
            vec![(
                "POST /v2/spaces/space/search?ids=full&limit=20&offset=7 ".into(),
                json!({"data":[{"id":"secret"}]}),
            )],
            None,
        );
        assert!(guarded_read(
            &client,
            &config("read", "fullAccess"),
            &search(),
            &|| Ok(())
        )
        .is_err());
        server.join().unwrap();
    }
    #[test]
    fn missing_objects_and_unavailable_anytype_return_no_data() {
        let (client, server) = mock(
            vec![(
                "GET /v2/spaces/space/objects/missing?ids=full ".into(),
                json!({"_fixture_status":404,"message":"sensitive raw failure"}),
            )],
            None,
        );
        let error = guarded_read(
            &client,
            &config("read", "fullAccess"),
            &Operation::ReadObject {
                space_id: "space".into(),
                object_id: "missing".into(),
            },
            &|| Ok(()),
        )
        .unwrap_err();
        assert!(!error.contains("sensitive raw failure"));
        server.join().unwrap();
        let client = crate::kanban_api::Client::with_connection("fixture-key", "127.0.0.1", 0);
        assert!(guarded_read(
            &client,
            &config("read", "fullAccess"),
            &search(),
            &|| Ok(())
        )
        .is_err());
    }
    #[test]
    fn saved_permissions_and_approval_rechecks() {
        let saved = Mutex::new(config("none", "alwaysAsk"));
        let _unsaved = config("read", "fullAccess");
        assert!(dispatch_with(
            request(search()),
            || true,
            || Ok(enabled()),
            || Ok(saved.lock().unwrap().clone()),
            || panic!("must not connect"),
            |_, _| panic!("must not prompt")
        )
        .is_err());
        for access in ["read", "readWrite"] {
            for approval in ["alwaysAsk", "approveForMe", "fullAccess"] {
                *saved.lock().unwrap() = config(access, approval);
                let asked = AtomicBool::new(false);
                let result = dispatch_with(
                    request(search()),
                    || true,
                    || Ok(enabled()),
                    || Ok(saved.lock().unwrap().clone()),
                    || Err("fixture unavailable".into()),
                    |_, check| {
                        asked.store(true, Ordering::Release);
                        check()
                    },
                );
                assert!(result.is_err());
                assert_eq!(asked.load(Ordering::Acquire), approval == "alwaysAsk");
            }
        }
        *saved.lock().unwrap() = config("read", "alwaysAsk");
        assert!(dispatch_with(
            request(search()),
            || true,
            || Ok(enabled()),
            || Ok(saved.lock().unwrap().clone()),
            || panic!("revoked read must not connect"),
            |_, check| {
                *saved.lock().unwrap() = config("none", "alwaysAsk");
                check()
            }
        )
        .is_err());
        assert!(dispatch_with(
            request(search()),
            || true,
            || Ok(json!({"enabled":false})),
            || panic!(),
            || panic!(),
            |_, _| panic!()
        )
        .is_err());
        assert!(dispatch_with(
            request(search()),
            || true,
            || Ok(json!({"enabled":true,"verifier":digest(&"b".repeat(64))})),
            || panic!(),
            || panic!(),
            |_, _| panic!()
        )
        .is_err());
    }
    #[test]
    fn token_rotation_and_disconnect_during_approval() {
        let state = Mutex::new(enabled());
        let connected = AtomicBool::new(true);
        for rotate in [true, false] {
            *state.lock().unwrap() = enabled();
            connected.store(true, Ordering::Release);
            assert!(dispatch_with(
                request(search()),
                || connected.load(Ordering::Acquire),
                || Ok(state.lock().unwrap().clone()),
                || Ok(config("read", "alwaysAsk")),
                || panic!("must not connect"),
                |_, check| {
                    if rotate {
                        *state.lock().unwrap() =
                            json!({"enabled":true,"verifier":digest(&"b".repeat(64))});
                    } else {
                        connected.store(false, Ordering::Release);
                    }
                    check()
                }
            )
            .is_err());
        }
    }
    #[cfg(windows)]
    #[test]
    fn executable_to_pipe_to_blobui_to_mock_anytype() {
        use blobui_mcp::transport::{Pipe, CALL_TIMEOUT};
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
        };
        // Test-Anyblob supplies a unique instance. Never bind the real app's pipe.
        assert!(std::env::var("ANYBLOB_BRIDGE_INSTANCE")
            .unwrap_or_default()
            .starts_with("fixture-"));
        let package =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../codex-plugin/dist/anyblob");
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(package.join(".mcp.json")).expect("Build the plugin before this test."),
        )
        .unwrap();
        let launch = &manifest["mcpServers"]["anyblob"];
        let (client, api) = mock(
            vec![
                (
                    "GET /v2/spaces?ids=full&limit=100&offset=0 ".into(),
                    json!({"data":[{"id":"space","name":"Work"}],"has_more":false}),
                ),
                (
                    "POST /v2/spaces/space/search?ids=full&limit=20&offset=7 ".into(),
                    json!({"data":[{"id":"object","name":"Note","type":"page"}],"total":1,"has_more":false}),
                ),
                (
                    "GET /v2/spaces/space/objects/object?ids=full ".into(),
                    json!({"id":"object","type":"page","properties":{"name":"Note"},"blocks":[]}),
                ),
                (
                    "GET /v2/spaces/space/objects/object?ids=full&format=md ".into(),
                    json!({"id":"object","markdown":"fixture content"}),
                ),
            ],
            None,
        );
        let saved_config = Arc::new(Mutex::new(config("read", "alwaysAsk")));
        let saved = saved_config.clone();
        let mut listener = Pipe::listen(true).unwrap();
        let bridge = std::thread::spawn(move || {
            let mut outcomes = Vec::new();
            for index in 0..5 {
                let deadline = Instant::now() + Duration::from_secs(15);
                while !listener.accept().unwrap() {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(5));
                }
                let pipe = std::mem::replace(&mut listener, Pipe::listen(false).unwrap());
                let cancel = AtomicBool::new(false);
                let request: Request =
                    serde_json::from_slice(&pipe.receive(&cancel, deadline).unwrap()).unwrap();
                let result = dispatch_with(
                    request,
                    || pipe.connected(),
                    || Ok(enabled()),
                    || Ok(saved.lock().unwrap().clone()),
                    || Ok(client.clone_for_fixture()),
                    |_, check| {
                        if index == 4 {
                            approve_with(check, Duration::from_secs(5), || None)
                        } else {
                            check()
                        }
                    },
                );
                outcomes.push(result.is_ok());
                let response = match result {
                    Ok(value) => Response {
                        result: Some(value),
                        error: None,
                    },
                    Err(error) => Response {
                        result: None,
                        error: Some(error),
                    },
                };
                let _ = pipe.send(
                    &serde_json::to_vec(&response).unwrap(),
                    &cancel,
                    Instant::now() + CALL_TIMEOUT,
                );
            }
            outcomes
        });
        let mut child = Command::new(launch["command"].as_str().unwrap())
            .args(
                launch["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a.as_str().unwrap()),
            )
            .current_dir(package.join(launch["cwd"].as_str().unwrap()))
            .env("ANYBLOB_CREDENTIAL_FILE", {
                let path = package.join("../../../.test-run/anyblob-handoff/fixture.dpapi");
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(
                    &path,
                    blobui_mcp::credential::encrypt(&"a".repeat(64)).unwrap(),
                )
                .unwrap();
                path.canonicalize().unwrap()
            })
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        fn send(input: &mut impl Write, id: u32, method: &str, params: Value) {
            writeln!(
                input,
                "{}",
                json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
            )
            .unwrap();
            input.flush().unwrap();
        }
        fn receive(output: &mut impl BufRead) -> Value {
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            serde_json::from_str(&line).expect("protocol-only stdout")
        }
        send(
            &mut input,
            1,
            "initialize",
            json!({"protocolVersion":"2025-03-26"}),
        );
        assert_eq!(
            receive(&mut output)["result"]["serverInfo"]["name"],
            "anyblob"
        );
        for (id, name, args) in [
            (2, "list_spaces", json!({})),
            (
                3,
                "search_objects",
                json!({"space_id":"space","query":"private fixture query","offset":7}),
            ),
            (
                4,
                "read_object",
                json!({"space_id":"space","object_id":"object"}),
            ),
        ] {
            send(
                &mut input,
                id,
                "tools/call",
                json!({"name":name,"arguments":args}),
            );
            let response = receive(&mut output);
            assert_eq!(response["result"]["isError"], false, "{response}");
            assert!(response["result"]["structuredContent"].is_object());
        }
        *saved_config.lock().unwrap() = config("none", "alwaysAsk");
        send(
            &mut input,
            5,
            "tools/call",
            json!({"name":"read_object","arguments":{"space_id":"space","object_id":"object"}}),
        );
        let denied = receive(&mut output);
        assert_eq!(denied["result"]["isError"], true);
        assert!(denied["result"].get("structuredContent").is_none());
        *saved_config.lock().unwrap() = config("read", "alwaysAsk");
        send(
            &mut input,
            6,
            "tools/call",
            json!({"name":"search_objects","arguments":{"space_id":"space","query":"cancel me"}}),
        );
        std::thread::sleep(Duration::from_millis(100));
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":6}})
        )
        .unwrap();
        input.flush().unwrap();
        assert_eq!(receive(&mut output)["error"]["code"], -32800);
        drop(input);
        assert!(child.wait().unwrap().success());
        assert_eq!(bridge.join().unwrap(), vec![true, true, true, false, false]);
        api.join().unwrap();
    }
    #[test]
    fn permissions_and_tokens_fail_closed() {
        for access in ["none", "read", "readWrite", "createDelete"] {
            for approval in ["alwaysAsk", "approveForMe", "fullAccess"] {
                let config = crate::codex::normalize(
                    &json!({"spaces":{"space":{"access":access,"approval":approval,"filesEnabled":true}}}),
                );
                assert_eq!(
                    readable(&config, "space"),
                    ["read", "readWrite"].contains(&access)
                );
                assert!(allowed(
                    &config,
                    &Operation::ReadObject {
                        space_id: "other".into(),
                        object_id: "known-id".into()
                    }
                )
                .is_err());
            }
        }
        let token = "a".repeat(64);
        let state = json!({"enabled":true,"verifier":digest(&token)});
        assert!(authenticate(&state, &token).is_ok());
        assert!(authenticate(&state, &"b".repeat(64)).is_err());
        assert!(authenticate(&json!({"enabled":false}), &token).is_err());
    }
    #[test]
    fn approval_is_bound_and_cancellable() {
        assert!(approve_with(|| Ok(()), Duration::ZERO, || Some(true)).is_ok());
        assert!(approve_with(|| Ok(()), Duration::ZERO, || Some(false)).is_err());
        assert!(approve_with(|| Ok(()), Duration::ZERO, || None).is_err());
        for reason in ["cancelled", "revoked", "disconnected", "dismissed"] {
            assert!(
                approve_with(|| Err(reason.into()), Duration::from_secs(1), || Some(true)).is_err()
            );
        }
        assert!(approve_anyblob("stale".into(), true).is_err());
    }
}

#[cfg(all(test, windows))]
mod credential_tests {
    use super::*;
    #[test]
    fn lifecycle_migration_encryption_and_rollback() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../.test-run")
            .join(format!(
                "credential-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let state_path = root.join("state.json");
        let secret = root.join("bridge.dpapi");
        // Existing enabled installs cannot recover the old token from its verifier.
        let old = "a".repeat(64);
        std::fs::write(
            &state_path,
            json!({"enabled":true,"verifier":digest(&old)}).to_string(),
        )
        .unwrap();
        assert!(migrate_at(&state_path, &secret).unwrap());
        let token = blobui_mcp::credential::read_at(&secret).unwrap();
        assert_ne!(token, old);
        let value = crate::read_json_file(&state_path).unwrap();
        authenticate(&value, &token).unwrap();
        assert!(authenticate(&value, &old).is_err());
        assert!(!std::fs::read(&secret)
            .unwrap()
            .windows(64)
            .any(|v| v == token.as_bytes()));
        // The credential must have one protected owner-only access rule.
        unsafe {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::{
                Foundation::LocalFree,
                Security::{
                    Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW,
                    GetFileSecurityW, DACL_SECURITY_INFORMATION,
                },
            };
            let name = secret
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let mut size = 0;
            GetFileSecurityW(
                name.as_ptr(),
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                0,
                &mut size,
            );
            let mut descriptor = vec![0u8; size as usize];
            assert_ne!(
                GetFileSecurityW(
                    name.as_ptr(),
                    DACL_SECURITY_INFORMATION,
                    descriptor.as_mut_ptr().cast(),
                    size,
                    &mut size
                ),
                0
            );
            let mut text = std::ptr::null_mut();
            assert_ne!(
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor.as_mut_ptr().cast(),
                    1,
                    DACL_SECURITY_INFORMATION,
                    &mut text,
                    std::ptr::null_mut()
                ),
                0
            );
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let acl = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            assert!(acl.starts_with("D:P"));
            assert!(acl.contains("(A;;FA;;;OW)"));
            assert_eq!(acl.matches('(').count(), 1);
        }
        let saved_secret = std::fs::read(&secret).unwrap();
        let saved_state = std::fs::read(&state_path).unwrap();
        assert!(migrate_at(&state_path, &secret).unwrap());
        assert_eq!(blobui_mcp::credential::read_at(&secret).unwrap(), token);
        for failed in [&secret, &state_path] {
            crate::storage::FAIL_AFTER_REPLACE.with(|v| *v.borrow_mut() = Some(failed.clone()));
            assert!(configure_at(&state_path, &secret, true).is_err());
            assert_eq!(std::fs::read(&secret).unwrap(), saved_secret);
            assert_eq!(std::fs::read(&state_path).unwrap(), saved_state);
        }
        configure_at(&state_path, &secret, false).unwrap();
        assert!(!secret.exists());
        assert!(authenticate(&crate::read_json_file(&state_path).unwrap(), &token).is_err());
        configure_at(&state_path, &secret, true).unwrap();
        assert_ne!(blobui_mcp::credential::read_at(&secret).unwrap(), token);
        std::fs::write(&secret, b"corrupted credential").unwrap();
        assert!(migrate_at(&state_path, &secret).unwrap());
        authenticate(
            &crate::read_json_file(&state_path).unwrap(),
            &blobui_mcp::credential::read_at(&secret).unwrap(),
        )
        .unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
