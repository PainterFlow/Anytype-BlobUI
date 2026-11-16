//! Starts the official tunnel client beside BlobUI with no public listener.
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
};

#[cfg(windows)]
use std::fs::File;

struct TunnelProcess {
    child: Child,
    health_url: String,
    _job: TunnelJob,
    _lock: TunnelLock,
}

#[cfg(windows)]
struct TunnelLock {
    _file: File,
}

#[cfg(not(windows))]
struct TunnelLock;

#[cfg(windows)]
struct TunnelJob(usize);

#[cfg(not(windows))]
struct TunnelJob;

impl TunnelJob {
    fn create() -> Result<Self, String> {
        #[cfg(windows)]
        {
            use std::{mem::size_of, ptr::null};
            use windows_sys::Win32::{
                Foundation::CloseHandle,
                System::JobObjects::{
                    CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
                    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                },
            };

            // The runtime can spawn Anyblob after startup. A job with this limit keeps both
            // processes tied to BlobUI, including when Windows ends BlobUI unexpectedly.
            let job = unsafe { CreateJobObjectW(null(), null()) };
            if job.is_null() {
                return Err("Could not create the Secure MCP Tunnel process group.".into());
            }
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            } != 0;
            if !configured {
                unsafe { CloseHandle(job) };
                return Err("Could not configure the Secure MCP Tunnel process group.".into());
            }
            Ok(Self(job as usize))
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Ok(Self)
        }
    }

    fn attach(&self, child: &Child) -> Result<(), String> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{
                Foundation::HANDLE, System::JobObjects::AssignProcessToJobObject,
            };
            let assigned = unsafe {
                AssignProcessToJobObject(self.0 as HANDLE, child.as_raw_handle() as HANDLE) != 0
            };
            if !assigned {
                return Err(
                    "Could not bind the Secure MCP Tunnel client to BlobUI's process lifetime."
                        .into(),
                );
            }
        }
        #[cfg(not(windows))]
        let _ = child;
        Ok(())
    }

    fn resume(child: &Child) -> Result<(), String> {
        #[cfg(windows)]
        {
            use std::mem::size_of;
            use windows_sys::Win32::{
                Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
                System::{
                    Diagnostics::ToolHelp::{
                        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                        THREADENTRY32,
                    },
                    Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
                },
            };

            let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
            if snapshot == INVALID_HANDLE_VALUE {
                return Err("Could not find the Secure MCP Tunnel startup thread.".into());
            }
            let mut entry = THREADENTRY32 {
                dwSize: size_of::<THREADENTRY32>() as u32,
                ..THREADENTRY32::default()
            };
            let mut found = None;
            let mut has_entry = unsafe { Thread32First(snapshot, &mut entry) != 0 };
            while has_entry {
                if entry.th32OwnerProcessID == child.id() {
                    found = Some(entry.th32ThreadID);
                    break;
                }
                has_entry = unsafe { Thread32Next(snapshot, &mut entry) != 0 };
            }
            unsafe { CloseHandle(snapshot) };
            let Some(thread_id) = found else {
                return Err("Could not find the Secure MCP Tunnel startup thread.".into());
            };
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
            if thread.is_null() {
                return Err("Could not resume the Secure MCP Tunnel client.".into());
            }
            let resumed = unsafe { ResumeThread(thread) } == 1;
            unsafe { CloseHandle(thread) };
            if !resumed {
                return Err("Could not resume the Secure MCP Tunnel client.".into());
            }
        }
        #[cfg(not(windows))]
        let _ = child;
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for TunnelJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(
                self.0 as windows_sys::Win32::Foundation::HANDLE,
            )
        };
    }
}

static PROCESS: Mutex<Option<TunnelProcess>> = Mutex::new(None);
const SECRET_FILE: &str = "Settings/DO-NOT-SHARE/secure-mcp-tunnel.dpapi";
const HEALTH_FILE: &str = "Settings/anyblob/tunnel-health.url";

struct SecretString(String);

impl Drop for SecretString {
    fn drop(&mut self) {
        unsafe { self.0.as_mut_vec().fill(0) }
    }
}

fn secret_path() -> Result<PathBuf, String> {
    let install = crate::resolve_install_directory(&crate::app_data_directory())?;
    Ok(crate::settings_root_for_install(&install)?.join(SECRET_FILE))
}

fn health_path() -> Result<PathBuf, String> {
    let install = crate::resolve_install_directory(&crate::app_data_directory())?;
    Ok(crate::settings_root_for_install(&install)?.join(HEALTH_FILE))
}

fn tunnel_lock_path() -> Result<PathBuf, String> {
    Ok(crate::app_data_directory().join("anyblob-secure-mcp-tunnel.lock"))
}

impl TunnelLock {
    fn acquire() -> Result<Self, String> {
        let path = tunnel_lock_path()?;
        crate::reject_reparse_component(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        match options.open(path) {
            Ok(file) => {
                #[cfg(windows)]
                {
                    Ok(Self { _file: file })
                }
                #[cfg(not(windows))]
                {
                    let _ = file;
                    Ok(Self)
                }
            }
            Err(error) if is_lock_conflict(&error) => Err(
                "Another BlobUI process already owns the Secure MCP Tunnel client. Close that BlobUI instance before starting another client.".into(),
            ),
            Err(_) => Err("Could not claim the Secure MCP Tunnel process lock.".into()),
        }
    }
}

#[cfg(windows)]
fn is_lock_conflict(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32 | 33))
}

#[cfg(not(windows))]
fn is_lock_conflict(_: &io::Error) -> bool {
    false
}

fn runtime_path() -> Option<PathBuf> {
    let names = ["tunnel-client-runtime.exe", "tunnel-client.exe"];
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut directories: Vec<_> = std::env::split_paths(&inherited).collect();
    #[cfg(windows)]
    directories.extend(registered_path_directories());
    directories
        .into_iter()
        .filter(|directory| directory.is_absolute())
        .flat_map(|directory| names.map(|name| directory.join(name)))
        .find(|path| path.is_file())
}

#[cfg(windows)]
fn registered_path_directories() -> Vec<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};

    let mut directories = Vec::new();
    let name: Vec<_> = "Path".encode_utf16().chain([0]).collect();
    for (root, key) in [
        (
            HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ),
        (HKEY_CURRENT_USER, "Environment"),
    ] {
        let key: Vec<_> = key.encode_utf16().chain([0]).collect();
        let mut value = vec![0u16; 32768];
        let mut len = (value.len() * std::mem::size_of::<u16>()) as u32;
        let result = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                std::ptr::null_mut(),
                value.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if result == ERROR_SUCCESS {
            let end = value
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(value.len());
            directories.extend(std::env::split_paths(&OsString::from_wide(&value[..end])));
        }
    }
    directories
}

fn mcp_path() -> Result<PathBuf, String> {
    crate::marketplace::mcp_executable_path()
}

fn mcp_command_argument(path: &std::path::Path) -> String {
    format!("\"{}\"", path.to_string_lossy().replace('\\', "\\\\"))
}

fn valid_tunnel_id(value: &str) -> bool {
    let Some(value) = value.strip_prefix("tunnel_") else {
        return false;
    };
    let id = if value.len() == 32 {
        value
    } else if value.len() == 37 {
        let Some((namespace, id)) = value.split_once('_') else {
            return false;
        };
        if namespace.len() != 4
            || !namespace
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return false;
        }
        id
    } else {
        return false;
    };
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn update_state(update: impl FnOnce(&mut serde_json::Map<String, Value>)) -> Result<Value, String> {
    let path = crate::anyblob::bridge_path()?;
    crate::reject_reparse_component(&path)?;
    let mut state = if path.is_file() {
        crate::read_json_file(&path)?
    } else {
        json!({})
    };
    let fields = state
        .as_object_mut()
        .ok_or("Anyblob settings must contain an object.")?;
    update(fields);
    let bytes = serde_json::to_vec(&state).map_err(|_| "Could not encode Anyblob settings.")?;
    crate::storage::write_atomic(&path, &bytes)?;
    crate::anyblob::invalidate();
    Ok(state)
}

fn read_saved_secret(path: &std::path::Path) -> Result<SecretString, String> {
    let value = blobui_mcp::credential::read_secret_at(path)?;
    if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err("The saved Secure MCP Tunnel key is invalid. Enter it again.".into());
    }
    Ok(SecretString(value))
}

fn save_secret(path: &std::path::Path, secret: &str) -> Result<(), String> {
    let encrypted = blobui_mcp::credential::encrypt_secret(secret)?;
    let staged = crate::storage::prepare_temp(path, &encrypted, None, true)?;
    let result = crate::storage::replace_credential(&staged, path)
        .map_err(|_| "Could not save the Secure MCP Tunnel key.".to_string());
    let _ = std::fs::remove_file(staged);
    result
}

fn process_running() -> Result<bool, String> {
    let mut process = PROCESS
        .lock()
        .map_err(|_| "Secure MCP Tunnel process state is unavailable.")?;
    if let Some(current) = process.as_mut() {
        if current
            .child
            .try_wait()
            .map_err(|_| "Could not check Secure MCP Tunnel status.")?
            .is_some()
        {
            let health_url = current.health_url.clone();
            *process = None;
            remove_health_file_if_matches(&health_url);
            return Ok(false);
        }
        return Ok(true);
    }
    Ok(false)
}

fn stop_process() -> Result<(), String> {
    let mut process = PROCESS
        .lock()
        .map_err(|_| "Secure MCP Tunnel process state is unavailable.")?;
    stop_process_locked(&mut process)
}

fn stop_process_locked(process: &mut Option<TunnelProcess>) -> Result<(), String> {
    let Some(current) = process.as_mut() else {
        return Ok(());
    };
    let health_url = current.health_url.clone();
    let status = current
        .child
        .try_wait()
        .map_err(|_| "Could not check Secure MCP Tunnel status.")?;
    if status.is_none() {
        if current.child.kill().is_err()
            && current
                .child
                .try_wait()
                .map_err(|_| "Could not check Secure MCP Tunnel status.")?
                .is_none()
        {
            return Err("Could not stop the Secure MCP Tunnel client.".into());
        }
        current
            .child
            .wait()
            .map_err(|_| "Could not wait for the Secure MCP Tunnel client to stop.")?;
    }
    *process = None;
    remove_health_file_if_matches(&health_url);
    Ok(())
}

fn remove_health_file_if_matches(expected_url: &str) {
    let Ok(path) = health_path() else {
        return;
    };
    if crate::reject_reparse_component(&path).is_ok()
        && std::fs::read_to_string(&path)
            .map(|value| value.trim() == expected_url)
            .unwrap_or(false)
    {
        let _ = std::fs::remove_file(path);
    }
}

fn health_url_from_file() -> Result<Option<String>, String> {
    let path = health_path()?;
    crate::reject_reparse_component(&path)?;
    match std::fs::read_to_string(path) {
        Ok(value) => {
            let value = value.trim().to_owned();
            validate_health_url(&value)?;
            Ok(Some(value))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Could not read the Secure MCP Tunnel health URL.".into()),
    }
}

fn validate_health_url(value: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "The Secure MCP Tunnel health URL is invalid.")?;
    if url.scheme() != "http"
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err("The Secure MCP Tunnel health URL is invalid.".into());
    }
    let host = url
        .host_str()
        .ok_or("The Secure MCP Tunnel health URL is invalid.")?;
    if !matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1") || url.port().is_none() {
        return Err("The Secure MCP Tunnel health URL must use a loopback address.".into());
    }
    Ok(url)
}

fn health_get(url_value: &str, route: &str) -> Result<Option<(u16, String)>, String> {
    use std::{
        io::{Read, Write},
        net::{TcpStream, ToSocketAddrs},
        time::Duration,
    };
    if !route.starts_with('/') || route.bytes().any(|byte| matches!(byte, b'?' | b'#' | b' ')) {
        return Err("Invalid Secure MCP Tunnel health route.".into());
    }
    let url = validate_health_url(url_value)?;
    let host = url
        .host_str()
        .ok_or("The Secure MCP Tunnel health URL is invalid.")?;
    let port = url
        .port()
        .ok_or("The Secure MCP Tunnel health URL is invalid.")?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let address = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let socket = (host, port)
        .to_socket_addrs()
        .map_err(|_| "The Secure MCP Tunnel health URL is invalid.")?
        .find(|address| address.ip().is_loopback())
        .ok_or("The Secure MCP Tunnel health URL must use a loopback address.")?;
    let mut stream = match TcpStream::connect_timeout(&socket, Duration::from_millis(400)) {
        Ok(stream) => stream,
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => return Ok(None),
        Err(_) => return Err("Could not verify the Secure MCP Tunnel health listener.".into()),
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(400)))
        .map_err(|_| "Could not read the Secure MCP Tunnel health response.")?;
    stream
        .set_write_timeout(Some(Duration::from_millis(400)))
        .map_err(|_| "Could not send the Secure MCP Tunnel health request.")?;
    write!(
        stream,
        "GET {route} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|_| "Could not send the Secure MCP Tunnel health request.")?;
    let mut response = String::new();
    stream
        .take(64 * 1024)
        .read_to_string(&mut response)
        .map_err(|_| "Could not read the Secure MCP Tunnel health response.")?;
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or("The Secure MCP Tunnel health response is invalid.")?;
    if status == 200 {
        Ok(Some((status, body.to_owned())))
    } else if status == 404 || status == 503 {
        Ok(None)
    } else {
        Err("Could not verify the Secure MCP Tunnel health listener.".into())
    }
}

fn runtime_ready(health_url: Option<&str>) -> bool {
    health_url
        .and_then(|url| health_get(url, "/readyz").ok().flatten())
        .is_some()
}

fn mcp_health_summary(health_url: Option<&str>) -> Value {
    let Some(health_url) = health_url else {
        return json!({"available":false,"status":"unknown","state":"not_running"});
    };
    let Some((_, body)) = health_get(health_url, "/health/mcp").ok().flatten() else {
        return json!({"available":false,"status":"unknown","state":"unavailable"});
    };
    let Ok(value) = serde_json::from_str::<Value>(&body) else {
        return json!({"available":false,"status":"unknown","state":"invalid_response"});
    };
    summarize_mcp_health(&value)
}

fn summarize_mcp_health(value: &Value) -> Value {
    let details = &value["details"];
    let tools = &details["tools_list"];
    let state = value["state"].as_str().unwrap_or("unknown");
    let status = value["status"].as_str().unwrap_or("unknown");
    json!({
        "available": true,
        "status": status,
        "state": state,
        "reasonCode": value["reason_code"],
        "discovered": status == "ok" && state == "discovered" && tools["ok"] == true && tools["complete"] == true,
        "serverName": details["initialize"]["server_name"],
        "serverVersion": details["initialize"]["server_version"],
        "toolCount": tools["retained_count"],
        "toolsComplete": tools["complete"] == true,
    })
}

fn start_process(tunnel_id: &str, secret: SecretString) -> Result<(), String> {
    let runtime = runtime_path()
        .ok_or("OpenAI tunnel-client-runtime.exe is not installed or is missing from PATH.")?;
    crate::reject_reparse_component(&runtime)?;
    let mcp = mcp_path()?;
    let health = health_path()?;
    crate::reject_reparse_component(&health)?;
    let mut process = PROCESS
        .lock()
        .map_err(|_| "Secure MCP Tunnel process state is unavailable.")?;
    stop_process_locked(&mut process)?;
    let process_lock = TunnelLock::acquire()?;
    if let Some(existing_url) = health_url_from_file()? {
        match health_get(&existing_url, "/healthz")? {
            Some(_) => {
                return Err(
                    "An existing Secure MCP Tunnel client is still active for this BlobUI installation. BlobUI will not start a duplicate client. Stop the existing client from its owning BlobUI process, then try again.".into(),
                );
            }
            None => remove_health_file_if_matches(&existing_url),
        }
    }
    let process_job = TunnelJob::create()?;
    let mut command = Command::new(runtime);
    command
        .arg("run")
        .arg("--control-plane.tunnel-id")
        .arg(tunnel_id)
        .arg("--mcp.command")
        .arg(mcp_command_argument(&mcp))
        .arg("--health.listen-addr")
        .arg("127.0.0.1:0")
        .arg("--health.url-file")
        .arg(health)
        .env("CONTROL_PLANE_API_KEY", &secret.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(
            windows_sys::Win32::System::Threading::CREATE_NO_WINDOW
                | windows_sys::Win32::System::Threading::CREATE_SUSPENDED,
        );
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Could not start the OpenAI tunnel-client runtime.")?;
    if let Err(error) = process_job.attach(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    if let Err(error) = TunnelJob::resume(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let startup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let health_url = loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return Err("The OpenAI tunnel-client runtime stopped during startup. Check its installation and tunnel settings.".into());
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Could not check Secure MCP Tunnel status.".into());
            }
            Ok(None) => {}
        }
        match health_url_from_file() {
            Ok(Some(url)) => {
                break url;
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
        if std::time::Instant::now() >= startup_deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "The tunnel-client runtime did not publish its health URL during startup.".into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    *process = Some(TunnelProcess {
        child,
        health_url,
        _job: process_job,
        _lock: process_lock,
    });
    Ok(())
}

fn save_configuration(enabled: bool, tunnel_id: &str) -> Result<Value, String> {
    update_state(|fields| {
        fields.insert("secureMcpTunnelEnabled".into(), json!(enabled));
        fields.insert("secureMcpTunnelId".into(), json!(tunnel_id));
    })
}

#[tauri::command]
pub fn secure_mcp_tunnel_status() -> Result<Value, String> {
    let state = crate::anyblob::state()?;
    let enabled = state["enabled"] == true && state["secureMcpTunnelEnabled"] == true;
    let tunnel_id = state["secureMcpTunnelId"].as_str().unwrap_or("");
    let key_configured = secret_path()
        .ok()
        .and_then(|path| read_saved_secret(&path).ok())
        .is_some();
    let running = process_running()?;
    let health_url = PROCESS
        .lock()
        .map_err(|_| "Secure MCP Tunnel process state is unavailable.")?
        .as_ref()
        .map(|process| process.health_url.clone());
    let active_health_url = if running {
        health_url.clone()
    } else {
        health_url_from_file().ok().flatten()
    };
    let ready = running && runtime_ready(health_url.as_deref());
    let another_runtime_active = !running
        && active_health_url
            .as_deref()
            .and_then(|url| health_get(url, "/healthz").ok().flatten())
            .is_some();
    Ok(json!({
        "enabled": enabled,
        "tunnelId": tunnel_id,
        "keyConfigured": key_configured,
        "runtimeAvailable": runtime_path().is_some(),
        "running": running,
        "ready": ready,
        "anotherRuntimeActive": another_runtime_active,
        "mcp": mcp_health_summary(health_url.as_deref()),
    }))
}

#[tauri::command]
pub fn configure_secure_mcp_tunnel(
    enabled: bool,
    tunnel_id: String,
    runtime_api_key: Option<String>,
    start: Option<bool>,
) -> Result<Value, String> {
    let _guard = crate::acquire_save_guard("Wait for the current settings save to finish.")?;
    if !enabled {
        if let Some(key) = runtime_api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
        {
            if key.len() > 4096 || key.chars().any(char::is_control) {
                return Err("The Secure MCP Tunnel key is invalid.".into());
            }
            save_secret(&secret_path()?, key)?;
        } else if crate::anyblob::state()?["secureMcpTunnelId"]
            .as_str()
            .unwrap_or("")
            != tunnel_id.trim()
        {
            let path = secret_path()?;
            if path.exists() {
                std::fs::remove_file(path)
                    .map_err(|_| "Could not remove the previous tunnel key.")?;
            }
        }
        stop_process()?;
        return save_configuration(false, tunnel_id.trim());
    }
    if crate::anyblob::state()?["enabled"] != true {
        return Err("Enable Anyblob before starting the Secure MCP Tunnel.".into());
    }
    let tunnel_id = tunnel_id.trim();
    if !valid_tunnel_id(tunnel_id) {
        return Err("Enter a valid Secure MCP Tunnel ID.".into());
    }
    let path = secret_path()?;
    let current = crate::anyblob::state()?;
    let secret = match runtime_api_key
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(value) if value.len() <= 4096 && !value.chars().any(char::is_control) => {
            save_secret(&path, &value)?;
            SecretString(value)
        }
        Some(_) => return Err("The Secure MCP Tunnel key is invalid.".into()),
        None if current["secureMcpTunnelId"].as_str() == Some(tunnel_id) => {
            read_saved_secret(&path)?
        }
        None => return Err("Enter the runtime API key for this tunnel.".into()),
    };
    if start.unwrap_or(true) {
        start_process(tunnel_id, secret)?;
    }
    if let Err(error) = save_configuration(true, tunnel_id) {
        let _ = stop_process();
        return Err(error);
    }
    secure_mcp_tunnel_status()
}

pub(crate) fn disable_for_anyblob() {
    let _ = stop_process();
    let tunnel_id = crate::anyblob::state()
        .ok()
        .and_then(|state| state["secureMcpTunnelId"].as_str().map(String::from))
        .unwrap_or_default();
    let _ = save_configuration(false, &tunnel_id);
}

#[tauri::command]
pub fn refresh_secure_mcp_tunnel(restart: bool) -> Result<Value, String> {
    let _guard = crate::acquire_save_guard("Wait for the current settings save to finish.")?;
    let running = process_running()?;
    if (if restart { running } else { !running }) && (restart || runtime_path().is_some()) {
        start_saved_tunnel()?;
    }
    secure_mcp_tunnel_status()
}

fn start_saved_tunnel() -> Result<(), String> {
    let state = crate::anyblob::state()?;
    if state["enabled"] != true || state["secureMcpTunnelEnabled"] != true {
        return Ok(());
    }
    let tunnel_id = state["secureMcpTunnelId"].as_str().unwrap_or("");
    if !valid_tunnel_id(tunnel_id) {
        return Err("Enter a valid Secure MCP Tunnel ID.".into());
    }
    start_process(tunnel_id, read_saved_secret(&secret_path()?)?)
}

pub(crate) fn start_if_configured() {
    if let Err(error) = start_saved_tunnel() {
        eprintln!("Secure MCP Tunnel did not start: {error}");
    }
}

pub(crate) fn stop_for_exit() -> Result<(), String> {
    stop_process()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_path_escapes_windows_backslashes_inside_quotes() {
        let path =
            std::path::Path::new(r"C:\Users\First Last\.codex\plugins\anyblob\bin\blobui-mcp.exe");
        assert_eq!(
            mcp_command_argument(path),
            r#""C:\\Users\\First Last\\.codex\\plugins\\anyblob\\bin\\blobui-mcp.exe""#
        );
    }

    #[test]
    fn tunnel_ids_are_safe_for_process_arguments() {
        assert!(valid_tunnel_id("tunnel_0123456789abcdef0123456789abcdef"));
        assert!(valid_tunnel_id(
            "tunnel_ab12_0123456789abcdef0123456789abcdef"
        ));
        let too_long = format!("tunnel_{}", "x".repeat(129));
        for invalid in [
            "",
            "short",
            "has space 123",
            "https://example.com",
            too_long.as_str(),
        ] {
            assert!(!valid_tunnel_id(invalid));
        }
    }

    #[test]
    fn health_urls_are_loopback_only_and_have_no_path_or_credentials() {
        assert!(validate_health_url("http://127.0.0.1:8080").is_ok());
        assert!(validate_health_url("http://[::1]:8080/").is_ok());
        for value in [
            "https://127.0.0.1:8080",
            "http://example.com:8080",
            "http://127.0.0.1:8080/healthz",
            "http://user@127.0.0.1:8080",
        ] {
            assert!(validate_health_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn tunnel_ready_does_not_claim_mcp_discovery() {
        let health = json!({
            "status":"degraded",
            "state":"failed",
            "reason_code":"mcp_discovery_error",
            "details":{"tools_list":{"ok":false,"complete":false,"retained_count":0}}
        });
        let summary = summarize_mcp_health(&health);
        assert_eq!(summary["discovered"], false);
        assert_eq!(summary["state"], "failed");
        assert_eq!(summary["reasonCode"], "mcp_discovery_error");
    }

    #[test]
    fn mcp_discovery_requires_a_complete_tools_list() {
        let health = json!({
            "status":"ok",
            "state":"discovered",
            "details":{
                "initialize":{"server_name":"anyblob","server_version":"0.3.15"},
                "tools_list":{"ok":true,"complete":true,"retained_count":64}
            }
        });
        let summary = summarize_mcp_health(&health);
        assert_eq!(summary["discovered"], true);
        assert_eq!(summary["serverName"], "anyblob");
        assert_eq!(summary["serverVersion"], "0.3.15");
        assert_eq!(summary["toolCount"], 64);
    }
}
