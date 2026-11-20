//! Anyblob's trust boundary. Only saved settings authorize reads.
use blobui_mcp::{Operation, Request, Response};
#[cfg(windows)]
use blobui_mcp::transport::Bridge;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
#[cfg(any(windows, test))]
use std::sync::atomic::AtomicBool;

static PROMPT: Mutex<()> = Mutex::new(());
#[cfg(windows)]
static NEXT: AtomicU64 = AtomicU64::new(1);
static REVISION: AtomicU64 = AtomicU64::new(0);
pub fn invalidate() {
    REVISION.fetch_add(1, Ordering::AcqRel);
}
fn install() -> Result<PathBuf, String> {
    crate::resolve_install_directory(&crate::app_data_directory())
}
pub(crate) fn bridge_path() -> Result<PathBuf, String> {
    let install = install()?;
    #[cfg(test)]
    {
        return Ok(install.join("blob/anyblob.json"));
    }
    #[cfg(not(test))]
    {
        let _ = crate::settings_data_path(&install)?;
        Ok(crate::settings_root_for_install(&install)?.join("Settings/anyblob/settings.json"))
    }
}
pub(crate) fn state() -> Result<Value, String> {
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
    if crate::HEADLESS_MODE.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(());
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
    let mut state = if path.is_file() {
        crate::read_json_file(path)?
    } else {
        json!({})
    };
    let fields = state
        .as_object_mut()
        .ok_or("Anyblob settings must contain an object.")?;
    fields.insert("enabled".into(), json!(enabled));
    fields.remove("remoteAccess");
    fields.insert(
        "verifier".into(),
        json!(if enabled {
            digest(&token)
        } else {
            String::new()
        }),
    );
    let state = serde_json::to_vec(&state).map_err(|_| "Could not encode Anyblob settings.")?;
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
    let path = bridge_path()?;
    let enabled = migrate_at(&path, &blobui_mcp::credential::path()?)?;
    remove_legacy_tunnel_secret(&path)?;
    Ok(enabled)
}

fn remove_legacy_tunnel_secret(settings_path: &std::path::Path) -> Result<(), String> {
    let settings = settings_path.parent().and_then(std::path::Path::parent)
        .ok_or("The Anyblob Settings folder is unavailable.")?;
    let secret = settings.join("DO-NOT-SHARE/secure-mcp-tunnel.dpapi");
    match std::fs::symlink_metadata(&secret) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() && !crate::is_windows_reparse_point(&metadata) => {
            std::fs::remove_file(&secret).map_err(|_| "Could not remove the saved tunnel credential.".to_string())
        }
        Ok(_) => Err("The saved tunnel credential is not a regular file.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Could not inspect the saved tunnel credential.".into()),
    }
}
fn migrate_at(path: &std::path::Path, credential: &std::path::Path) -> Result<bool, String> {
    let mut value = if path.exists() {
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
        value = crate::read_json_file(path)?;
    } else if !enabled && credential.exists() {
        configure_at(path, credential, false)?;
        value = crate::read_json_file(path)?;
    }
    if let Some(fields) = value.as_object_mut() {
        let mut changed = fields.remove("remoteAccess").is_some();
        for name in ["secureMcpTunnelId", "secureMcpTunnelEnabled"] {
            changed |= fields.remove(name).is_some();
        }
        if changed {
            let bytes = serde_json::to_vec(&value)
                .map_err(|_| "Could not encode Anyblob settings during migration.")?;
            crate::storage::write_atomic(path, &bytes)?;
            invalidate();
        }
    }
    Ok(enabled)
}
#[tauri::command]
pub fn anyblob_status() -> Result<Value, String> {
    let current = match crate::acquire_save_guard(
        "Wait for the current save before changing Anyblob.",
    ) {
        Ok(_guard) => {
            let enabled = migrate_at(&bridge_path()?, &blobui_mcp::credential::path()?)?;
            let mut current = state()?;
            if let Some(fields) = current.as_object_mut() {
                fields.insert("enabled".into(), json!(enabled));
            }
            current
        }
        Err(error) if error == "Wait for the current save before changing Anyblob." => state()?,
        Err(error) => return Err(error),
    };
    let enabled = current["enabled"] == true;
    Ok(json!({"enabled":enabled}))
}
#[tauri::command]
pub fn configure_anyblob(enabled: bool) -> Result<Value, String> {
    let _guard = crate::acquire_save_guard("Wait for the current save before changing Anyblob.")?;
    configure_at(&bridge_path()?, &blobui_mcp::credential::path()?, enabled)?;
    Ok(json!({"enabled":enabled}))
}

fn saved() -> Result<crate::codex::Configuration, String> {
    let data = crate::load_settings_data(&install()?)?;
    Ok(crate::settings_from_data(&data).codex_integration)
}

pub(crate) fn readable(config: &crate::codex::Configuration, space: &str) -> bool {
    config.spaces.get(space).is_some_and(|p| p.permits("read"))
}
fn allowed(config: &crate::codex::Configuration, op: &Operation) -> Result<(), String> {
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
    let unavailable = |error: String| {
        if error == "Full-text search exceeds the safe 1000-result snapshot limit. Narrow the search and retry." {
            return error;
        }
        "Anytype API v2 could not complete this read. Check the saved API connection, object ID and Anytype availability.".to_string()
    };
    match op {
        Operation::Blobui { .. } => {
            Err("BlobUI configuration requires the authorized publication path.".into())
        }
        Operation::Content { .. } => {
            Err("Content operations require the authorized mutation path.".into())
        }
        Operation::SearchMentions { .. } => {
            Err("Metadata searches require the authenticated global search path.".into())
        }
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
            let document = client
                .content_request("GET", &path, None, None, None, None)
                .map_err(unavailable)?;
            if document.get("_http_error").is_some() {
                return Err(unavailable(document.to_string()));
            }
            let etag = document["etag"]
                .as_str()
                .filter(|etag| !etag.is_empty())
                .ok_or("Anytype did not return an object ETag. Re-read the object before patching.")?;
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
            result["etag"] = json!(etag);
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
#[cfg(test)]
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
#[cfg(windows)]
fn pipe_prompt(
    pipe: &impl Bridge,
    operation_id: &str,
    op: &Operation,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<(), String> {
    use blobui_mcp::approval::{Frame, VERSION};
    let deadline = Instant::now() + Duration::from_secs(120);
    let _prompt = acquire_prompt(check, deadline)?;
    check()?;
    let approval_id = format!("approval-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let cancel = AtomicBool::new(false);
    let frame = Frame::ApprovalRequest {
        version: VERSION,
        operation_id: operation_id.into(),
        approval_id: approval_id.clone(),
        message: blobui_mcp::approval::describe(op),
    };
    pipe.send(
        &serde_json::to_vec(&frame).map_err(|_| "Invalid approval")?,
        &cancel,
        deadline,
    )
    .map_err(|e| e.to_string())?;
    let frame: Frame =
        serde_json::from_slice(&pipe.receive(&cancel, deadline).map_err(|e| e.to_string())?)
            .map_err(|_| "Invalid approval decision")?;
    check()?;
    if Instant::now() >= deadline {
        return Err("Anyblob approval timed out.".into());
    }
    match frame {
        Frame::ApprovalDecision {
            version,
            operation_id: id,
            approval_id: aid,
            accepted: true,
        } if version == VERSION && id == operation_id && aid == approval_id => Ok(()),
        _ => Err("Anyblob approval denied or invalid.".into()),
    }
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
    if let Operation::SearchMentions { query } = &request.request {
        let revision = REVISION.load(Ordering::Acquire);
        let check = || {
            if !connected() {
                return Err("MCP request cancelled or disconnected.".into());
            }
            let current = load_state()?;
            authenticate(&current, &request.token)?;
            if revision != REVISION.load(Ordering::Acquire)
                || current != initial
                || load_config()?.object_mentions != config.object_mentions
            {
                return Err(
                    "Anyblob mention filters or integration changed. Retry the search.".into(),
                );
            }
            Ok(())
        };
        check()?;
        let result =
            crate::anyblob_mentions::search(&connect()?, &config.object_mentions, query, &check)?;
        check()?;
        return Ok(result);
    }
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
        Ok(())
    };
    check()?;
    if let Operation::Content { content } = &request.request {
        if content.tool == "list_creation_targets"
            || (content.tool == "create_object" && content.arguments.get("destination").is_some())
        {
            let client = connect()?;
            return crate::anyblob_creation::dispatch_global(
                &client,
                &config,
                content,
                &check,
                &approve,
            );
        }
    }
    if let Operation::Blobui { blobui } = &request.request {
        let client = connect()?;
        return crate::anyblob_blobui::execute_at(
            &install()?,
            &client,
            &config,
            blobui,
            &check,
            &approve,
        );
    }
    if let Operation::Content { content } = &request.request {
        let client = connect()?;
        crate::anyblob_content::permission(&config, content, false)?;
        if content.arguments["create_missing_options"] == true {
            let mut wide = content.clone();
            wide.tool = "create_property".into();
            crate::anyblob_content::permission(&config, &wide, false)?;
        }
        let mut prepared = crate::anyblob_content::prepare(&client, content, &config, &check)?;
        let inspected_files = std::cell::Cell::new(prepared.files);
        let inspect_check = || -> Result<(), String> {
            check()?;
            crate::anyblob_content::permission(&config, content, inspected_files.get())?;
            Ok(())
        };
        crate::anyblob_content::inspect(
            &client,
            &mut prepared,
            &config,
            &inspect_check,
        )?;
        inspected_files.set(prepared.files);
        let permission = crate::anyblob_content::permission(&config, content, prepared.files)?;
        let needs_confirmation = content.requires_confirmation(&permission.approval);
        let validation = crate::anyblob_content::preflight(
            &client,
            &mut prepared,
            &config,
            &check,
        )?;
        if needs_confirmation {
            approve(&prepared.approval_prompt(&client, &check)?, &check)?;
        }
        if needs_confirmation {
            crate::anyblob_content::inspect(
                &client,
                &mut prepared,
                &config,
                &inspect_check,
            )?;
            inspected_files.set(prepared.files);
            if content.mutates() && content.arguments["dry_run"] != true {
                crate::anyblob_content::preflight(
                    &client,
                    &mut prepared,
                    &config,
                    &check,
                )?;
            }
        }
        if blobui_mcp::sources::needs_display_context(&request.request) && !needs_confirmation {
            let _ = prepared.approval_prompt(&client, &inspect_check)?;
        }
        let files = prepared.files;
        let before_dispatch = || {
            check()?;
            crate::anyblob_content::permission(&config, content, files)?;
            if content.arguments["create_missing_options"] == true {
                let mut wide = content.clone();
                wide.tool = "create_property".into();
                crate::anyblob_content::permission(&config, &wide, false)?;
            }
            Ok(())
        };
        if let Some(result) = validation {
            before_dispatch()?;
            return Ok(result);
        }
        // Every preflight and commit rechecks saved space permissions.
        let mut result = crate::anyblob_content::execute_validated(
            &client,
            &mut prepared,
            &check,
            &before_dispatch,
            &config,
        )?;
        if !content.mutates() {
            check()?;
            crate::anyblob_content::permission(&config, content, false)?;
        }
        if blobui_mcp::sources::needs_display_context(&request.request) {
            result["_anyblob_source"] = json!({"name":prepared.source_context["name"],"space":prepared.source_context["space"]});
        }
        return Ok(result);
    }
    allowed(&config, &request.request)?;
    let approval = request
        .request
        .space()
        .and_then(|space| config.spaces.get(space));
    if approval.is_some_and(|p| p.approval == "alwaysAsk") {
        approve(&request.request, &check)?;
    }
    check()?;
    let client = connect()?;
    let mut result = guarded_read(&client, &config, &request.request, &check)?;
    if let Operation::ReadObject { space_id, object_id } = &request.request {
        result["_anyblob_source"] = crate::anyblob_content::display_context(
            &client,
            space_id,
            Some(object_id),
            &check,
        )?;
    }
    check()?;
    Ok(result)
}
#[cfg(windows)]
fn dispatch(
    pipe: &impl Bridge,
    request: Request,
    connected: impl Fn() -> bool,
) -> Result<Value, String> {
    let operation_id = request.id.clone();
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
        |op, check| pipe_prompt(pipe, &operation_id, op, check),
    )
}

#[cfg(all(feature = "headless-cli", not(windows)))]
pub(crate) fn dispatch_headless(
    request: Request,
    cancel: &std::sync::atomic::AtomicBool,
    approval: std::sync::Arc<blobui_mcp::server::Approval>,
) -> Result<Response, String> {
    let deadline = || Instant::now() + Duration::from_secs(120);
    let result = dispatch_with(
        request,
        || !cancel.load(std::sync::atomic::Ordering::Acquire),
        state,
        saved,
        crate::kanban_api::Client::connect,
        move |operation, check| {
            let end = deadline();
            let _guard = acquire_prompt(check, end)?;
            check()?;
            let remaining = end.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Approval queue timed out. Retry the request.".into());
            }
            let accepted = approval(&blobui_mcp::approval::describe(operation), remaining)?;
            check()?;
            if accepted {
                Ok(())
            } else {
                Err("The operation was not approved.".into())
            }
        },
    );
    Ok(match result {
        Ok(value) => Response {
            result: Some(value),
            error: None,
        },
        Err(error) => Response {
            result: None,
            error: Some(error),
        },
    })
}
#[cfg(any(windows, test))]
fn publication_event(operation: &Operation, result: &Result<Value, String>) -> Option<Value> {
    let Operation::Blobui { blobui } = operation else {
        return None;
    };
    let value = result.as_ref().ok()?;
    if !blobui.mutates()
        || blobui.arguments["dry_run"] == true
        || value["outcome"] != "confirmed"
        || value["publication_status"] != "published"
    {
        return None;
    }
    Some(
        json!({"sections": if blobui.calendar() { vec!["calendarRules"] } else { vec!["formulas"] },
        "revision":value["revision"], "publishedAt":value["published_at"]}),
    )
}

pub fn start(_app: tauri::AppHandle) {
    if migrate().is_err() {
        eprintln!("Anyblob credential setup needs attention in Settings.");
    }
    #[cfg(windows)]
    std::thread::spawn(move || {
        use blobui_mcp::transport::{Pipe, CALL_TIMEOUT};
        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        loop {
            let mut listener = match Pipe::listen(active.load(Ordering::Acquire) == 0) {
                Ok(p) => p,
                Err(error) => {
                    eprintln!("Anyblob bridge could not start: {error}");
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };
            loop {
                match listener.accept() {
                    Ok(false) => std::thread::sleep(Duration::from_millis(30)),
                    Err(error) => {
                        eprintln!("Anyblob bridge accept failed: {error}");
                        break;
                    }
                    Ok(true) => {
                        let next = match Pipe::listen(false) {
                            Ok(p) => p,
                            Err(error) => {
                                eprintln!("Anyblob bridge listener failed: {error}");
                                break;
                            }
                        };
                        let pipe = std::mem::replace(&mut listener, next);
                        if active.load(Ordering::Acquire) >= 15 {
                            drop(pipe);
                            continue;
                        }
                        active.fetch_add(1, Ordering::AcqRel);
                        let active = active.clone();
                        let app = _app.clone();
                        std::thread::spawn(move || {
                            let cancel = AtomicBool::new(false);
                            let deadline = Instant::now() + CALL_TIMEOUT;
                            let start = Instant::now();
                            if let Ok(bytes) =
                                pipe.receive(&cancel, Instant::now() + Duration::from_secs(5))
                            {
                                if let Ok(envelope) = serde_json::from_slice::<Value>(&bytes) {
                                    if envelope["version"] != blobui_mcp::approval::VERSION {
                                        let response = Response {
                                            result: None,
                                            error: Some(blobui_mcp::approval::UPDATE.into()),
                                        };
                                        let _ = pipe.send(
                                            &serde_json::to_vec(
                                                &blobui_mcp::approval::Frame::Response {
                                                    version: blobui_mcp::approval::VERSION,
                                                    response,
                                                },
                                            )
                                            .unwrap(),
                                            &cancel,
                                            deadline,
                                        );
                                        active.fetch_sub(1, Ordering::AcqRel);
                                        return;
                                    }
                                    let Ok(request) = serde_json::from_value::<Request>(
                                        envelope["request"].clone(),
                                    ) else {
                                        active.fetch_sub(1, Ordering::AcqRel);
                                        return;
                                    };
                                    let operation = request.request.name().to_string();
                                    let id = NEXT.fetch_add(1, Ordering::Relaxed);
                                    let requested_operation = request.request.clone();
                                    let result = dispatch(&pipe, request, || {
                                        Instant::now() < deadline && pipe.connected()
                                    });
                                    if let Some(payload) =
                                        publication_event(&requested_operation, &result)
                                    {
                                        use tauri::Emitter;
                                        let _ = app.emit("blobui-saved-state-changed", payload);
                                    }
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
                                    if let Ok(bytes) =
                                        serde_json::to_vec(&blobui_mcp::approval::Frame::Response {
                                            version: blobui_mcp::approval::VERSION,
                                            response,
                                        })
                                    {
                                        let _ = pipe.send(&bytes, &cancel, deadline);
                                    }
                                }
                            }
                            active.fetch_sub(1, Ordering::AcqRel);
                        });
                    }
                }
            }
            drop(listener);
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    #[cfg(not(windows))]
    let _ = _app;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_state_notifications_only_follow_confirmed_blobui_publications() {
        let confirmed = Ok(
            json!({"outcome":"confirmed","publication_status":"published","revision":"rev","published_at":"stamp"}),
        );
        for tool in [
            "set_kanban_calculation",
            "remove_kanban_calculation",
            "refresh_kanban_calculation",
            "set_calendar_color_rule",
            "remove_calendar_color_rule",
            "refresh_calendar_colors",
        ] {
            let mut operation = Operation::Blobui {
                blobui: blobui_mcp::blobui::BlobuiRequest {
                    tool: tool.into(),
                    arguments: json!({}),
                },
            };
            let payload = publication_event(&operation, &confirmed).unwrap();
            assert_eq!(payload["revision"], "rev");
            assert_eq!(
                payload["sections"][0],
                if tool.contains("calendar") {
                    "calendarRules"
                } else {
                    "formulas"
                }
            );
            assert!(publication_event(&operation, &Err("rolled back".into())).is_none());
            assert!(publication_event(
                &operation,
                &Ok(json!({"outcome":"preview","publication_status":"unchanged"}))
            )
            .is_none());
            if let Operation::Blobui { blobui } = &mut operation {
                blobui.arguments["dry_run"] = json!(true);
            }
            assert!(publication_event(&operation, &confirmed).is_none());
        }
        for tool in [
            "preview_kanban_calculation",
            "preview_calendar_color_rule",
            "get_calendar_color_rule",
            "list_kanban_views",
        ] {
            let operation = Operation::Blobui {
                blobui: blobui_mcp::blobui::BlobuiRequest {
                    tool: tool.into(),
                    arguments: json!({}),
                },
            };
            assert!(publication_event(&operation, &confirmed).is_none());
        }
        assert!(publication_event(&Operation::ListSpaces {}, &confirmed).is_none());
    }
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
    #[test]
    fn content_access_denial_precedes_http_and_approval_follows_preflight() {
        let operation = || {
            blobui_mcp::operation(
                "create_object",
                json!({"space_id":"space","body":{"type":"page","name":"fixture"}}),
            )
            .unwrap()
        };
        for access in ["none", "read", "readWrite"] {
            let config = config(access, "fullAccess");
            let result = dispatch_with_test_adapter(
                request(operation()),
                || true,
                || Ok(enabled()),
                || Ok(config.clone()),
                || {
                    Ok(crate::kanban_api::Client::with_connection(
                        "fixture-key",
                        "127.0.0.1",
                        1,
                    ))
                },
                |_, _| panic!("Denied requests must not prompt"),
                || Ok(None),
            );
            assert!(result.unwrap_err().contains("Access denied"));
        }
        for approval in ["alwaysAsk", "approveForMe"] {
            let config = config("createDelete", approval);
            let captured_body = json!({"type":"page","name":"fixture"});
            let (client, captured, server) = mock_recording(
                vec![
                    (
                        "POST /v2/spaces/space/objects?dry_run=true ".into(),
                        json!({"_fixture_request_body":captured_body,"dry_run":true}),
                    ),
                    (
                        "GET /v2/spaces?ids=full&limit=100&offset=0 ".into(),
                        json!({"data":[{"id":"space","name":"Work"}],"has_more":false}),
                    ),
                ],
                None,
            );
            let prompted = std::cell::Cell::new(false);
            let result = dispatch_with_test_adapter(
                request(operation()),
                || true,
                || Ok(enabled()),
                || Ok(config.clone()),
                || Ok(client.clone_for_fixture()),
                |op, _| {
                    prompted.set(true);
                    assert_eq!(op.name(), "create_object");
                    assert!(captured.lock().unwrap()[0]
                        .starts_with("POST /v2/spaces/space/objects?dry_run=true "));
                    Err("User denied mutation".into())
                },
                || Ok(None),
            );
            assert!(
                prompted.get(),
                "dispatch returned {:?}, requests {:?}",
                result,
                *captured.lock().unwrap()
            );
            assert!(result.unwrap_err().contains("User denied mutation"));
            server.join().unwrap();
            let requests = captured.lock().unwrap();
            assert_eq!(requests.len(), 2);
            assert!(requests
                .iter()
                .all(|request| !request.starts_with("POST /v2/spaces/space/objects ")));
        }
    }
    #[test]
    fn dry_run_create_validates_without_confirmation_or_write() {
        let operation = || {
            blobui_mcp::operation(
                "create_object",
                json!({"space_id":"space","body":{"type":"page","name":"fixture"},"dry_run":true}),
            )
            .unwrap()
        };

        for access in ["none", "read", "readWrite"] {
            let config = config(access, "alwaysAsk");
            let result = dispatch_with_test_adapter(
                request(operation()),
                || true,
                || Ok(enabled()),
                || Ok(config.clone()),
                || {
                    Ok(crate::kanban_api::Client::with_connection(
                        "fixture-key",
                        "127.0.0.1",
                        1,
                    ))
                },
                |_, _| panic!("Denied dry_run requests must not prompt"),
                || Ok(None),
            );
            assert!(result.unwrap_err().contains("Access denied"));
        }

        for approval in ["alwaysAsk", "approveForMe"] {
            let config = config("createDelete", approval);
            let captured_body = json!({"type":"page","name":"fixture"});
            let (client, captured, server) = mock_recording(
                vec![(
                    "POST /v2/spaces/space/objects?dry_run=true ".into(),
                    json!({"_fixture_request_body":captured_body,"valid":true}),
                )],
                None,
            );
            let prompted = std::cell::Cell::new(false);
            let result = dispatch_with_test_adapter(
                request(operation()),
                || true,
                || Ok(enabled()),
                || Ok(config.clone()),
                || Ok(client.clone_for_fixture()),
                |_, _| {
                    prompted.set(true);
                    Err("Dry-run requests must not prompt".into())
                },
                || Ok(None),
            )
            .unwrap();

            assert!(!prompted.get(), "dry_run requested approval");
            assert_eq!(result["outcome"], "validated");
            assert_eq!(result["receipt"]["valid"], true);
            server.join().unwrap();
            let requests = captured.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert!(requests[0].starts_with("POST /v2/spaces/space/objects?dry_run=true "));
            assert!(!requests[0].starts_with("POST /v2/spaces/space/objects "));
        }
    }
    #[test]
    fn invalid_create_option_returns_guidance_without_approval_or_creation() {
        let body = json!({"type":"page","name":"fixture","properties":{"status":{"select":{"name":"Todo"}}}});
        let (client, captured, server) = mock_recording(
            vec![
                (
                    "GET /v2/spaces/space/properties?ids=full&limit=100&offset=0 ".into(),
                    json!({"data":[{"key":"status","name":"Status","format":"select"}],"has_more":false}),
                ),
                (
                    "POST /v2/spaces/space/objects?dry_run=true ".into(),
                    json!({"_fixture_status":400,"_fixture_request_body":body,"error":{"code":"validation_failed","message":"Request validation failed","issues":[{"path":"body.properties.status","message":"Option does not exist"}]}}),
                ),
                (
                    "GET /v2/spaces/space/properties?ids=full&limit=100&offset=0 ".into(),
                    json!({"data":[{"key":"status","name":"Status","format":"select"}],"has_more":false}),
                ),
                (
                    "GET /v2/spaces/space/properties/status/options?ids=full&limit=100&offset=0 "
                        .into(),
                    json!({"data":[{"name":"Open"},{"name":"Done"}],"has_more":false}),
                ),
            ],
            None,
        );
        let saved = config("createDelete", "alwaysAsk");
        let result = dispatch_with_test_adapter(
            request(
                blobui_mcp::operation("create_object", json!({"space_id":"space","body":body}))
                    .unwrap(),
            ),
            || true,
            || Ok(enabled()),
            || Ok(saved.clone()),
            || Ok(client.clone_for_fixture()),
            |_, _| panic!("Invalid create requests must fail before approval"),
            || Ok(None),
        );
        let error = result.unwrap_err();
        assert!(
            error.contains("Cannot create the object because Status has no option named \"Todo\""),
            "{error}"
        );
        assert!(
            error.contains("Available options for Status are \"Open\", \"Done\""),
            "{error}"
        );
        assert!(
            error.contains("create_missing_options=true")
                && error.contains("space-wide Create-Delete permission"),
            "{error}"
        );
        assert!(
            error.contains("Technical details:") && error.contains("validation_failed"),
            "{error}"
        );
        server.join().unwrap();
        let requests = captured.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(
            requests[0].starts_with("GET /v2/spaces/space/properties?ids=full&limit=100&offset=0 ")
        );
        assert!(requests[1].starts_with("POST /v2/spaces/space/objects?dry_run=true "));
        assert!(requests
            .iter()
            .all(|request| !request.starts_with("POST /v2/spaces/space/objects ")));
    }
    fn config(access: &str, approval: &str) -> crate::codex::Configuration {
        crate::codex::normalize(&json!({"spaces":{"space":{"access":access,"approval":approval}}}))
    }
    fn request(op: Operation) -> Request {
        Request {
            id: "fixture".into(),
            token: "a".repeat(64),
            request: op,
        }
    }
    fn search() -> Operation {
        Operation::SearchObjects {
            space_id: "space".into(),
            query: "private fixture query".into(),
            limit: 20,
            offset: 7,
        }
    }
    #[cfg(test)]
    fn dispatch_with_test_adapter(
        request: Request,
        connected: impl Fn() -> bool,
        load_state: impl Fn() -> Result<Value, String>,
        load_config: impl Fn() -> Result<crate::codex::Configuration, String>,
        connect: impl Fn() -> Result<crate::kanban_api::Client, String>,
        approve: impl Fn(&Operation, &dyn Fn() -> Result<(), String>) -> Result<(), String>,
        _unused: impl Fn() -> Result<Option<()>, String>,
    ) -> Result<Value, String> {
        dispatch_with(request, connected, load_state, load_config, connect, approve)
    }
    fn enabled() -> Value {
        json!({"enabled":true,"verifier":digest(&"a".repeat(64))})
    }
    #[test]
    fn mention_metadata_ignores_read_grants_but_never_authorizes_reads() {
        let config = crate::codex::normalize(
            &json!({"spaces":{"another-space":{"access":"read","approval":"alwaysAsk"}}}),
        );
        let (client, server) = mock(
            vec![
                (
                    "GET /v2/spaces?ids=full&limit=100&offset=0 ".into(),
                    json!({"data":[{"id":"denied","name":"Private"}],"has_more":false}),
                ),
                (
                    "POST /v2/spaces/denied/search?ids=full&limit=100&offset=0 ".into(),
                    json!({"_fixture_request_body":{},"data":[{"id":"object","name":"Visible metadata","markdown":"secret","properties":{"secret":true}}],"has_more":false}),
                ),
            ],
            None,
        );
        let result = dispatch_with_test_adapter(
            request(Operation::SearchMentions {
                query: String::new(),
            }),
            || true,
            || Ok(enabled()),
            || Ok(config.clone()),
            || Ok(client.clone_for_fixture()),
            |_, _| panic!("metadata search must not prompt"),
            || panic!("metadata search does not use identity"),
        )
        .unwrap();
        assert_eq!(result["items"][0]["uri"], "anyblob://objects/denied/object");
        assert!(!result.to_string().contains("secret"));
        server.join().unwrap();
        let read = Operation::ReadObject {
            space_id: "denied".into(),
            object_id: "object".into(),
        };
        assert!(dispatch_with_test_adapter(
            request(read),
            || true,
            || Ok(enabled()),
            || Ok(config.clone()),
            || panic!("denied reads must fail before connecting"),
            |_, _| panic!(),
            || Ok(None)
        )
        .is_err());
        for state in [
            json!({"enabled":false}),
            json!({"enabled":true,"verifier":"invalid"}),
        ] {
            assert!(dispatch_with_test_adapter(
                request(Operation::SearchMentions {
                    query: String::new()
                }),
                || true,
                || Ok(state.clone()),
                || Ok(config.clone()),
                || panic!("authentication first"),
                |_, _| panic!(),
                || panic!()
            )
            .is_err());
        }
    }
    fn mock(
        responses: Vec<(String, Value)>,
        revoke: Option<Arc<AtomicBool>>,
    ) -> (crate::kanban_api::Client, std::thread::JoinHandle<()>) {
        let (client, _, server) = mock_recording(responses, revoke);
        (client, server)
    }
    fn mock_recording(
        responses: Vec<(String, Value)>,
        revoke: Option<Arc<AtomicBool>>,
    ) -> (
        crate::kanban_api::Client,
        Arc<std::sync::Mutex<Vec<String>>>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = requests.clone();
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
                // Accepted sockets inherit nonblocking mode on Windows.
                stream.set_nonblocking(false).unwrap();
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
                captured.lock().unwrap().push(text.clone());
                assert!(text.starts_with(&expected), "{text}");
                assert!(expected.contains("/v2/"));
                if expected.starts_with("POST") {
                    let expected_body = body
                        .get("_fixture_request_body")
                        .cloned()
                        .unwrap_or(json!({"query":"private fixture query"}));
                    assert!(text.ends_with(&expected_body.to_string()));
                }
                if let Some(flag) = &revoke {
                    flag.store(true, Ordering::Release);
                }
                let status = body["_fixture_status"].as_u64().unwrap_or(200);
                let mut response = body;
                let etag_header = response["_fixture_etag_header"]
                    .as_str()
                    .map(|etag| format!("ETag: {etag}\r\n"))
                    .or_else(|| {
                        (expected.starts_with("GET /v2/spaces/")
                            && expected.contains("/objects/")
                            && !expected.contains("format=md"))
                        .then(|| "ETag: \"fixture-object-rev\"\r\n".into())
                    })
                    .unwrap_or_default();
                if let Some(fields) = response.as_object_mut() {
                    fields.remove("_fixture_status");
                    fields.remove("_fixture_request_body");
                    fields.remove("_fixture_etag_header");
                }
                let body = response.to_string();
                write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\n{etag_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        (
            crate::kanban_api::Client::with_connection("fixture-api-key", "127.0.0.1", port),
            requests,
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
                    json!({"_fixture_etag_header":"\"object-rev-1\"","id":"object","type":"page","properties":{"name":"Note","link":["other-object"]},"blocks":[{"type":"file","id":"file-ref","target":"attachment"}]}),
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
        assert_eq!(read["etag"], "\"object-rev-1\"");
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
    #[test]
    fn permissions_and_tokens_fail_closed() {
        for access in ["none", "read", "readWrite", "createDelete"] {
            for approval in ["alwaysAsk", "approveForMe", "fullAccess"] {
                let config = crate::codex::normalize(
                    &json!({"spaces":{"space":{"access":access,"approval":approval,"filesEnabled":true}}}),
                );
                assert_eq!(
                    readable(&config, "space"),
                    ["read", "readWrite", "createDelete"].contains(&access)
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
            json!({"enabled":true,"verifier":digest(&old),"remoteAccess":{"enabled":true}}).to_string(),
        )
        .unwrap();
        assert!(migrate_at(&state_path, &secret).unwrap());
        let token = blobui_mcp::credential::read_at(&secret).unwrap();
        assert_ne!(token, old);
        let value = crate::read_json_file(&state_path).unwrap();
        authenticate(&value, &token).unwrap();
        assert!(value.get("remoteAccess").is_none());
        let mut value = value;
        value["remoteAccess"] = json!({"enabled":true});
        std::fs::write(&state_path, value.to_string()).unwrap();
        assert!(migrate_at(&state_path, &secret).unwrap());
        let value = crate::read_json_file(&state_path).unwrap();
        assert!(value.get("remoteAccess").is_none());
        assert_eq!(blobui_mcp::credential::read_at(&secret).unwrap(), token);
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
            assert!(acl.contains("D:P"), "credential DACL was not protected: {acl}");
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

#[cfg(test)]
#[path = "anyblob_live.rs"]
mod live_tests;
