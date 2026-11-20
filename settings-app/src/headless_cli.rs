use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::mem::MaybeUninit;
#[cfg(unix)]
use std::sync::atomic::AtomicI32;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
#[cfg(any(test, not(windows)))]
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
#[cfg(not(windows))]
use std::{
    net::TcpStream,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::Mutex,
};

const ANYTYPE_API_PORT: u16 = 31012;
#[cfg(not(windows))]
const ANYTYPE_API_START_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MAX_ARCHIVE_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MCP_TOKEN: &str = "blobui-headless-stdio-session";
#[cfg(not(windows))]
static ANYTYPE_SERVE_ATTEMPTED: AtomicBool = AtomicBool::new(false);
#[cfg(not(windows))]
static ANYTYPE_SERVE_LOCK: Mutex<()> = Mutex::new(());

pub fn entry() {
    let result = run(env::args().skip(1).collect());
    if let Err(error) = result {
        eprintln!("blob-ui: {error}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    if args.is_empty() || args[0] == "help" || args[0] == "?" || args[0] == "--help" {
        print_help();
        return Ok(());
    }
    match args[0].as_str() {
        "--build-channel" => {
            if args.len() != 2 {
                return Err("Usage: blob-ui --build-channel <main|dev>".into());
            }
            set_headless_build_channel(&args[1])
        }
        "-v" | "--version" => {
            println!("BlobUI Headless {}", env!("BLOBUI_BUILD_VERSION"));
            Ok(())
        }
        "info" => {
            if args.len() != 1 {
                return Err("Usage: blob-ui info".into());
            }
            info()
        }
        "update" => {
            if args.len() > 2 || (args.len() == 2 && !matches!(args[1].as_str(), "-f" | "--force"))
            {
                return Err("Usage: blob-ui update [-f]".into());
            }
            update(
                args.get(1)
                    .is_some_and(|arg| arg == "-f" || arg == "--force"),
            )
        }
        "api" => api_command(&args[1..]),
        "perms" => global_perms_command(&args[1..]),
        "spaces" => spaces_command(&args[1..]),
        "object-mention" => object_mention_command(&args[1..]),
        "--toggle-anyblob" => {
            if args.len() != 2 {
                return Err("Usage: blob-ui --toggle-anyblob <bool>".into());
            }
            let value = args
                .get(1)
                .ok_or("Expected `true` or `false` after --toggle-anyblob.")?;
            toggle_anyblob(parse_bool(value)?)
        }
        "import" => {
            if args.len() != 2 {
                return Err("Usage: blob-ui import <path.zip>".into());
            }
            let path = args.get(1).ok_or("Usage: blob-ui import <path.zip>")?;
            import_archive(Path::new(path))
        }
        "export" => {
            if args.len() != 2 {
                return Err("Usage: blob-ui export <path.zip>".into());
            }
            let path = args.get(1).ok_or("Usage: blob-ui export <path.zip>")?;
            export_archive(Path::new(path))
        }
        "mcp" => {
            run_mcp();
            Ok(())
        }
        other => return Err(format!("Unknown command `{other}`. Run `blob-ui help`.")),
    }
}

fn print_help() {
    println!(
        "BlobUI Headless {}\n\
         Usage: blob-ui <command>\n\
         \n\
         Commands:\n\
           help | ?                         Show this help\n\
           --build-channel <main|dev>         Select the update channel (default: main)\n\
           info                              Show version and update status\n\
           update [-f]                      Check for and install an update\n\
           api status|login [-f]|logout      Manage the Anytype API v2 key\n\
           api --host-ip <IP> [--host-port <PORT>]  Set the Anytype API endpoint\n\
           perms <space-id> ...             Manage global space permissions\n\
           perms ls                         List global permissions\n\
           spaces ls                       List Anytype spaces and their IDs\n\
           object-mention ls|add|remove ... Manage object type filters\n\
           object-mention spaces ls|add|remove ... Manage mention spaces\n\
           object-mention --filter-done <bool> Filter completed task mentions\n\
           --toggle-anyblob <bool>          Enable or disable Anyblob\n\
           import <path.zip> | export <path.zip>  Import or export settings\n\
           mcp                               Start the stdio MCP server\n\
         Permission values are read, read-write, or create-delete. Approval values are always, never, or auto.\n\
         Headless settings live in {}",
        env!("BLOBUI_BUILD_VERSION"),
        crate::headless_home_directory()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("unavailable ({error})"))
    );
}

fn ensure_settings() -> Result<PathBuf, String> {
    let root = crate::prepare_headless_home_directory()?;
    let _ = crate::settings_data_path(&root)?;
    Ok(root)
}

fn load_data() -> Result<(PathBuf, Value), String> {
    let root = ensure_settings()?;
    let data = crate::load_settings_data(&root)?;
    Ok((root, data))
}

fn save_data(root: &Path, data: &Value) -> Result<(), String> {
    crate::settings_store::save(root, data)
}

fn info() -> Result<(), String> {
    println!("BlobUI Headless {}", env!("BLOBUI_BUILD_VERSION"));
    let channel = selected_headless_build_channel()?;
    println!("Build channel: {}", channel.as_headless_value());
    match latest_headless_release(channel) {
        Ok(Some(release)) => {
            println!("Latest version: {}", release.version_label);
            if headless_release_is_newer(&release.version_label, env!("BLOBUI_BUILD_VERSION")) {
                println!("An update is available. Run `blob-ui update`.");
            } else {
                println!("BlobUI Headless is up to date.");
            }
        }
        Ok(None) => println!("No BlobUI Headless release is available yet."),
        Err(error) => println!("Update check failed: {error}"),
    }
    Ok(())
}

fn set_headless_build_channel(value: &str) -> Result<(), String> {
    if !matches!(value, "main" | "dev") {
        return Err("Build channel must be `main` or `dev`.".into());
    }
    let (root, mut data) = load_data()?;
    let object = data.as_object_mut().ok_or("Headless settings must be a JSON object.")?;
    object.insert("headlessBuildChannel".into(), Value::String(value.into()));
    save_data(&root, &data)?;
    println!("BlobUI Headless build channel set to {value}.");
    Ok(())
}

fn selected_headless_build_channel() -> Result<crate::updater::ReleaseChannel, String> {
    let (_, data) = load_data()?;
    match data.get("headlessBuildChannel").and_then(Value::as_str) {
        None | Some("main") => Ok(crate::updater::ReleaseChannel::Release),
        Some("dev") => Ok(crate::updater::ReleaseChannel::Dev),
        Some(_) => Err("The saved BlobUI Headless build channel is invalid. Run `blob-ui --build-channel main` to reset it.".into()),
    }
}

fn api_command(args: &[String]) -> Result<(), String> {
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "--host-ip" | "--host-port"))
    {
        return configure_api_endpoint(args);
    }
    match args.first().map(String::as_str) {
        Some("status") if args.len() == 1 => {
            let (root, data) = load_data()?;
            let (ip, port) = configured_api_endpoint(&data)?;
            let key_status = match crate::read_saved_api_key_at(&root) {
                Ok(key) => {
                    drop(crate::SecretString::new(key));
                    true
                }
                Err(_) => false,
            };
            println!(
                "Anytype API v2: {}\nEndpoint: {}",
                if data
                    .pointer("/apiConnection/enabled")
                    .and_then(Value::as_bool)
                    == Some(true)
                    && key_status
                {
                    "configured"
                } else {
                    "not configured"
                },
                format_api_endpoint(&ip, port)
            );
        }
        Some("logout") if args.len() == 1 => {
            let (root, mut data) = load_data()?;
            let (ip, port) = configured_api_endpoint(&data)?;
            let credential = crate::api_credential_path(&root)?;
            let previous_key = read_api_key_snapshot(&credential)?;
            if previous_key.is_some() {
                fs::remove_file(&credential)
                    .map_err(|_| "Could not remove the saved Anytype API v2 key.".to_string())?;
            }
            data["apiConnection"] = api_connection(&data, false, &ip, port);
            if let Err(error) = save_data(&root, &data) {
                rollback_api_key(&credential, previous_key.as_deref()).map_err(|rollback| {
                    format!("{error} API credential rollback failed: {rollback}")
                })?;
                return Err(error);
            }
            println!("Logged out of the Anytype API v2 connection.");
        }
        Some("login")
            if args.len() <= 2
                && args
                    .get(1)
                    .is_none_or(|arg| matches!(arg.as_str(), "-f" | "--force")) =>
        {
            api_login(
                args.get(1)
                    .is_some_and(|arg| arg == "-f" || arg == "--force"),
            )?
        }
        _ => {
            return Err(
                "Usage: blob-ui api status|login [-f]|logout | --host-ip <IP> [--host-port <PORT>]"
                    .into(),
            )
        }
    }
    Ok(())
}

fn api_login(force: bool) -> Result<(), String> {
    let (root, mut data) = load_data()?;
    let (ip, port) = configured_api_endpoint(&data)?;
    let credential = crate::api_credential_path(&root)?;
    if credential.is_file() && !force {
        return Err(
            "An Anytype API v2 key is already saved. Use `blob-ui api login -f` to replace it."
                .into(),
        );
    }
    let key = crate::validate_api_key(prompt_secret("Anytype API v2 key: ")?)?;
    #[cfg(not(windows))]
    ensure_anytype_serve(&ip, port)?;
    crate::api::verify_connection_quick(&key, &ip, port).map_err(|error| {
        format!(
            "Could not connect to the Anytype Headless API at {}: {error}",
            format_api_endpoint(&ip, port)
        )
    })?;
    let previous_key = read_api_key_snapshot(&credential)?;
    store_api_key(&credential, &key)?;
    data["apiConnection"] = api_connection(&data, true, &ip, port);
    if let Err(error) = save_data(&root, &data) {
        rollback_api_key(&credential, previous_key.as_deref())
            .map_err(|rollback| format!("{error} API credential rollback failed: {rollback}"))?;
        return Err(error);
    }
    println!("Anytype API v2 key saved for {}.", format_api_endpoint(&ip, port));
    Ok(())
}

fn configure_api_endpoint(args: &[String]) -> Result<(), String> {
    let (root, mut data) = load_data()?;
    let (current_ip, current_port) = configured_api_endpoint(&data)?;
    let (ip, port) = parse_api_endpoint_options(args, &current_ip, current_port)?;
    let enabled = data
        .pointer("/apiConnection/enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    data["apiConnection"] = api_connection(&data, enabled, &ip, port);
    save_data(&root, &data)?;
    println!("Anytype API endpoint set to {}.", format_api_endpoint(&ip, port));
    Ok(())
}

fn parse_api_endpoint_options(
    args: &[String],
    default_ip: &str,
    default_port: u16,
) -> Result<(String, u16), String> {
    let mut ip = default_ip.to_string();
    let mut port = default_port;
    let mut host_ip_seen = false;
    let mut host_port_seen = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--host-ip" if !host_ip_seen => {
                let value = args
                    .get(index + 1)
                    .ok_or("Usage: blob-ui api --host-ip <IP> [--host-port <PORT>]")?;
                ip = crate::normalize_api_ip(Some(value))?;
                host_ip_seen = true;
                index += 2;
            }
            "--host-port" if !host_port_seen => {
                let value = args
                    .get(index + 1)
                    .ok_or("Usage: blob-ui api --host-ip <IP> [--host-port <PORT>]")?;
                port = value
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .ok_or("API host port must be a number from 1 to 65535.")?;
                host_port_seen = true;
                index += 2;
            }
            _ => {
                return Err("Usage: blob-ui api --host-ip <IP> [--host-port <PORT>]".into())
            }
        }
    }
    if !host_ip_seen && !host_port_seen {
        return Err("Usage: blob-ui api --host-ip <IP> [--host-port <PORT>]".into());
    }
    Ok((ip, port))
}

fn configured_api_endpoint(data: &Value) -> Result<(String, u16), String> {
    let ip = crate::normalize_api_ip(data.pointer("/apiConnection/ip").and_then(Value::as_str))?;
    let port = data
        .pointer("/apiConnection/port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .unwrap_or(ANYTYPE_API_PORT);
    Ok((ip, port))
}

fn format_api_endpoint(ip: &str, port: u16) -> String {
    ip.parse::<std::net::IpAddr>()
        .map(|ip| std::net::SocketAddr::new(ip, port).to_string())
        .unwrap_or_else(|_| format!("{ip}:{port}"))
}

fn prompt_secret(prompt: &str) -> Result<String, String> {
    eprint!("{prompt}");
    std::io::stderr()
        .flush()
        .map_err(|_| "Could not write the API key prompt.".to_string())?;
    #[cfg(windows)]
    let result = read_secret_windows();
    #[cfg(unix)]
    let result = read_secret_unix();
    #[cfg(not(any(windows, unix)))]
    let result = Err("Hidden API key entry is unavailable on this platform.".to_string());
    eprintln!();
    result
}

#[cfg(unix)]
static ACTIVE_TERMINAL_FD: AtomicI32 = AtomicI32::new(-1);
#[cfg(unix)]
static mut SAVED_TERMINAL: MaybeUninit<libc::termios> = MaybeUninit::uninit();

#[cfg(unix)]
unsafe extern "C" fn restore_terminal_on_signal(signal: libc::c_int) {
    let fd = ACTIVE_TERMINAL_FD.load(std::sync::atomic::Ordering::Relaxed);
    if fd >= 0 {
        let saved = std::ptr::addr_of!(SAVED_TERMINAL).cast::<libc::termios>();
        libc::tcsetattr(fd, libc::TCSANOW, saved);
    }
    libc::_exit(128 + signal);
}

#[cfg(unix)]
struct UnixTerminalEchoGuard {
    fd: libc::c_int,
    original: libc::termios,
    previous_handlers: Vec<(libc::c_int, libc::sigaction)>,
}

#[cfg(unix)]
impl UnixTerminalEchoGuard {
    fn disable() -> Result<Self, String> {
        let fd = libc::STDIN_FILENO;
        let mut original = unsafe { MaybeUninit::<libc::termios>::zeroed().assume_init() };
        if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
            return Err("API key login requires an interactive terminal.".into());
        }
        unsafe {
            std::ptr::addr_of_mut!(SAVED_TERMINAL).write(MaybeUninit::new(original));
        }
        ACTIVE_TERMINAL_FD.store(fd, std::sync::atomic::Ordering::Release);

        let mut action = unsafe { MaybeUninit::<libc::sigaction>::zeroed().assume_init() };
        action.sa_sigaction = restore_terminal_on_signal as *const () as usize;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
        }
        let mut previous_handlers = Vec::new();
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
            let mut previous = unsafe { MaybeUninit::<libc::sigaction>::zeroed().assume_init() };
            if unsafe { libc::sigaction(signal, &action, &mut previous) } != 0 {
                for (installed_signal, handler) in previous_handlers.iter().rev() {
                    unsafe {
                        libc::sigaction(*installed_signal, handler, std::ptr::null_mut());
                    }
                }
                ACTIVE_TERMINAL_FD.store(-1, std::sync::atomic::Ordering::Release);
                return Err("Could not prepare terminal signal handling for API key input.".into());
            }
            previous_handlers.push((signal, previous));
        }

        let mut hidden = original;
        hidden.c_lflag &= !libc::ECHO;
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &hidden) } != 0 {
            for (signal, handler) in previous_handlers.iter().rev() {
                unsafe {
                    libc::sigaction(*signal, handler, std::ptr::null_mut());
                }
            }
            ACTIVE_TERMINAL_FD.store(-1, std::sync::atomic::Ordering::Release);
            return Err("Could not hide API key input.".into());
        }
        Ok(Self {
            fd,
            original,
            previous_handlers,
        })
    }
}

#[cfg(unix)]
impl Drop for UnixTerminalEchoGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
        }
        for (signal, handler) in self.previous_handlers.iter().rev() {
            unsafe {
                libc::sigaction(*signal, handler, std::ptr::null_mut());
            }
        }
        ACTIVE_TERMINAL_FD.store(-1, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(unix)]
fn read_secret_unix() -> Result<String, String> {
    let guard = UnixTerminalEchoGuard::disable()?;
    let mut value = String::new();
    let read = std::io::stdin().read_line(&mut value);
    drop(guard);
    read.map_err(|_| "Could not read the Anytype API v2 key.".to_string())?;
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
    Ok(value)
}

#[cfg(windows)]
fn read_secret_windows() -> Result<String, String> {
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_ECHO_INPUT, STD_INPUT_HANDLE,
    };
    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    let mut mode = 0;
    if handle.is_null() || unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
        return Err("API key login requires an interactive terminal.".into());
    }
    if unsafe { SetConsoleMode(handle, mode & !ENABLE_ECHO_INPUT) } == 0 {
        return Err("Could not hide API key input.".into());
    }
    let mut value = String::new();
    let read = std::io::stdin().read_line(&mut value);
    let restored = unsafe { SetConsoleMode(handle, mode) };
    if restored == 0 {
        return Err("Could not restore terminal input after reading the API key.".into());
    }
    read.map_err(|_| "Could not read the Anytype API v2 key.".to_string())?;
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
    Ok(value)
}

fn store_api_key(path: &Path, key: &str) -> Result<(), String> {
    let saved = encrypt_api_key(key)?;
    crate::storage::write_atomic_private(path, saved.as_bytes())
        .map_err(|_| "Could not save the Anytype API v2 key.".to_string())
}

fn api_connection(data: &Value, enabled: bool, ip: &str, port: u16) -> Value {
    let mut connection = data
        .get("apiConnection")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    connection.insert("enabled".into(), json!(enabled));
    connection.insert("ip".into(), json!(ip));
    connection.insert("port".into(), json!(port));
    Value::Object(connection)
}

fn read_api_key_snapshot(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && !crate::is_windows_reparse_point(&metadata) =>
        {
            if metadata.len() > 16 * 1024 {
                return Err(
                    "The saved Anytype API credential is too large to manage safely.".into(),
                );
            }
            fs::read(path)
                .map(Some)
                .map_err(|_| "Could not read the saved Anytype API credential.".into())
        }
        Ok(_) => Err("The saved API credential is not a regular file.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Could not inspect the saved API credential.".into()),
    }
}

fn rollback_api_key(path: &Path, previous: Option<&[u8]>) -> Result<(), String> {
    if let Some(previous) = previous {
        return crate::storage::write_atomic_private(path, previous);
    }
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && !crate::is_windows_reparse_point(&metadata) =>
        {
            fs::remove_file(path)
                .map_err(|_| "Could not remove the newly saved API credential.".into())
        }
        Ok(_) => Err("The API credential changed to an unsupported file during rollback.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Could not inspect the API credential during rollback.".into()),
    }
}

fn encrypt_api_key(key: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        crate::connection::encrypt_api_key(key)
    }
    #[cfg(unix)]
    {
        crate::validate_api_key(key.to_string())?;
        Ok(key.to_string())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = key;
        Err("Secure headless API key storage is unavailable on this platform.".into())
    }
}

fn spaces_command(args: &[String]) -> Result<(), String> {
    if args.len() != 1 || args[0] != "ls" {
        return Err("Usage: blob-ui spaces ls".into());
    }
    let client = crate::kanban_api::Client::connect()?;
    let spaces = client.pages("GET", "/v2/spaces?ids=full", None)?;
    let rows = space_list_rows(spaces)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&rows).unwrap_or_default()
    );
    Ok(())
}

fn space_list_rows(spaces: Vec<Value>) -> Result<Vec<Value>, String> {
    spaces
        .into_iter()
        .map(|space| {
            let id = space
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or("Anytype returned a space without an ID.")?;
            Ok(json!({
                "id": id,
                "name": space.get("name").and_then(Value::as_str).unwrap_or("Unnamed space")
            }))
        })
        .collect()
}

fn global_perms_command(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) == Some("ls") {
        if args.len() != 1 {
            return Err("Usage: blob-ui perms ls".into());
        }
        let (_, data) = load_data()?;
        let entries = data
            .pointer("/settings/codexIntegration/spaces")
            .cloned()
            .unwrap_or_else(|| json!({}));
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).unwrap_or_default()
        );
        return Ok(());
    }
    let space_id = args.first().ok_or("Usage: blob-ui perms <space-id> --approval <value> --permission <value> --file-uploads <bool>")?;
    validate_cli_id(space_id, "space")?;
    blobui_mcp::Operation::ReadObject {
        space_id: space_id.clone(),
        object_id: space_id.clone(),
    }
    .validate()?;
    let permission = permission_from_args(&args[1..], false)?;
    let client = crate::kanban_api::Client::connect()?;
    let spaces = client.pages("GET", "/v2/spaces?ids=full", None)?;
    let space = spaces
        .iter()
        .find(|space| space["id"].as_str() == Some(space_id))
        .ok_or("The space ID was not found in the authenticated Anytype account.")?;
    let (root, mut data) = load_data()?;
    data["settings"]["codexIntegration"]["spaces"][space_id] = permission_value(&permission);
    save_data(&root, &data)?;
    println!(
        "Saved global permission for `{}`.",
        space["name"].as_str().unwrap_or(space_id)
    );
    Ok(())
}

fn permission_from_args(
    args: &[String],
    allow_space_id: bool,
) -> Result<crate::codex::Permission, String> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let name = args[index].as_str();
        if !matches!(name, "--permission" | "--approval" | "--file-uploads")
            && !(allow_space_id && name == "--space-id")
        {
            return Err(format!("Unknown permission option `{name}`."));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("Missing a value after {name}."))?;
        if values.insert(name, value.as_str()).is_some() {
            return Err(format!(
                "Permission option `{name}` was passed more than once."
            ));
        }
        index += 2;
    }
    let access = values
        .get("--permission")
        .copied()
        .ok_or("Pass --permission read|read-write|create-delete.")?;
    let access = match access {
        "read" => "read",
        "read-write" => "readWrite",
        "create-delete" => "createDelete",
        _ => return Err("Permission must be read, read-write, or create-delete.".into()),
    };
    let approval = values
        .get("--approval")
        .copied()
        .ok_or("Pass --approval always|never|auto.")?;
    let approval = match approval {
        "always" => "alwaysAsk",
        "never" => "fullAccess",
        "auto" => "approveForMe",
        _ => return Err("Approval must be always, never, or auto.".into()),
    };
    let files = values
        .get("--file-uploads")
        .copied()
        .ok_or("Pass --file-uploads true|false.")?;
    Ok(crate::codex::Permission {
        access: access.into(),
        approval: approval.into(),
        files_enabled: parse_bool(files)?,
    })
}

fn permission_value(permission: &crate::codex::Permission) -> Value {
    json!({"access":permission.access,"approval":permission.approval,"filesEnabled":permission.files_enabled})
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err("Expected true or false.".into()),
    }
}

fn validate_cli_id(value: &str, label: &str) -> Result<(), String> {
    blobui_mcp::Operation::ReadObject {
        space_id: value.to_string(),
        object_id: value.to_string(),
    }
    .validate()
    .map_err(|_| format!("The {label} ID is invalid."))
}

fn toggle_anyblob(enabled: bool) -> Result<(), String> {
    let path = crate::anyblob::bridge_path()?;
    let mut state = if path.is_file() {
        crate::read_json_file(&path)?
    } else {
        json!({})
    };
    let fields = state
        .as_object_mut()
        .ok_or("Anyblob settings must contain a JSON object.")?;
    fields.insert("enabled".into(), json!(enabled));
    fields.insert(
        "verifier".into(),
        json!(format!("{:x}", Sha256::digest(MCP_TOKEN.as_bytes()))),
    );
    crate::storage::write_atomic(
        &path,
        &serde_json::to_vec(&state).map_err(|_| "Could not encode Anyblob settings.")?,
    )?;
    crate::anyblob::invalidate();
    println!(
        "Anyblob is {}.",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

fn object_mention_command(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) == Some("--filter-done") {
        let value = args
            .get(1)
            .ok_or("Usage: blob-ui object-mention --filter-done <bool>")?;
        if args.len() != 2 {
            return Err("Usage: blob-ui object-mention --filter-done <bool>".into());
        }
        let filter = parse_bool(value)?;
        let (root, mut data) = load_data()?;
        let mut configuration = crate::settings_from_data(&data).codex_integration;
        configuration.object_mentions.filter_incomplete_tasks = filter;
        data["settings"]["codexIntegration"] = serde_json::to_value(configuration)
            .map_err(|_| "Could not encode object mention filters.")?;
        save_data(&root, &data)?;
        println!(
            "Incomplete task filtering is {}.",
            if filter { "enabled" } else { "disabled" }
        );
        return Ok(());
    }
    let (root, mut data) = load_data()?;
    let mut configuration = crate::settings_from_data(&data).codex_integration;
    match args.first().map(String::as_str) {
        Some("ls") => {
            if args.len() != 1 { return Err("Usage: blob-ui object-mention ls".into()); }
            println!("{}", serde_json::to_string_pretty(&configuration.object_mentions).unwrap_or_default());
            return Ok(());
        }
        Some("spaces") => return mention_spaces_command(&args[1..], &mut data, &mut configuration, &root),
        Some("add") | Some("remove") => {
            if args.len() != 2 { return Err("Usage: blob-ui object-mention add|remove <object-type-id|space-id/object-type-id>".into()); }
            let selection_id = args.get(1).ok_or("Usage: blob-ui object-mention add|remove <object-type-id|space-id/object-type-id>")?;
            let (requested_space, id) = match selection_id.split_once('/') {
                Some((space, id)) => { validate_cli_id(space, "space")?; validate_cli_id(id, "object type")?; (Some(space), id) }
                None => { validate_cli_id(selection_id, "object type")?; (None, selection_id.as_str()) }
            };
            let client = crate::kanban_api::Client::connect()?;
            let rows = crate::codex::discover_sources(&client)?;
            let types: Vec<_> = rows.iter().filter(|row| row["kind"] == "type" && row["id"].as_str() == Some(id)
                && requested_space.is_none_or(|space| row["spaceId"].as_str() == Some(space))).collect();
            let type_row = match types.as_slice() {
                [row] => *row,
                [] => return Err(format!("Object type ID `{id}` was not found.")),
                _ => return Err("This object type ID appears in more than one space. Use <space-id>/<object-type-id>.".into()),
            };
            let selection = &mut configuration.object_mentions.types;
            if args[0] == "add" {
                if selection.all { selection.all = false; selection.items.clear(); }
                if !selection.items.iter().any(|item| item.space_id == type_row["spaceId"] && item.id == id) {
                    selection.items.push(crate::anyblob_mentions::ObjectType {
                        space_id: type_row["spaceId"].as_str().unwrap_or_default().into(),
                        id: id.to_string(),
                        name: type_row["name"].as_str().unwrap_or("Unnamed type").into(),
                        space: type_row["space"].as_str().unwrap_or("Unnamed space").into(),
                    });
                }
            } else {
                if selection.all {
                    return Err("The object type filter is set to All. Add an object type first to create an explicit selection, then remove it.".into());
                }
                selection.items.retain(|item| !(item.space_id == type_row["spaceId"] && item.id == id));
            }
        }
        _ => return Err("Usage: blob-ui object-mention ls|add|remove <object-type-id>|spaces ls|add|remove <space-id>".into()),
    }
    data["settings"]["codexIntegration"] = serde_json::to_value(configuration)
        .map_err(|_| "Could not encode object mention filters.")?;
    save_data(&root, &data)?;
    println!("Saved object mention filters.");
    Ok(())
}

fn mention_spaces_command(
    args: &[String],
    data: &mut Value,
    configuration: &mut crate::codex::Configuration,
    root: &Path,
) -> Result<(), String> {
    let (action, id) = parse_mention_space_args(args)?;
    match action {
        "ls" => {
            let client = crate::kanban_api::Client::connect()?;
            let rows = client.pages("GET", "/v2/spaces?ids=full", None)?;
            let result = mark_space_selection(rows, &configuration.object_mentions.spaces)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
            return Ok(());
        }
        "add" | "remove" => {
            let id = id.ok_or("Usage: blob-ui object-mention spaces add|remove <space-id>")?;
            validate_cli_id(id, "space")?;
            let client = crate::kanban_api::Client::connect()?;
            let rows = client.pages("GET", "/v2/spaces?ids=full", None)?;
            let row = rows
                .iter()
                .find(|row| row["id"].as_str() == Some(id))
                .ok_or("Space ID was not found in the authenticated Anytype account.")?;
            let selection = &mut configuration.object_mentions.spaces;
            if action == "add" {
                if selection.all {
                    selection.all = false;
                    selection.items.clear();
                }
                if !selection.items.iter().any(|item| item.id == *id) {
                    selection.items.push(crate::anyblob_mentions::Space {
                        id: id.to_string(),
                        name: row["name"].as_str().unwrap_or("Unnamed space").into(),
                    });
                }
            } else {
                if selection.all {
                    return Err("The space filter is set to All. Add a space first to create an explicit selection, then remove it.".into());
                }
                selection.items.retain(|item| item.id != *id);
            }
        }
        _ => return Err("Usage: blob-ui object-mention spaces ls|add|remove <space-id>".into()),
    }
    data["settings"]["codexIntegration"] = serde_json::to_value(configuration.clone())
        .map_err(|_| "Could not encode object mention filters.")?;
    save_data(root, data)?;
    println!("Saved object mention space filters.");
    Ok(())
}

fn parse_mention_space_args(args: &[String]) -> Result<(&str, Option<&str>), String> {
    match args {
        [action] if action == "ls" => Ok(("ls", None)),
        [action, id] if action == "add" || action == "remove" => Ok((action, Some(id))),
        _ => Err("Usage: blob-ui object-mention spaces ls|add|remove <space-id>".into()),
    }
}

fn mark_space_selection(
    mut rows: Vec<Value>,
    selection: &crate::anyblob_mentions::Selection<crate::anyblob_mentions::Space>,
) -> Result<Vec<Value>, String> {
    for row in &mut rows {
        let object = row
            .as_object_mut()
            .ok_or("Anytype returned a space without an object.")?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or("Anytype returned a space without an ID.")?
            .to_string();
        let selected = selection.all || selection.items.iter().any(|space| space.id == id);
        object.insert("selected".into(), json!(selected));
    }
    Ok(rows)
}

struct HeadlessRelease {
    version: semver::Version,
    version_label: String,
    asset_url: String,
    checksum_url: String,
}

fn latest_headless_release(channel: crate::updater::ReleaseChannel) -> Result<Option<HeadlessRelease>, String> {
    let response = crate::updater::http_get(
        "api.github.com",
        "/repos/PainterFlow/Anytype-BlobUI/releases?per_page=100",
        "application/vnd.github+json",
        4 * 1024 * 1024,
    )?;
    if response.status != 200 {
        return Err(format!(
            "GitHub release check returned HTTP {}.",
            response.status
        ));
    }
    let releases: Value = serde_json::from_slice(&response.body)
        .map_err(|_| "GitHub returned an invalid release list.".to_string())?;
    let releases = releases
        .as_array()
        .ok_or("GitHub returned an invalid release list.")?;
    let current_asset = if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "blob-ui-windows-x86_64.exe"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "blob-ui-linux-x86_64"
    } else {
        return Err("Self-updates are available only for x86_64 Windows and Linux builds.".into());
    };
    let mut candidates = Vec::new();
    for release in releases {
        let Some(tag) = release["tag_name"].as_str() else {
            continue;
        };
        let Some((version_text, version_label)) = headless_tag_version(tag, channel) else {
            continue;
        };
        let Ok(version) = semver::Version::parse(version_text) else {
            continue;
        };
        if release.get("prerelease").and_then(Value::as_bool)
            .is_some_and(|prerelease| prerelease != (channel == crate::updater::ReleaseChannel::Dev))
        {
            continue;
        }
        let Some(assets) = release["assets"].as_array() else {
            continue;
        };
        let asset = assets
            .iter()
            .find(|asset| asset["name"].as_str() == Some(current_asset));
        let checksum = assets
            .iter()
            .find(|asset| asset["name"].as_str() == Some(&format!("{current_asset}.sha256")));
        let (Some(asset), Some(checksum)) = (asset, checksum) else {
            continue;
        };
        let (Some(asset_url), Some(checksum_url)) = (
            asset["browser_download_url"].as_str(),
            checksum["browser_download_url"].as_str(),
        ) else {
            continue;
        };
        validate_release_asset_url(asset_url, tag, current_asset)?;
        validate_release_asset_url(checksum_url, tag, &format!("{current_asset}.sha256"))?;
        candidates.push(HeadlessRelease {
            version_label,
            asset_url: asset_url.into(),
            checksum_url: checksum_url.into(),
            version,
        });
    }
    candidates.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(candidates.into_iter().next())
}

fn headless_tag_version(
    tag: &str,
    channel: crate::updater::ReleaseChannel,
) -> Option<(&str, String)> {
    match channel {
        crate::updater::ReleaseChannel::Release => {
            let version = tag.strip_prefix("headless-v")?;
            Some((version, version.to_string()))
        }
        crate::updater::ReleaseChannel::Dev => {
            let version = tag.strip_prefix("headless-dev-")?;
            Some((version, format!("dev-{version}")))
        }
    }
}

fn headless_release_is_newer(candidate: &str, current: &str) -> bool {
    let candidate_base = candidate.strip_prefix("dev-").unwrap_or(candidate);
    let current_base = current.strip_prefix("dev-").unwrap_or(current);
    let (Ok(candidate_version), Ok(current_version)) = (
        semver::Version::parse(candidate_base),
        semver::Version::parse(current_base),
    ) else {
        return false;
    };
    candidate_version > current_version
        || (candidate_version == current_version && candidate != current)
}

fn validate_release_asset_url(value: &str, tag: &str, name: &str) -> Result<(), String> {
    let parsed =
        url::Url::parse(value).map_err(|_| "The headless release asset URL is invalid.")?;
    let expected_path = format!("/PainterFlow/Anytype-BlobUI/releases/download/{tag}/{name}");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some_and(|port| port != 443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != expected_path
    {
        return Err("GitHub returned an unexpected headless release asset URL.".into());
    }
    Ok(())
}

fn download_url(value: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let parsed = url::Url::parse(value).map_err(|_| "The release asset URL is invalid.")?;
    let host = parsed
        .host_str()
        .ok_or("The release asset URL has no host.")?;
    let path = format!(
        "{}{}",
        parsed.path(),
        parsed
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default()
    );
    let response = crate::updater::http_get(host, &path, "application/octet-stream", max_bytes)?;
    if response.status != 200 {
        return Err(format!(
            "GitHub asset download returned HTTP {}.",
            response.status
        ));
    }
    Ok(response.body)
}

fn update(force: bool) -> Result<(), String> {
    let channel = selected_headless_build_channel()?;
    let Some(release) = latest_headless_release(channel)? else {
        println!("No BlobUI Headless release is available for this platform yet.");
        return Ok(());
    };
    if !headless_release_is_newer(&release.version_label, env!("BLOBUI_BUILD_VERSION")) {
        println!("BlobUI Headless is up to date at {}.", env!("BLOBUI_BUILD_VERSION"));
        return Ok(());
    }
    if !force {
        eprint!("Install BlobUI Headless {} now? [y/N] ", release.version_label);
        std::io::stderr()
            .flush()
            .map_err(|_| "Could not write the update prompt.")?;
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|_| "Could not read the update response.")?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("Update cancelled.");
            return Ok(());
        }
    }
    println!("Downloading BlobUI Headless {}…", release.version_label);
    let bytes = download_url(&release.asset_url, MAX_ARCHIVE_BYTES)?;
    let sidecar = download_url(&release.checksum_url, 4096)?;
    let expected = std::str::from_utf8(&sidecar)
        .map_err(|_| "The release checksum is invalid.")?
        .split_whitespace()
        .next()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or("The release checksum is invalid.")?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("The downloaded release failed its SHA-256 check.".into());
    }
    replace_current_executable(&bytes)?;
    println!(
        "Updated BlobUI Headless to {}. Restart the command to use the new version.",
        release.version_label
    );
    Ok(())
}

fn replace_current_executable(bytes: &[u8]) -> Result<(), String> {
    let current = env::current_exe()
        .map_err(|_| "Could not locate the running BlobUI Headless executable.")?;
    crate::reject_reparse_component(&current)?;
    if !current.is_file() {
        return Err("The running BlobUI Headless executable is not a regular file.".into());
    }
    let parent = current
        .parent()
        .ok_or("The executable folder is unavailable.")?;
    let temp = parent.join(format!(".blob-ui-update-{}", std::process::id()));
    let backup = parent.join(format!(".blob-ui-old-{}", std::process::id()));
    if temp.exists() || backup.exists() {
        return Err("An earlier BlobUI Headless update is still staged. Remove its temporary files and retry.".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|_| "Could not stage the downloaded BlobUI Headless executable.")?;
    let _cleanup_temp = TemporaryFileCleanup(temp.clone());
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Could not write the downloaded BlobUI Headless executable.")?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&current)
            .map_err(|_| "Could not read executable permissions.")?
            .permissions()
            .mode();
        fs::set_permissions(&temp, fs::Permissions::from_mode(mode))
            .map_err(|_| "Could not preserve executable permissions.")?;
    }
    #[cfg(windows)]
    {
        fs::rename(&current, &backup)
            .map_err(|_| "Close other BlobUI Headless processes and retry the update.")?;
        if let Err(error) = fs::rename(&temp, &current) {
            let _ = fs::rename(&backup, &current);
            return Err(format!(
                "Could not install the downloaded executable: {error}"
            ));
        }
        let _ = fs::remove_file(&backup);
    }
    #[cfg(not(windows))]
    fs::rename(&temp, &current).map_err(|_| {
        "Could not replace the running executable. Check write permissions and retry."
    })?;
    Ok(())
}

struct TemporaryFileCleanup(PathBuf);

impl Drop for TemporaryFileCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run_mcp() {
    #[cfg(windows)]
    blobui_mcp::serve_headless_bridge_stdio();

    #[cfg(not(windows))]
    blobui_mcp::server::serve_headless_stdio(
        || Ok(MCP_TOKEN.to_string()),
        |request, cancel, approval| crate::anyblob::dispatch_headless(request, cancel, approval),
    );
}

#[cfg(not(windows))]
pub(crate) fn ensure_anytype_serve(ip: &str, port: u16) -> Result<(), String> {
    let address = ip
        .parse::<std::net::IpAddr>()
        .map_err(|_| "The Anytype Headless API endpoint is invalid.".to_string())?;
    if !should_start_anytype_serve(ip) {
        return Ok(());
    }
    let endpoint = SocketAddr::new(address, port);
    if anytype_api_reachable(endpoint) {
        return Ok(());
    }

    let _guard = ANYTYPE_SERVE_LOCK
        .lock()
        .map_err(|_| "Could not check the Anytype Headless service state.".to_string())?;
    if anytype_api_reachable(endpoint) {
        return Ok(());
    }
    if ANYTYPE_SERVE_ATTEMPTED.load(Ordering::Acquire) {
        return Err(format!(
            "BlobUI started `anytype serve` in the background, but the Anytype API at {endpoint} is not ready yet. Check the Anytype CLI account and retry."
        ));
    }

    let mut spawned = false;
    let result = ensure_anytype_api_with(
        endpoint,
        ANYTYPE_API_START_TIMEOUT,
        |_| anytype_api_reachable(endpoint),
        || {
            Command::new("anytype")
                .arg("serve")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| {
                    format!(
                        "The Anytype API is unavailable at {endpoint}. BlobUI could not start `anytype serve`: {error}. Install and configure the Anytype CLI, then retry."
                    )
                })?;
            spawned = true;
            Ok(())
        },
    );
    if spawned {
        ANYTYPE_SERVE_ATTEMPTED.store(true, Ordering::Release);
    }
    result
}

#[cfg(any(test, not(windows)))]
fn should_start_anytype_serve(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>()
        .is_ok_and(|address| address.is_loopback())
}

#[cfg(any(test, not(windows)))]
fn ensure_anytype_api_with(
    endpoint: SocketAddr,
    timeout: Duration,
    mut reachable: impl FnMut(SocketAddr) -> bool,
    start: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if reachable(endpoint) {
        return Ok(());
    }
    start()?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if reachable(endpoint) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!(
        "BlobUI started `anytype serve` in the background, but the Anytype API at {endpoint} did not become available. Check the Anytype CLI account and retry."
    ))
}

#[cfg(not(windows))]
fn anytype_api_reachable(endpoint: SocketAddr) -> bool {
    TcpStream::connect_timeout(&endpoint, Duration::from_millis(200)).is_ok()
}

fn export_archive(destination: &Path) -> Result<(), String> {
    ensure_zip_extension(destination)?;
    let root = ensure_settings()?;
    let settings = root.join("Settings");
    crate::reject_reparse_component(&settings)?;
    let mut files = Vec::new();
    let mut total = 0usize;
    collect_archive_files(&settings, &settings, &mut files, &mut total)?;
    if files.iter().all(|(name, _)| name != "blob.meta.json") {
        return Err("Headless Settings metadata is missing. Run a BlobUI Headless command and retry export.".into());
    }
    for (name, bytes) in &mut files {
        if name == "blob.meta.json" {
            let mut metadata: Value = serde_json::from_slice(bytes)
                .map_err(|_| "Headless Settings metadata is invalid.")?;
            mark_headless_metadata(&mut metadata)?;
            *bytes = serde_json::to_vec_pretty(&metadata)
                .map_err(|_| "Could not encode Settings metadata.")?;
        }
    }
    let bytes = make_zip(&files)?;
    crate::storage::write_atomic_private(destination, &bytes)?;
    println!(
        "Exported BlobUI Headless settings to {}.",
        destination.display()
    );
    Ok(())
}

fn collect_archive_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
    total: &mut usize,
) -> Result<(), String> {
    for entry in
        fs::read_dir(directory).map_err(|_| format!("Could not read {}.", directory.display()))?
    {
        let path = entry
            .map_err(|_| "Could not read a Settings entry.")?
            .path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| "Could not inspect a Settings entry.")?;
        if metadata.file_type().is_symlink() || crate::is_windows_reparse_point(&metadata) {
            return Err(format!(
                "Settings archive refused a link at {}.",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_archive_files(root, &path, files, total)?;
            continue;
        }
        if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_FILE_BYTES as u64 {
            return Err(format!(
                "Settings archive refused an unsupported file at {}.",
                path.display()
            ));
        }
        let name = path
            .strip_prefix(root)
            .map_err(|_| "A Settings file escaped the export folder.")?
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if !archive_path_is_safe(&name, false) {
            return Err("Settings contains an unsupported archive path.".into());
        }
        archive_size_sum(*total, metadata.len() as usize)
            .map_err(|_| "Settings contains more data than the supported archive limit.")?;
        let mut contents = Vec::with_capacity(metadata.len() as usize);
        fs::File::open(&path)
            .map_err(|_| format!("Could not read {}.", path.display()))?
            .take(MAX_ARCHIVE_FILE_BYTES as u64 + 1)
            .read_to_end(&mut contents)
            .map_err(|_| format!("Could not read {}.", path.display()))?;
        if contents.len() > MAX_ARCHIVE_FILE_BYTES {
            return Err(format!(
                "Settings archive refused an oversized file at {}.",
                path.display()
            ));
        }
        *total = archive_size_sum(*total, contents.len())
            .map_err(|_| "Settings contains more data than the supported archive limit.")?;
        files.push((name, contents));
        if files.len() > MAX_ARCHIVE_ENTRIES {
            return Err("Settings contains too many files to export.".into());
        }
    }
    Ok(())
}

fn import_archive(source: &Path) -> Result<(), String> {
    ensure_zip_extension(source)?;
    crate::reject_reparse_component(source)?;
    let metadata = fs::symlink_metadata(source)
        .map_err(|_| format!("Could not inspect {}.", source.display()))?;
    if metadata.file_type().is_symlink()
        || crate::is_windows_reparse_point(&metadata)
        || !metadata.is_file()
    {
        return Err("The Settings archive must be a regular file, not a link or directory.".into());
    }
    if metadata.len() > MAX_ARCHIVE_BYTES as u64 {
        return Err("The Settings archive is larger than the supported limit.".into());
    }
    let mut archive = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(source)
        .map_err(|_| format!("Could not read {}.", source.display()))?
        .take(MAX_ARCHIVE_BYTES as u64 + 1)
        .read_to_end(&mut archive)
        .map_err(|_| format!("Could not read {}.", source.display()))?;
    if archive.len() > MAX_ARCHIVE_BYTES {
        return Err("The Settings archive is larger than the supported limit.".into());
    }
    let entries = read_zip(&archive)?;
    let metadata_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.name.rsplit('/').next() == Some("blob.meta.json"))
        .collect();
    let metadata_entry = match metadata_entries.as_slice() {
        [entry] => *entry,
        [] => return Err("The selected ZIP does not contain BlobUI Settings metadata.".into()),
        _ => {
            return Err(
                "The selected ZIP contains more than one BlobUI Settings metadata file.".into(),
            )
        }
    };
    let prefix = metadata_entry
        .name
        .strip_suffix("blob.meta.json")
        .unwrap_or_default();
    let metadata: Value = serde_json::from_slice(&metadata_entry.bytes)
        .map_err(|_| "The BlobUI Settings metadata is invalid.")?;
    let metadata = metadata
        .as_object()
        .ok_or("The BlobUI Settings metadata must contain a JSON object.")?;
    let cli_archive = metadata.get("headlessCli").and_then(Value::as_bool) == Some(true);
    let root = ensure_settings()?;
    let stage_root = root.join(format!(".import-stage-{}", std::process::id()));
    if stage_root.exists() {
        return Err(
            "A previous Settings import is still staged. Remove its staging folder and retry."
                .into(),
        );
    }
    fs::create_dir(&stage_root)
        .map_err(|_| "Could not create a private Settings import folder.")?;
    let result = (|| {
        let staged_settings = stage_root.join("Settings");
        fs::create_dir(&staged_settings).map_err(|_| "Could not create staged Settings.")?;
        let allowed = [
            "blob.meta.json",
            "data.json",
            "basic-settings.json",
            "advanced-settings.json",
            "calendar-events.json",
            "kanban-calc.json",
            "keybinds.json",
            "anyblob/settings.json",
            "anyblob/permissions.json",
            "DO-NOT-SHARE/connection.json",
            "DO-NOT-SHARE/anytype-api-key.dpapi",
        ];
        for entry in &entries {
            let Some(relative) = entry.name.strip_prefix(prefix) else {
                continue;
            };
            if !archive_path_is_safe(relative, false) || !allowed.contains(&relative) {
                continue;
            }
            let credential = relative == "DO-NOT-SHARE/anytype-api-key.dpapi";
            if credential && !cli_archive {
                continue;
            }
            let destination = staged_settings.join(relative);
            let parent = destination
                .parent()
                .ok_or("Invalid imported Settings path.")?;
            fs::create_dir_all(parent)
                .map_err(|_| "Could not prepare imported Settings folders.")?;
            let mut file_options = fs::OpenOptions::new();
            file_options.write(true).create_new(true);
            #[cfg(unix)]
            if credential {
                use std::os::unix::fs::OpenOptionsExt;
                file_options.mode(0o600);
            }
            let mut file = file_options
                .open(&destination)
                .map_err(|_| "Could not write an imported Settings file.")?;
            file.write_all(&entry.bytes)
                .map_err(|_| "Could not write an imported Settings file.")?;
            file.sync_all()
                .map_err(|_| "Could not flush an imported Settings file.")?;
        }
        if !staged_settings.join("data.json").is_file()
            || !staged_settings.join("blob.meta.json").is_file()
        {
            return Err("The selected ZIP is not a complete BlobUI Settings archive.".into());
        }
        if cli_archive {
            let credential = staged_settings.join("DO-NOT-SHARE/anytype-api-key.dpapi");
            if credential.is_file() {
                let text = fs::read_to_string(&credential)
                    .map_err(|_| "The imported API credential is invalid.")?;
                #[cfg(windows)]
                crate::connection::decrypt_saved_key(text.trim())?;
                #[cfg(unix)]
                crate::validate_api_key(text.trim().to_string())?;
            }
        }
        let data = crate::settings_store::load(&stage_root)?;
        let configuration = crate::settings_from_data(&data).codex_integration;
        let has_credential = cli_archive
            && staged_settings
                .join("DO-NOT-SHARE/anytype-api-key.dpapi")
                .is_file();
        let api_connection = if cli_archive {
            imported_api_connection(&data, has_credential)?
        } else {
            json!({"enabled":false,"ip":"127.0.0.1","port":ANYTYPE_API_PORT})
        };
        let filtered =
            json!({"settings":{"codexIntegration":configuration},"apiConnection":api_connection});
        crate::settings_store::save(&stage_root, &filtered)?;
        let metadata_path = staged_settings.join("blob.meta.json");
        let mut next_metadata: Value = serde_json::from_slice(
            &fs::read(&metadata_path).map_err(|_| "Could not read staged Settings metadata.")?,
        )
        .map_err(|_| "The staged Settings metadata is invalid.")?;
        mark_headless_metadata(&mut next_metadata)?;
        fs::write(
            &metadata_path,
            serde_json::to_vec_pretty(&next_metadata)
                .map_err(|_| "Could not encode staged Settings metadata.")?,
        )
        .map_err(|_| "Could not save staged Settings metadata.")?;
        commit_imported_settings(&root, &stage_root)?;
        Ok::<_, String>(())
    })();
    let _ = fs::remove_dir_all(&stage_root);
    result?;
    println!(
        "Imported {} settings from {}.",
        if cli_archive {
            "BlobUI Headless"
        } else {
            "BlobUI Settings"
        },
        source.display()
    );
    if !cli_archive {
        println!("The archive came from BlobUI Settings. Its API credentials were not imported.");
    }
    Ok(())
}

fn commit_imported_settings(root: &Path, stage_root: &Path) -> Result<(), String> {
    let destination = root.join("Settings");
    let staged = stage_root.join("Settings");
    let backup = root.join(format!(".Settings-import-rollback-{}", std::process::id()));
    crate::reject_reparse_component(root)?;
    crate::reject_reparse_component(&destination)?;
    if backup.exists() {
        return Err("A previous Settings import rollback folder is still present.".into());
    }
    fs::rename(&destination, &backup)
        .map_err(|_| "Could not prepare to replace the current Settings folder.")?;
    if let Err(error) = fs::rename(&staged, &destination) {
        let _ = fs::rename(&backup, &destination);
        return Err(format!("Could not install imported Settings: {error}"));
    }
    let _ = fs::remove_dir_all(&backup);
    Ok(())
}

fn ensure_zip_extension(path: &Path) -> Result<(), String> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("zip"))
    {
        return Err("Settings import and export paths must use the .zip extension.".into());
    }
    Ok(())
}

fn mark_headless_metadata(metadata: &mut Value) -> Result<(), String> {
    let metadata = metadata
        .as_object_mut()
        .ok_or("The BlobUI Settings metadata must contain a JSON object.")?;
    metadata.insert("headlessCli".into(), json!(true));
    Ok(())
}

fn imported_api_connection(data: &Value, has_credential: bool) -> Result<Value, String> {
    let connection = data.get("apiConnection").and_then(Value::as_object);
    if data.get("apiConnection").is_some() && connection.is_none() {
        return Err("The imported Anytype API connection settings are malformed.".into());
    }
    let connection = connection.cloned().unwrap_or_default();
    if let Some(ip) = connection.get("ip") {
        let ip = ip
            .as_str()
            .ok_or("The imported Anytype API address is malformed.")?;
        if crate::normalize_api_ip(Some(ip))? != "127.0.0.1" {
            return Err("The imported Anytype API address must be 127.0.0.1.".into());
        }
    }
    if let Some(port) = connection.get("port") {
        if port.as_u64() != Some(ANYTYPE_API_PORT as u64) {
            return Err("The imported Anytype API port must be 31012.".into());
        }
    }
    let enabled = match connection.get("enabled") {
        Some(Value::Bool(enabled)) => has_credential && *enabled,
        None => has_credential,
        Some(_) => return Err("The imported Anytype API connection setting is malformed.".into()),
    };
    let mut imported = serde_json::Map::new();
    imported.insert("enabled".into(), json!(enabled));
    imported.insert("ip".into(), json!("127.0.0.1"));
    imported.insert("port".into(), json!(ANYTYPE_API_PORT));
    if let Some(revision) = connection.get("metadataRevision").and_then(Value::as_str) {
        imported.insert("metadataRevision".into(), json!(revision));
    }
    Ok(Value::Object(imported))
}

fn archive_path_is_safe(value: &str, allow_directory: bool) -> bool {
    if value.is_empty() || value.starts_with('/') || value.contains('\\') || value.contains(':') {
        return false;
    }
    let value = if allow_directory {
        value.trim_end_matches('/')
    } else {
        value
    };
    !value.is_empty()
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

struct ZipEntry {
    name: String,
    bytes: Vec<u8>,
}

fn make_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    if files.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Settings archive contains too many files.".into());
    }
    let mut archive = Vec::new();
    let mut central = Vec::new();
    let mut total_uncompressed = 0usize;
    for (name, contents) in files {
        if !archive_path_is_safe(name, false) || contents.len() > MAX_ARCHIVE_FILE_BYTES {
            return Err("Settings archive contains an unsupported file.".into());
        }
        total_uncompressed = archive_size_sum(total_uncompressed, contents.len())?;
        let name_bytes = name.as_bytes();
        let compressed = {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(contents)
                .map_err(|_| "Could not compress a Settings file.")?;
            encoder
                .finish()
                .map_err(|_| "Could not finish compressing a Settings file.")?
        };
        let crc = crc32fast::hash(contents);
        let offset = u32::try_from(archive.len())
            .map_err(|_| "Settings archive exceeds ZIP size limits.")?;
        push_u32(&mut archive, 0x04034b50);
        push_u16(&mut archive, 20);
        push_u16(&mut archive, 0x0800);
        push_u16(&mut archive, 8);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u32(&mut archive, crc);
        push_u32(
            &mut archive,
            u32::try_from(compressed.len())
                .map_err(|_| "Settings archive exceeds ZIP size limits.")?,
        );
        push_u32(
            &mut archive,
            u32::try_from(contents.len())
                .map_err(|_| "Settings archive exceeds ZIP size limits.")?,
        );
        push_u16(
            &mut archive,
            u16::try_from(name_bytes.len()).map_err(|_| "Settings archive path is too long.")?,
        );
        push_u16(&mut archive, 0);
        archive.extend_from_slice(name_bytes);
        archive.extend_from_slice(&compressed);

        let mut item = Vec::new();
        push_u32(&mut item, 0x02014b50);
        push_u16(&mut item, 20);
        push_u16(&mut item, 20);
        push_u16(&mut item, 0x0800);
        push_u16(&mut item, 8);
        push_u16(&mut item, 0);
        push_u16(&mut item, 0);
        push_u32(&mut item, crc);
        push_u32(&mut item, compressed.len() as u32);
        push_u32(&mut item, contents.len() as u32);
        push_u16(&mut item, name_bytes.len() as u16);
        push_u16(&mut item, 0);
        push_u16(&mut item, 0);
        push_u16(&mut item, 0);
        push_u16(&mut item, 0);
        push_u32(&mut item, 0);
        push_u32(&mut item, offset);
        item.extend_from_slice(name_bytes);
        central.extend_from_slice(&item);
        if archive.len().saturating_add(central.len()) > MAX_ARCHIVE_BYTES {
            return Err("Settings archive is larger than the supported limit.".into());
        }
    }
    let central_offset =
        u32::try_from(archive.len()).map_err(|_| "Settings archive exceeds ZIP size limits.")?;
    archive.extend_from_slice(&central);
    let count =
        u16::try_from(files.len()).map_err(|_| "Settings archive contains too many files.")?;
    let central_size =
        u32::try_from(central.len()).map_err(|_| "Settings archive exceeds ZIP size limits.")?;
    push_u32(&mut archive, 0x06054b50);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, count);
    push_u16(&mut archive, count);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);
    if archive.len() > MAX_ARCHIVE_BYTES {
        return Err("Settings archive is larger than the supported limit.".into());
    }
    Ok(archive)
}

fn read_zip(bytes: &[u8]) -> Result<Vec<ZipEntry>, String> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err("The Settings archive is larger than the supported limit.".into());
    }
    let search_start = bytes.len().saturating_sub(65_557);
    let eocd = bytes[search_start..]
        .windows(4)
        .rposition(|window| window == b"PK\x05\x06")
        .map(|offset| search_start + offset)
        .ok_or("The selected ZIP archive is incomplete or invalid.")?;
    let disk = read_u16(bytes, eocd + 4)?;
    let central_disk = read_u16(bytes, eocd + 6)?;
    let disk_count = read_u16(bytes, eocd + 8)?;
    let count = read_u16(bytes, eocd + 10)? as usize;
    let central_size = read_u32(bytes, eocd + 12)? as usize;
    let central_offset = read_u32(bytes, eocd + 16)? as usize;
    let comment_len = read_u16(bytes, eocd + 20)? as usize;
    if disk != 0
        || central_disk != 0
        || disk_count as usize != count
        || count > MAX_ARCHIVE_ENTRIES
        || central_offset
            .checked_add(central_size)
            .is_none_or(|end| end > eocd)
        || eocd
            .checked_add(22 + comment_len)
            .is_none_or(|end| end > bytes.len())
    {
        return Err("Multi-disk, ZIP64, or malformed Settings archives are not supported.".into());
    }
    let mut entries = Vec::with_capacity(count);
    let mut cursor = central_offset;
    let mut names = BTreeSet::new();
    let mut total_uncompressed = 0usize;
    for _ in 0..count {
        if read_u32(bytes, cursor)? != 0x02014b50 {
            return Err("The Settings ZIP central directory is invalid.".into());
        }
        let made_by = read_u16(bytes, cursor + 4)?;
        let flags = read_u16(bytes, cursor + 8)?;
        let method = read_u16(bytes, cursor + 10)?;
        let crc = read_u32(bytes, cursor + 16)?;
        let compressed_size = read_u32(bytes, cursor + 20)? as usize;
        let uncompressed_size = read_u32(bytes, cursor + 24)? as usize;
        let name_len = read_u16(bytes, cursor + 28)? as usize;
        let extra_len = read_u16(bytes, cursor + 30)? as usize;
        let comment_len = read_u16(bytes, cursor + 32)? as usize;
        let disk_start = read_u16(bytes, cursor + 34)?;
        let external = read_u32(bytes, cursor + 38)?;
        let local_offset = read_u32(bytes, cursor + 42)? as usize;
        let name_start = cursor + 46;
        let name_end = name_start
            .checked_add(name_len)
            .ok_or("The ZIP entry path is invalid.")?;
        let extra_end = name_end
            .checked_add(extra_len)
            .ok_or("The ZIP entry metadata is invalid.")?;
        let end = extra_end
            .checked_add(comment_len)
            .ok_or("The ZIP entry metadata is invalid.")?;
        if end > central_offset + central_size {
            return Err("The ZIP central directory entry is truncated.".into());
        }
        let raw_name = bytes
            .get(name_start..name_end)
            .ok_or("The ZIP entry path is truncated.")?;
        let name = if flags & (1 << 11) != 0 {
            std::str::from_utf8(raw_name).map_err(|_| "The ZIP entry path is not UTF-8.")?
        } else if raw_name.is_ascii() {
            std::str::from_utf8(raw_name).unwrap()
        } else {
            return Err("Non-UTF-8 ZIP entry paths are not supported.".into());
        };
        let directory = name.ends_with('/');
        if !archive_path_is_safe(name, directory) || !names.insert(name.to_string()) {
            return Err("The ZIP contains an unsafe or duplicate path.".into());
        }
        let unix_mode = external >> 16;
        let unix_platform = made_by >> 8 == 3;
        if (flags & !0x080e) != 0
            || (flags & 1) != 0
            || disk_start != 0
            || uncompressed_size > MAX_ARCHIVE_FILE_BYTES
            || (unix_platform && unix_mode & 0o170000 == 0o120000)
        {
            return Err("Encrypted, linked, or oversized ZIP entries are not supported.".into());
        }
        if !directory {
            if !matches!(method, 0 | 8) {
                return Err("The ZIP uses an unsupported compression method.".into());
            }
            total_uncompressed = archive_size_sum(total_uncompressed, uncompressed_size)?;
            if local_offset >= central_offset {
                return Err("The ZIP entry header overlaps its central directory.".into());
            }
            if read_u32(bytes, local_offset)? != 0x04034b50 {
                return Err("The ZIP entry header is invalid.".into());
            }
            let local_flags = read_u16(bytes, local_offset + 6)?;
            let local_method = read_u16(bytes, local_offset + 8)?;
            let local_crc = read_u32(bytes, local_offset + 14)?;
            let local_compressed_size = read_u32(bytes, local_offset + 18)? as usize;
            let local_uncompressed_size = read_u32(bytes, local_offset + 22)? as usize;
            let local_name_len = read_u16(bytes, local_offset + 26)? as usize;
            let local_extra_len = read_u16(bytes, local_offset + 28)? as usize;
            let data_start = local_offset
                .checked_add(30 + local_name_len + local_extra_len)
                .ok_or("The ZIP entry data is invalid.")?;
            let data_end = data_start
                .checked_add(compressed_size)
                .ok_or("The ZIP entry data is invalid.")?;
            if data_end > central_offset
                || local_flags != flags
                || local_method != method
                || bytes.get(local_offset + 30..local_offset + 30 + local_name_len)
                    != Some(raw_name)
                || (flags & 0x0008 == 0
                    && (local_crc != crc
                        || local_compressed_size != compressed_size
                        || local_uncompressed_size != uncompressed_size))
            {
                return Err("The ZIP entry data is truncated or its path changed.".into());
            }
            let compressed = &bytes[data_start..data_end];
            let content = if method == 0 {
                compressed.to_vec()
            } else {
                let decoder = DeflateDecoder::new(compressed);
                let mut output = Vec::with_capacity(uncompressed_size.min(MAX_ARCHIVE_FILE_BYTES));
                decoder
                    .take(uncompressed_size as u64 + 1)
                    .read_to_end(&mut output)
                    .map_err(|_| "Could not decompress a Settings ZIP entry.")?;
                output
            };
            if content.len() != uncompressed_size || crc32fast::hash(&content) != crc {
                return Err("A Settings ZIP entry failed its size or CRC check.".into());
            }
            entries.push(ZipEntry {
                name: name.to_string(),
                bytes: content,
            });
        }
        cursor = end;
    }
    if cursor != central_offset + central_size {
        return Err("The ZIP central directory size is inconsistent.".into());
    }
    Ok(entries)
}

fn archive_size_sum(current: usize, next: usize) -> Result<usize, String> {
    current
        .checked_add(next)
        .filter(|total| *total <= MAX_ARCHIVE_BYTES)
        .ok_or_else(|| "Settings archive expands beyond the supported limit.".into())
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or("The ZIP archive is truncated.")?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("The ZIP archive is truncated.")?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_mcp_starts_anytype_when_the_api_is_unavailable() {
        use std::cell::Cell;

        let endpoint = "127.0.0.1:31012".parse().unwrap();
        let reachable = Cell::new(false);
        let starts = Cell::new(0);
        ensure_anytype_api_with(
            endpoint,
            Duration::from_secs(1),
            |_| reachable.get(),
            || {
                starts.set(starts.get() + 1);
                reachable.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(starts.get(), 1);
    }

    #[test]
    fn headless_mcp_skips_anytype_start_when_the_api_is_already_available() {
        let endpoint = "127.0.0.1:31012".parse().unwrap();
        ensure_anytype_api_with(
            endpoint,
            Duration::from_secs(1),
            |_| true,
            || Err("Anytype should not start when the API is available.".into()),
        )
        .unwrap();
    }

    #[test]
    fn remote_anytype_endpoints_do_not_start_anytype_serve() {
        assert!(should_start_anytype_serve("127.0.0.1"));
        assert!(should_start_anytype_serve("::1"));
        assert!(!should_start_anytype_serve("192.168.1.20"));
    }

    #[test]
    fn api_endpoint_options_accept_ipv4_ipv6_and_validate_ports() {
        let ipv4_args = [
            "--host-ip".into(),
            "192.168.1.20".into(),
            "--host-port".into(),
            "31013".into(),
        ];
        assert_eq!(
            parse_api_endpoint_options(&ipv4_args, "127.0.0.1", ANYTYPE_API_PORT).unwrap(),
            ("192.168.1.20".into(), 31013),
        );
        assert_eq!(
            parse_api_endpoint_options(
                &["--host-ip".into(), "::1".into()],
                "127.0.0.1",
                ANYTYPE_API_PORT,
            ).unwrap(),
            ("::1".into(), ANYTYPE_API_PORT),
        );
        assert_eq!(
            parse_api_endpoint_options(
                &["--host-port".into(), "31013".into()],
                "127.0.0.1",
                ANYTYPE_API_PORT,
            ).unwrap(),
            ("127.0.0.1".into(), 31013),
        );
        for args in [
            vec!["--host-ip".into()],
            vec!["--host-ip".into(), "api.example.com".into()],
            vec!["--host-port".into(), "0".into()],
            vec!["--host-port".into(), "65536".into()],
            vec!["--host-port".into(), "31012".into(), "--host-port".into(), "31013".into()],
        ] {
            assert!(
                parse_api_endpoint_options(&args, "127.0.0.1", ANYTYPE_API_PORT).is_err(),
                "{args:?}"
            );
        }
    }

    #[test]
    fn api_endpoint_updates_preserve_other_connection_settings() {
        let data = json!({
            "apiConnection": {
                "enabled": true,
                "metadataRevision": "rev-7",
                "futureSetting": 42
            }
        });
        assert_eq!(
            api_connection(&data, true, "192.168.1.20", 31012),
            json!({
                "enabled": true,
                "ip": "192.168.1.20",
                "port": 31012,
                "metadataRevision": "rev-7",
                "futureSetting": 42
            }),
        );
    }

    #[test]
    fn headless_mcp_reports_when_anytype_cannot_be_started() {
        let endpoint = "127.0.0.1:31012".parse().unwrap();
        let error = ensure_anytype_api_with(
            endpoint,
            Duration::from_secs(1),
            |_| false,
            || Err("Anytype CLI is missing.".into()),
        )
        .unwrap_err();
        assert_eq!(error, "Anytype CLI is missing.");
    }

    #[test]
    fn headless_release_channels_parse_tags_and_compare_labels() {
        use crate::updater::ReleaseChannel;
        assert_eq!(headless_tag_version("headless-v1.0.0", ReleaseChannel::Release), Some(("1.0.0", "1.0.0".into())));
        assert_eq!(headless_tag_version("headless-dev-1.0.1", ReleaseChannel::Dev), Some(("1.0.1", "dev-1.0.1".into())));
        assert_eq!(headless_tag_version("headless-dev-1.0.1", ReleaseChannel::Release), None);
        assert!(headless_release_is_newer("dev-1.0.1", "1.0.0"));
        assert!(headless_release_is_newer("dev-1.0.0", "1.0.0"));
        assert!(headless_release_is_newer("1.0.0", "dev-1.0.0"));
        assert!(!headless_release_is_newer("dev-1.0.1", "dev-1.0.1"));
    }

    #[test]
    fn build_channel_command_requires_main_or_dev() {
        assert!(run(vec!["--build-channel".into()]).unwrap_err().contains("Usage"));
        assert!(run(vec!["--build-channel".into(), "beta".into()]).unwrap_err().contains("main"));
    }

    #[test]
    fn spaces_listing_preserves_full_ids_and_names() {
        let rows = space_list_rows(vec![
            json!({"id":"space-full-id-1234567890","name":"Work"}),
            json!({"id":"personal-space-id","name":"Personal"}),
        ])
        .unwrap();
        assert_eq!(
            rows,
            vec![
                json!({"id":"space-full-id-1234567890","name":"Work"}),
                json!({"id":"personal-space-id","name":"Personal"}),
            ]
        );
        assert!(space_list_rows(vec![json!({"name":"Missing ID"})]).is_err());
    }

    #[test]
    fn removed_project_and_search_commands_are_unknown() {
        assert!(run(vec!["project".into()])
            .unwrap_err()
            .contains("Unknown command"));
        assert!(run(vec!["search".into(), "term".into()])
            .unwrap_err()
            .contains("Unknown command"));
        assert!(run(vec!["spaces".into()])
            .unwrap_err()
            .contains("Usage: blob-ui spaces ls"));
    }
    #[test]
    fn object_mention_spaces_use_the_documented_command_order() {
        assert_eq!(
            parse_mention_space_args(&["ls".into()]).unwrap(),
            ("ls", None)
        );
        assert_eq!(
            parse_mention_space_args(&["add".into(), "space-1".into()]).unwrap(),
            ("add", Some("space-1"))
        );
        assert_eq!(
            parse_mention_space_args(&["remove".into(), "space-1".into()]).unwrap(),
            ("remove", Some("space-1"))
        );
        for args in [
            vec![],
            vec!["spaces".into(), "ls".into()],
            vec!["add".into()],
            vec!["ls".into(), "space-1".into()],
        ] {
            assert!(parse_mention_space_args(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn explicit_mention_space_filters_still_list_every_space() {
        let spaces = vec![
            json!({"id":"space-1","name":"One"}),
            json!({"id":"space-2","name":"Two"}),
        ];
        let empty = crate::anyblob_mentions::Selection {
            all: false,
            items: Vec::new(),
        };
        let listed = mark_space_selection(spaces.clone(), &empty).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0]["selected"], false);
        assert_eq!(listed[1]["selected"], false);

        let selected = crate::anyblob_mentions::Selection {
            all: false,
            items: vec![crate::anyblob_mentions::Space {
                id: "space-2".into(),
                name: "Two".into(),
            }],
        };
        let listed = mark_space_selection(spaces, &selected).unwrap();
        assert_eq!(listed[0]["selected"], false);
        assert_eq!(listed[1]["selected"], true);
    }

    #[test]
    fn settings_archive_round_trips_and_rejects_corruption() {
        let files = vec![
            (
                "blob.meta.json".to_string(),
                br#"{"formatVersion":1,"headlessCli":true}"#.to_vec(),
            ),
            (
                "anyblob/settings.json".to_string(),
                br#"{"enabled":true}"#.to_vec(),
            ),
        ];
        let archive = make_zip(&files).unwrap();
        let extracted = read_zip(&archive).unwrap();
        assert_eq!(extracted.len(), 2);
        assert_eq!(extracted[0].name, "blob.meta.json");
        assert_eq!(extracted[0].bytes, files[0].1);
        assert_eq!(extracted[1].name, "anyblob/settings.json");
        assert_eq!(extracted[1].bytes, files[1].1);

        let mut corrupted = archive;
        let middle = corrupted.len() / 2;
        corrupted[middle] ^= 0x40;
        assert!(read_zip(&corrupted).is_err());
        let mut mismatched_header = make_zip(&files).unwrap();
        mismatched_header[8] = 0;
        assert!(read_zip(&mismatched_header).is_err());
        for path in [
            "../Settings/data.json",
            "Settings\\data.json",
            "C:/Settings/data.json",
        ] {
            assert!(!archive_path_is_safe(path, false), "{path}");
        }
    }

    #[test]
    fn archive_size_limit_is_checked_before_decoding() {
        assert_eq!(
            archive_size_sum(MAX_ARCHIVE_BYTES - 4, 4).unwrap(),
            MAX_ARCHIVE_BYTES
        );
        assert!(archive_size_sum(MAX_ARCHIVE_BYTES - 4, 5).is_err());
        assert!(archive_size_sum(usize::MAX, 1).is_err());
    }

    #[test]
    fn imported_api_connection_is_local_and_requires_a_credential() {
        let connection = imported_api_connection(
            &json!({"apiConnection":{"enabled":true,"ip":"127.0.0.1","port":31012}}),
            true,
        )
        .unwrap();
        assert_eq!(
            connection,
            json!({"enabled":true,"ip":"127.0.0.1","port":31012})
        );
        assert_eq!(
            imported_api_connection(
                &json!({"apiConnection":{"enabled":true,"ip":"127.0.0.1","port":31012}}),
                false
            )
            .unwrap()["enabled"],
            false,
        );
        for connection in [
            json!({"apiConnection":{"enabled":true,"ip":"192.168.1.4","port":31012}}),
            json!({"apiConnection":{"enabled":true,"ip":"127.0.0.1","port":31009}}),
        ] {
            assert!(imported_api_connection(&connection, true).is_err());
        }
    }

    #[test]
    fn headless_metadata_requires_an_object() {
        let mut metadata = json!({"formatVersion":1});
        mark_headless_metadata(&mut metadata).unwrap();
        assert_eq!(metadata["headlessCli"], true);
        assert!(mark_headless_metadata(&mut json!("invalid")).is_err());
    }
}
