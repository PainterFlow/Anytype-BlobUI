#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

mod connection;
mod hotkey;

static SAVE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct SaveGuard;
impl Drop for SaveGuard {
    fn drop(&mut self) {
        SAVE_IN_PROGRESS.store(false, Ordering::Release);
    }
}

const DEFAULT_FONT: &str = "Libron";
const DEFAULT_CODE_FONT: &str = "Consolas";
const DEFAULT_ACCENT: &str = "#DE50DE";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SettingsDraft {
    font_family: String,
    code_font_family: String,
    accent_color: String,
    accent_mode: String,
    accent_preset: String,
    reduced_motion: bool,
    tab_hover_effect: bool,
    link_hover_effect: bool,
    object_mention_hover_effect: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    ready: bool,
    install_directory: String,
    app_data_directory: String,
    theme: String,
    settings: SettingsDraft,
    message: Option<String>,
    hotkey_error: Option<String>,
    onboarding_complete: bool,
    api_port: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveResult {
    calendar_status: String,
}

#[tauri::command]
fn get_app_state(app: tauri::AppHandle) -> Result<AppState, String> {
    let state = load_app_state()?;
    apply_native_theme(&app, &state.theme);
    Ok(state)
}

#[tauri::command]
async fn save_settings(settings: SettingsDraft) -> Result<SaveResult, String> {
    if SAVE_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Settings are already being saved.".into());
    }
    let guard = SaveGuard;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        save_settings_blocking(settings)
    })
    .await
    .map_err(|error| format!("The save task stopped unexpectedly: {error}"))?
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionDraft {
    enabled: bool,
    api_key: String,
    port: Option<u16>,
}

#[tauri::command]
async fn configure_connection(connection: ConnectionDraft) -> Result<SaveResult, String> {
    if SAVE_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Wait for the current save to finish.".into());
    }
    let guard = SaveGuard;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        configure_connection_blocking(connection)
    })
    .await
    .map_err(|_| "The connection task stopped unexpectedly.".to_string())?
}

fn configure_connection_blocking(mut connection: ConnectionDraft) -> Result<SaveResult, String> {
    if connection.port == Some(0) {
        return Err("Port must be between 1 and 65535.".into());
    }
    connection.api_key = connection.api_key.trim().to_string();
    if connection.enabled
        && (connection.api_key.is_empty()
            || connection.api_key.len() > 4096
            || connection.api_key.chars().any(char::is_control))
    {
        return Err("Enter your Anytype API v2 key.".into());
    }
    if !connection.enabled {
        connection.api_key.clear();
    }
    let install = resolve_install_directory(&app_data_directory())?;
    ensure_not_release_path(&install)?;
    reject_reparse_component(&install)?;
    let script = resolve_source_directory()?
        .join("scripts")
        .join("Rebuild.ps1");
    let payload = serde_json::to_vec(&connection)
        .map_err(|_| "Could not encode the connection request.".to_string())?;
    for shell in ["pwsh.exe", "powershell.exe"] {
        let mut command = Command::new(shell);
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&script)
            .arg("-InstallDirectory")
            .arg(&install)
            .arg("-ConfigureConnectionFromStdin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("Could not start the connection setup.".into()),
        };
        let write_result = child
            .stdin
            .take()
            .ok_or_else(|| "Connection input is unavailable.".to_string())
            .and_then(|mut input| {
                input
                    .write_all(&payload)
                    .map_err(|_| "Could not send the connection request.".to_string())
            });
        let output = child
            .wait_with_output()
            .map_err(|_| "Could not finish connection setup.".to_string())?;
        write_result?;
        if !output.status.success() {
            return Err(connection::connection_error(
                &output.stdout,
                connection.port.unwrap_or(31009),
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let calendar_status = if text.contains("BlobUI_CALENDAR_CACHE=disabled") {
            "disabled"
        } else if text.contains("BlobUI_CALENDAR_CACHE=refreshed") {
            "refreshed"
        } else {
            "cached"
        };
        return Ok(SaveResult {
            calendar_status: calendar_status.into(),
        });
    }
    Err("PowerShell could not be found.".into())
}

fn show_settings_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn minimize_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn start_window_drag(window: tauri::WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())
}

#[tauri::command]
fn open_project_repository() -> Result<(), String> {
    open_github_link("https://github.com/PainterFlow/Anytype-BlobUI")
}

#[tauri::command]
fn open_author_profile() -> Result<(), String> {
    open_github_link("https://github.com/PainterFlow")
}

fn open_github_link(link: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let operation: Vec<u16> = "open\0".encode_utf16().collect();
        let url: Vec<u16> = format!("{link}\0").encode_utf16().collect();
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        } as usize;
        if result <= 32 {
            return Err(format!(
                "Could not open GitHub in your browser, Windows error {result}."
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    Err("Opening the project link is supported on Windows.".into())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    if SAVE_IN_PROGRESS.load(Ordering::Acquire) {
        return Err("Wait for the settings save to finish before quitting.".into());
    }
    app.exit(0);
    Ok(())
}

fn load_app_state() -> Result<AppState, String> {
    let app_data = app_data_directory();
    let install_directory = resolve_install_directory(&app_data)?;
    let data_path = install_directory.join("blob").join("data.json");
    let theme = read_anytype_theme(&app_data.join("devconfig.json"));

    let mut onboarding_complete = false;
    let mut api_port = 31009;
    let (ready, settings, message) = if data_path.is_file() {
        let data = read_json_file(&data_path)?;
        onboarding_complete = data
            .pointer("/apiConnection/enabled")
            .and_then(Value::as_bool)
            .is_some();
        api_port = data
            .pointer("/apiConnection/port")
            .and_then(Value::as_u64)
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port > 0)
            .unwrap_or(31009);
        let settings = settings_from_data(&data);
        (true, settings, None)
    } else {
        (
            false,
            SettingsDraft::default(),
            Some(format!(
                "BlobUI settings were not found at {}. Select your Anytype folder with scripts/Settings.ps1 -InstallDirectory.",
                data_path.display()
            )),
        )
    };

    Ok(AppState {
        ready,
        install_directory: install_directory.to_string_lossy().into_owned(),
        app_data_directory: app_data.to_string_lossy().into_owned(),
        theme,
        settings,
        message,
        hotkey_error: hotkey::error(),
        onboarding_complete,
        api_port,
    })
}

fn save_settings_blocking(settings: SettingsDraft) -> Result<SaveResult, String> {
    let settings = validate_settings(settings)?;
    let app_data = app_data_directory();
    let install_directory = resolve_install_directory(&app_data)?;
    ensure_not_release_path(&install_directory)?;

    let blob_directory = install_directory.join("blob");
    let data_path = blob_directory.join("data.json");
    if !data_path.is_file() {
        return Err(format!(
            "BlobUI settings were not found at {}. Select your Anytype folder with scripts/Settings.ps1 -InstallDirectory.",
            data_path.display()
        ));
    }
    let source_directory = resolve_source_directory()?;
    let rebuild_script = source_directory.join("scripts").join("Rebuild.ps1");
    if !rebuild_script.is_file() {
        return Err(format!(
            "BlobUI's Rebuild.ps1 was not found: {}",
            rebuild_script.display()
        ));
    }

    let staging_directory = blob_directory.join("staging");
    ensure_safe_staging_directory(&install_directory, &blob_directory, &staging_directory)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("The system clock could not create a staging name: {error}"))?
        .as_nanos();
    let override_path =
        staging_directory.join(format!("settings-v2-{}-{nonce}.json", std::process::id()));
    let overlay = json!({
        "version": 2,
        "settings": {
            "fontFamily": settings.font_family,
            "codeFontFamily": settings.code_font_family,
            "accentColor": settings.accent_color,
            "accentMode": settings.accent_mode,
            "accentPreset": settings.accent_preset,
            "reducedMotion": settings.reduced_motion,
            "tabHoverEffect": settings.tab_hover_effect,
            "linkHoverEffect": settings.link_hover_effect,
            "objectMentionHoverEffect": settings.object_mention_hover_effect,
        }
    });

    let result = (|| {
        let serialized = serde_json::to_vec_pretty(&overlay)
            .map_err(|error| format!("Could not encode the settings draft: {error}"))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&override_path)
            .map_err(|error| format!("Could not create a temporary settings file: {error}"))?;
        file.write_all(&serialized)
            .map_err(|error| format!("Could not write the temporary settings file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Could not flush the temporary settings file: {error}"))?;
        drop(file);

        let output = run_rebuild(&rebuild_script, &install_directory, &override_path)?;
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if !output.status.success() {
            return Err(format!("{}", summarize_process_error(&combined)));
        }

        let calendar_status = if combined.contains("BlobUI_CALENDAR_CACHE=disabled") {
            "disabled"
        } else if combined.contains("BlobUI_CALENDAR_CACHE=refreshed") {
            "refreshed"
        } else {
            "cached"
        };
        Ok(SaveResult {
            calendar_status: calendar_status.into(),
        })
    })();

    let _ = fs::remove_file(&override_path);
    result
}

fn validate_settings(mut settings: SettingsDraft) -> Result<SettingsDraft, String> {
    settings.font_family = validate_font_family(&settings.font_family, "Interface font")?;
    settings.code_font_family = validate_font_family(&settings.code_font_family, "Code font")?;
    settings.accent_color = settings.accent_color.trim().to_string();
    settings.accent_preset = settings.accent_preset.trim().to_string();

    if settings.accent_color.is_empty() || settings.accent_color.len() > 128 {
        return Err("Accent color must contain 1 to 128 characters.".into());
    }
    if settings
        .accent_color
        .chars()
        .any(|character| character.is_control() || ";{}<>\"'".contains(character))
    {
        return Err("Accent color contains unsupported characters.".into());
    }

    match settings.accent_mode.as_str() {
        "native" => {
            if settings.accent_preset != ""
                || !matches!(
                    settings.accent_color.as_str(),
                    "Default" | "var(--color-system-accent-100)"
                )
            {
                return Err("The Anytype accent selection is invalid.".into());
            }
        }
        "preset" => {
            let expected = preset_value(&settings.accent_preset)
                .ok_or_else(|| "The selected accent preset is unknown.".to_string())?;
            if settings.accent_color != expected {
                return Err("The accent preset does not match its stored color.".into());
            }
        }
        "custom" => {
            if !settings.accent_preset.is_empty() {
                return Err("A custom accent cannot include a preset key.".into());
            }
        }
        _ => return Err("Accent mode must be native, preset, or custom.".into()),
    }

    Ok(settings)
}

fn validate_font_family(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.chars().count() <= 80
        && value.chars().next().is_some_and(char::is_alphanumeric)
        && value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, ' ' | '_' | '-'));
    if !valid {
        return Err(format!(
            "{label} must use letters, numbers, spaces, underscores, or hyphens only."
        ));
    }
    Ok(value.to_string())
}

fn settings_from_data(data: &Value) -> SettingsDraft {
    let normalized = data.get("settings").unwrap_or(&Value::Null);
    let local = data
        .get("settingsV2")
        .and_then(|settings_v2| settings_v2.get("settings"));
    let font_family = read_string(local, "fontFamily")
        .or_else(|| read_string(Some(normalized), "fontFamily"))
        .unwrap_or_else(|| DEFAULT_FONT.into());
    let code_font_family = read_string(local, "codeFontFamily")
        .or_else(|| read_string(Some(normalized), "codeFontFamily"))
        .unwrap_or_else(|| DEFAULT_CODE_FONT.into());
    let accent_color = read_string(local, "accentColor")
        .or_else(|| read_string(Some(normalized), "accentColor"))
        .unwrap_or_else(|| DEFAULT_ACCENT.into());
    let reduced_motion = read_bool(local, "reducedMotion")
        .or_else(|| read_bool(Some(normalized), "reducedMotion"))
        .unwrap_or(true);
    let tab_hover_effect = read_bool(local, "tabHoverEffect")
        .or_else(|| read_bool(Some(normalized), "tabHoverEffect"))
        .unwrap_or(false);
    let link_hover_effect = read_bool(local, "linkHoverEffect")
        .or_else(|| read_bool(Some(normalized), "linkHoverEffect"))
        .unwrap_or(false);
    let object_mention_hover_effect = read_bool(local, "objectMentionHoverEffect")
        .or_else(|| read_bool(Some(normalized), "objectMentionHoverEffect"))
        .unwrap_or(false);

    let local_mode = read_string(local, "accentMode");
    let local_preset = read_string(local, "accentPreset");
    let (accent_mode, accent_preset, accent_color) = match local_mode.as_deref() {
        Some("native") => ("native".into(), String::new(), accent_color),
        Some("preset") => (
            "preset".into(),
            local_preset.unwrap_or_default(),
            accent_color,
        ),
        Some("custom") => ("custom".into(), String::new(), accent_color),
        _ => accent_for_ui(&accent_color),
    };

    SettingsDraft {
        font_family,
        code_font_family,
        accent_color,
        accent_mode,
        accent_preset,
        reduced_motion,
        tab_hover_effect,
        link_hover_effect,
        object_mention_hover_effect,
    }
}

fn accent_for_ui(value: &str) -> (String, String, String) {
    let normalized = value.trim();
    let lower = normalized.to_ascii_lowercase();
    if matches!(lower.as_str(), "default" | "var(--color-system-accent-100)") {
        return ("native".into(), String::new(), "Default".into());
    }
    if lower == "#de50de" || lower == "blob pink" {
        return ("preset".into(), "blob-pink".into(), "Blob Pink".into());
    }
    let aliases = [
        ("grey", "Grey", "grey"),
        ("var(--color-grey)", "Grey", "grey"),
        ("yellow", "Yellow", "yellow"),
        ("var(--color-yellow)", "Yellow", "yellow"),
        ("amber", "Amber", "amber"),
        ("var(--color-amber, var(--color-orange))", "Amber", "amber"),
        ("red", "Red", "red"),
        ("var(--color-red)", "Red", "red"),
        ("pink", "Pink", "pink"),
        ("var(--color-pink)", "Pink", "pink"),
        ("purple", "Purple", "purple"),
        ("var(--color-purple)", "Purple", "purple"),
        ("blue", "Blue", "blue"),
        ("var(--color-blue)", "Blue", "blue"),
        ("sky", "Sky", "sky"),
        ("var(--color-sky, var(--color-ice))", "Sky", "sky"),
        ("teal", "Teal", "teal"),
        ("var(--color-teal)", "Teal", "teal"),
        ("green", "Green", "green"),
        ("var(--color-green, var(--color-lime))", "Green", "green"),
    ];
    if let Some((_, preset_value, key)) = aliases.iter().find(|(alias, _, _)| *alias == lower) {
        return ("preset".into(), (*key).into(), (*preset_value).into());
    }
    ("custom".into(), String::new(), normalized.into())
}

fn preset_value(key: &str) -> Option<&'static str> {
    match key {
        "grey" => Some("Grey"),
        "yellow" => Some("Yellow"),
        "amber" => Some("Amber"),
        "red" => Some("Red"),
        "pink" => Some("Pink"),
        "purple" => Some("Purple"),
        "blue" => Some("Blue"),
        "sky" => Some("Sky"),
        "teal" => Some("Teal"),
        "green" => Some("Green"),
        "blob-pink" => Some("Blob Pink"),
        _ => None,
    }
}

fn read_anytype_theme(path: &Path) -> String {
    let contents = fs::read_to_string(path).unwrap_or_default();
    parse_anytype_theme(&contents)
}

fn parse_anytype_theme(contents: &str) -> String {
    let value = serde_json::from_str::<Value>(contents)
        .ok()
        .and_then(|root| root.get("theme").and_then(Value::as_str).map(str::to_owned));
    match value.as_deref() {
        Some("light") => "light".into(),
        Some("dark") => "dark".into(),
        Some("system") => "system".into(),
        _ => "system".into(),
    }
}

fn native_theme(theme: &str) -> Option<tauri::Theme> {
    match theme {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    }
}

fn apply_native_theme(app: &tauri::AppHandle, theme: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_theme(native_theme(theme));
    }
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| {
        format!(
            "Could not read BlobUI settings at {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|_| "BlobUI data.json is not valid JSON.".into())
}

fn read_string(value: Option<&Value>, name: &str) -> Option<String> {
    value
        .and_then(|object| object.get(name))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn read_bool(value: Option<&Value>, name: &str) -> Option<bool> {
    value
        .and_then(|object| object.get(name))
        .and_then(Value::as_bool)
}

fn app_data_directory() -> PathBuf {
    non_empty_path(env::var_os("APPDATA"))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("anytype")
}

fn default_install_directory(app_data: &Path) -> PathBuf {
    // app_data_directory() already resolves to %APPDATA%/anytype.
    app_data.to_path_buf()
}

fn non_empty_path(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value
        .filter(|value| !value.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
}

fn resolve_install_directory(app_data: &Path) -> Result<PathBuf, String> {
    resolve_install_directory_from(
        app_data,
        env::var_os("BLOBUI_INSTALL_DIRECTORY"),
        argument_value("--install-directory"),
        env::current_exe().ok().as_deref(),
    )
}

fn resolve_install_directory_from(
    app_data: &Path,
    environment_path: Option<std::ffi::OsString>,
    argument_path: Option<String>,
    executable: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(path) = non_empty_path(environment_path) {
        return Ok(path);
    }
    if let Some(path) = non_empty_path(argument_path.map(Into::into)) {
        return Ok(path);
    }

    if let Some(executable) = executable {
        for ancestor in executable.ancestors().skip(1) {
            if ancestor.join("scripts").join("Rebuild.ps1").is_file() {
                if let Some(pointer) = read_install_pointer(&ancestor.join(".blobui-install.json"))
                {
                    return Ok(pointer);
                }
                if ancestor
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("runtime"))
                    && ancestor.parent().is_some_and(|parent| {
                        parent
                            .file_name()
                            .is_some_and(|name| name.eq_ignore_ascii_case("blob"))
                    })
                {
                    if let Some(install_root) = ancestor.parent().and_then(Path::parent) {
                        return Ok(install_root.to_path_buf());
                    }
                }
                break;
            }
        }
    }
    Ok(default_install_directory(app_data))
}

fn argument_value(name: &str) -> Option<String> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next();
        }
        if let Some(value) = argument.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

fn read_install_pointer(path: &Path) -> Option<PathBuf> {
    let data = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    value
        .get("installDirectory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
}

fn resolve_source_directory() -> Result<PathBuf, String> {
    if let Some(root) = non_empty_path(env::var_os("BLOBUI_SOURCE_DIRECTORY")) {
        if root.join("scripts").join("Rebuild.ps1").is_file() {
            return Ok(root);
        }
        return Err("BLOBUI_SOURCE_DIRECTORY does not contain scripts/Rebuild.ps1.".into());
    }
    let executable = env::current_exe()
        .map_err(|error| format!("Could not locate the settings app executable: {error}"))?;
    executable
        .ancestors()
        .skip(1)
        .find(|ancestor| ancestor.join("scripts").join("Rebuild.ps1").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            "BlobUI's Rebuild.ps1 could not be found beside the app or in its source folder.".into()
        })
}

fn ensure_not_release_path(path: &Path) -> Result<(), String> {
    if path.components().any(|component| match component {
        Component::Normal(part) => part.eq_ignore_ascii_case("Releases"),
        _ => false,
    }) {
        return Err("BlobUI will not write custom.css for a directory inside Releases.".into());
    }
    Ok(())
}

fn ensure_safe_staging_directory(
    install: &Path,
    blob: &Path,
    staging: &Path,
) -> Result<(), String> {
    ensure_not_release_path(install)?;
    if !blob.is_dir() {
        return Err("The BlobUI data folder is missing.".into());
    }
    reject_reparse_component(blob)?;
    fs::create_dir_all(staging)
        .map_err(|error| format!("Could not prepare the BlobUI staging folder: {error}"))?;
    reject_reparse_component(staging)?;
    let canonical_blob = fs::canonicalize(blob)
        .map_err(|error| format!("Could not resolve the BlobUI data folder: {error}"))?;
    let canonical_staging = fs::canonicalize(staging)
        .map_err(|error| format!("Could not resolve the BlobUI staging folder: {error}"))?;
    if !canonical_staging.starts_with(&canonical_blob) {
        return Err("The BlobUI staging folder resolves outside the BlobUI data folder.".into());
    }
    Ok(())
}

fn reject_reparse_component(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        if !ancestor.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|error| format!("Could not inspect a BlobUI data path: {error}"))?;
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Err("A BlobUI data folder contains a symbolic link or reparse point.".into());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn run_rebuild(script: &Path, install: &Path, override_path: &Path) -> Result<Output, String> {
    let arguments = [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ];
    for shell in ["pwsh.exe", "powershell.exe"] {
        let mut command = Command::new(shell);
        command
            .args(arguments)
            .arg(script)
            .arg("-InstallDirectory")
            .arg(install)
            .arg("-SettingsOverridePath")
            .arg(override_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        match command.output() {
            Ok(output) => return Ok(output),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("Could not start {shell}: {error}")),
        }
    }
    Err("PowerShell 7 (pwsh) or Windows PowerShell could not be found.".into())
}

fn summarize_process_error(output: &str) -> String {
    let cleaned = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("BlobUI_CALENDAR_CACHE="))
        .collect::<Vec<_>>();
    let message = cleaned
        .iter()
        .rev()
        .take(8)
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    if message.is_empty() {
        "BlobUI could not save these settings. Check that Anytype and BlobUI are set up, then try again.".into()
    } else if message.chars().count() > 900 {
        let tail = message
            .chars()
            .rev()
            .take(900)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        format!("BlobUI could not save these settings: {tail}")
    } else {
        format!("BlobUI could not save these settings: {message}")
    }
}

fn configure_webview_profile() {
    let Some(profile) = env::var_os("BLOBUI_WEBVIEW_DATA_DIR") else {
        return;
    };
    let path = PathBuf::from(profile);
    if fs::create_dir_all(&path).is_ok() {
        env::set_var("WEBVIEW2_USER_DATA_FOLDER", path);
    }
}

fn main() {
    configure_webview_profile();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            if !arguments.iter().any(|argument| argument == "--background") {
                show_settings_window(app);
            }
        }))
        .invoke_handler(tauri::generate_handler![
            configure_connection,
            get_app_state,
            save_settings,
            minimize_window,
            hide_window,
            start_window_drag,
            open_project_repository,
            open_author_profile,
            quit_app
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let theme = read_anytype_theme(&app_data_directory().join("devconfig.json"));
            if let Some(window) = app.get_webview_window("main") {
                window.set_theme(native_theme(&theme))?;
            }
            hotkey::start(app.handle().clone());
            if !env::args().any(|argument| argument == "--background") {
                show_settings_window(app.handle());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running BlobUI Settings");
}

impl Default for SettingsDraft {
    fn default() -> Self {
        Self {
            font_family: DEFAULT_FONT.into(),
            code_font_family: DEFAULT_CODE_FONT.into(),
            accent_color: DEFAULT_ACCENT.into(),
            accent_mode: "preset".into(),
            accent_preset: "blob-pink".into(),
            reduced_motion: true,
            tab_hover_effect: false,
            link_hover_effect: false,
            object_mention_hover_effect: false,
        }
    }
}

#[cfg(test)]
mod save_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_v2_values_before_legacy_values() {
        let data = json!({
            "settings": {
                "fontFamily": "Old Font",
                "codeFontFamily": "Old Code",
                "accentColor": "#123456"
            },
            "settingsV2": {
                "version": 2,
                "settings": {
                    "fontFamily": "Inter",
                    "codeFontFamily": "Cascadia Code",
                    "accentColor": "Purple",
                    "accentMode": "preset",
                "accentPreset": "purple",
                "reducedMotion": false,
                "tabHoverEffect": true,
                "linkHoverEffect": true,
                "objectMentionHoverEffect": true
                }
            }
        });
        let settings = settings_from_data(&data);
        assert_eq!(settings.font_family, "Inter");
        assert_eq!(settings.accent_color, "Purple");
        assert_eq!(settings.accent_preset, "purple");
        assert!(!settings.reduced_motion);
        assert!(settings.tab_hover_effect);
        assert!(settings.link_hover_effect);
        assert!(settings.object_mention_hover_effect);
    }

    #[test]
    fn accessibility_settings_default_and_read_legacy_values() {
        let defaults = SettingsDraft::default();
        assert!(defaults.reduced_motion);
        assert!(!defaults.tab_hover_effect);
        assert!(!defaults.link_hover_effect);
        assert!(!defaults.object_mention_hover_effect);

        let data = json!({
            "settings": {
                "reducedMotion": false,
                "tabHoverEffect": true,
                "linkHoverEffect": true,
                "objectMentionHoverEffect": true
            }
        });
        let settings = settings_from_data(&data);
        assert!(!settings.reduced_motion);
        assert!(settings.tab_hover_effect);
        assert!(settings.link_hover_effect);
        assert!(settings.object_mention_hover_effect);
    }

    #[test]
    fn migrates_legacy_accent_tokens_into_picker_presets() {
        let data = json!({
            "settings": {
                "fontFamily": "Libron",
                "codeFontFamily": "Consolas",
                "accentColor": "var(--color-sky, var(--color-ice))"
            }
        });
        let settings = settings_from_data(&data);
        assert_eq!(settings.accent_mode, "preset");
        assert_eq!(settings.accent_preset, "sky");
        assert_eq!(settings.accent_color, "Sky");
    }

    #[test]
    fn keeps_supported_legacy_custom_colors_without_narrowing_them_to_hex() {
        let data = json!({
            "settings": {
                "fontFamily": "Libron",
                "codeFontFamily": "Consolas",
                "accentColor": "rgba(12 34 56 / 50%)"
            }
        });
        let settings = settings_from_data(&data);
        assert_eq!(settings.accent_mode, "custom");
        assert_eq!(settings.accent_color, "rgba(12 34 56 / 50%)");
        assert!(validate_settings(settings).is_ok());
    }

    #[test]
    fn reads_only_the_root_anytype_theme_value() {
        assert_eq!(
            parse_anytype_theme(r#"{"theme":"dark","settings":{"theme":"light"}}"#),
            "dark"
        );
        assert_eq!(parse_anytype_theme(r#"{"theme":"light"}"#), "light");
        assert_eq!(parse_anytype_theme(r#"{"theme":"unknown"}"#), "system");
    }

    #[test]
    fn default_install_path_does_not_append_a_second_anytype_directory() {
        let app_data = Path::new(r"C:\Users\test\AppData\Roaming\anytype");
        assert_eq!(default_install_directory(app_data), app_data);
    }

    #[test]
    fn empty_install_overrides_do_not_hide_an_existing_default_installation() {
        let app_data = Path::new(r"C:\Users\test\AppData\Roaming\anytype");
        for value in [None, Some(""), Some("   ")] {
            let resolved =
                resolve_install_directory_from(app_data, value.map(Into::into), None, None)
                    .unwrap();
            assert_eq!(resolved, app_data);
        }
        let custom = r"D:\Anytype data";
        assert_eq!(
            resolve_install_directory_from(app_data, Some(custom.into()), None, None).unwrap(),
            Path::new(custom)
        );
        assert_eq!(
            resolve_install_directory_from(app_data, Some("".into()), Some(custom.into()), None)
                .unwrap(),
            Path::new(custom)
        );
    }

    #[test]
    fn anytype_theme_selects_matching_native_window_theme() {
        assert_eq!(native_theme("light"), Some(tauri::Theme::Light));
        assert_eq!(native_theme("dark"), Some(tauri::Theme::Dark));
        assert_eq!(native_theme("system"), None);
    }

    #[test]
    fn rejects_font_and_accent_injection() {
        assert!(validate_font_family("Arial; color:red", "Font").is_err());
        let invalid_accent = SettingsDraft {
            accent_color: "#ffffff; color:red".into(),
            accent_mode: "custom".into(),
            accent_preset: String::new(),
            ..SettingsDraft::default()
        };
        assert!(validate_settings(invalid_accent).is_err());
    }
}
