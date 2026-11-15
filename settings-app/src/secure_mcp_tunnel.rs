//! Starts the official tunnel client beside BlobUI with no public listener.
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
};

static CHILD: Mutex<Option<Child>> = Mutex::new(None);
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

fn runtime_path() -> Option<PathBuf> {
    let names = ["tunnel-client-runtime.exe", "tunnel-client.exe"];
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|directory| names.map(|name| directory.join(name)))
        .find(|path| path.is_file())
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
    let mut child = CHILD
        .lock()
        .map_err(|_| "Secure MCP Tunnel process state is unavailable.")?;
    if let Some(process) = child.as_mut() {
        if process
            .try_wait()
            .map_err(|_| "Could not check Secure MCP Tunnel status.")?
            .is_some()
        {
            *child = None;
            return Ok(false);
        }
        return Ok(true);
    }
    Ok(false)
}

fn stop_process() -> Result<(), String> {
    let mut child = CHILD
        .lock()
        .map_err(|_| "Secure MCP Tunnel process state is unavailable.")?
        .take();
    if let Some(ref mut process) = child {
        if process
            .try_wait()
            .map_err(|_| "Could not check Secure MCP Tunnel status.")?
            .is_none()
        {
            process
                .kill()
                .map_err(|_| "Could not stop the Secure MCP Tunnel client.")?;
            let _ = process.wait();
        }
    }
    remove_health_file();
    Ok(())
}

fn remove_health_file() {
    let Ok(path) = health_path() else {
        return;
    };
    if crate::reject_reparse_component(&path).is_ok() {
        let _ = std::fs::remove_file(path);
    }
}

fn runtime_ready() -> bool {
    use std::{
        io::{BufRead, BufReader, Write},
        net::{SocketAddr, TcpStream, ToSocketAddrs},
        time::Duration,
    };
    let Ok(path) = health_path() else {
        return false;
    };
    if crate::reject_reparse_component(&path).is_err() {
        return false;
    }
    let Ok(value) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(url) = url::Url::parse(value.trim()) else {
        return false;
    };
    if url.scheme() != "http"
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return false;
    }
    let Some(port) = url.port() else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let Ok(addresses) = (host, port).to_socket_addrs() else {
        return false;
    };
    let address = addresses
        .filter(|address| address.ip().is_loopback())
        .next();
    let Some(address) = address else {
        return false;
    };
    let address = SocketAddr::new(address.ip(), address.port());
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(400)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    if write!(
        stream,
        "GET /readyz HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    )
    .is_err()
    {
        return false;
    }
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).is_ok()
        && (response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"))
}

fn start_process(tunnel_id: &str, secret: SecretString) -> Result<(), String> {
    stop_process()?;
    let runtime = runtime_path()
        .ok_or("OpenAI tunnel-client-runtime.exe is not installed or is missing from PATH.")?;
    crate::reject_reparse_component(&runtime)?;
    let mcp = mcp_path()?;
    let health = health_path()?;
    crate::reject_reparse_component(&health)?;
    remove_health_file();
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
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Could not start the OpenAI tunnel-client runtime.")?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    match child.try_wait() {
        Ok(Some(_)) => {
            remove_health_file();
            return Err("The OpenAI tunnel-client runtime stopped during startup. Check its installation and tunnel settings.".into());
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            remove_health_file();
            return Err("Could not check Secure MCP Tunnel status.".into());
        }
        Ok(None) => {}
    }
    *CHILD
        .lock()
        .map_err(|_| "Secure MCP Tunnel process state is unavailable.")? = Some(child);
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
    let ready = running && runtime_ready();
    Ok(json!({
        "enabled": enabled,
        "tunnelId": tunnel_id,
        "keyConfigured": key_configured,
        "runtimeAvailable": runtime_path().is_some(),
        "running": running,
        "ready": ready,
    }))
}

#[tauri::command]
pub fn configure_secure_mcp_tunnel(
    enabled: bool,
    tunnel_id: String,
    runtime_api_key: Option<String>,
) -> Result<Value, String> {
    let _guard = crate::acquire_save_guard("Wait for the current settings save to finish.")?;
    if !enabled {
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
    start_process(tunnel_id, secret)?;
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

pub(crate) fn start_if_configured() {
    let Ok(state) = crate::anyblob::state() else {
        return;
    };
    if state["enabled"] != true || state["secureMcpTunnelEnabled"] != true {
        return;
    }
    let tunnel_id = state["secureMcpTunnelId"].as_str().unwrap_or("");
    if !valid_tunnel_id(tunnel_id) {
        return;
    }
    let Ok(path) = secret_path() else {
        return;
    };
    let Ok(secret) = read_saved_secret(&path) else {
        return;
    };
    if let Err(error) = start_process(tunnel_id, secret) {
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
}
