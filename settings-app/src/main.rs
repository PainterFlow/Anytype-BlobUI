#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

mod api;
mod autostart;
mod css;
mod connection;
mod folder_dialog;
mod hotkey;
mod storage;
mod updater;

static SAVE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static UPDATER_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static SHOW_ONBOARDING: AtomicBool = AtomicBool::new(false);

struct SaveGuard;
impl Drop for SaveGuard {
    fn drop(&mut self) {
        SAVE_IN_PROGRESS.store(false, Ordering::Release);
    }
}

struct UpdaterGuard;
impl Drop for UpdaterGuard {
    fn drop(&mut self) {
        UPDATER_IN_PROGRESS.store(false, Ordering::Release);
    }
}

fn acquire_save_guard(message: &str) -> Result<SaveGuard, String> {
    SAVE_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| SaveGuard)
        .map_err(|_| message.to_string())
}

fn acquire_updater_guard() -> Result<UpdaterGuard, String> {
    UPDATER_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| UpdaterGuard)
        .map_err(|_| "An update check or installation is already in progress.".into())
}

fn acquire_save_and_updater_guards() -> Result<(SaveGuard, UpdaterGuard), String> {
    let save = acquire_save_guard("Wait for the current settings save to finish.")?;
    match acquire_updater_guard() {
        Ok(updater) => Ok((save, updater)),
        Err(error) => {
            drop(save);
            Err(error)
        }
    }
}

const DEFAULT_FONT: &str = "Libron";
const DEFAULT_CODE_FONT: &str = "Consolas";
const DEFAULT_ACCENT: &str = "#DE50DE";
const DEFAULT_API_IP: &str = "127.0.0.1";

struct SecretString(String);

impl SecretString {
    fn new(value: String) -> Self { Self(value) }
}

impl std::ops::Deref for SecretString {
    type Target = str;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        // Clear the key before the allocation is released.
        unsafe { self.0.as_mut_vec().fill(0); }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SettingsDraft {
    font_family: String,
    code_font_family: String,
    accent_color: String,
    accent_mode: String,
    accent_preset: String,
    #[serde(default)]
    banner_image: String,
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
    default_banner_preview: String,
    message: Option<String>,
    hotkey_error: Option<String>,
    onboarding_complete: bool,
    api_port: u16,
    api_ip: String,
    has_saved_api_key: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveResult {
    calendar_status: String,
}

#[tauri::command]
async fn reveal_api_key() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(read_saved_api_key)
        .await
        .map_err(|_| "Could not read the saved API key.".to_string())?
}

fn read_saved_api_key() -> Result<String, String> {
    let install = resolve_install_directory(&app_data_directory())?;
    reject_reparse_component(&install)?;
    let credential = install.join("blob/credentials/anytype-api-key.dpapi");
    reject_reparse_component(&credential)?;
    let encrypted = fs::read_to_string(&credential)
        .map_err(|_| "Could not read the saved API key.".to_string())?;
    connection::decrypt_saved_key(encrypted.trim())
}

#[tauri::command]
fn get_app_state(app: tauri::AppHandle) -> Result<AppState, String> {
    let state = load_app_state()?;
    apply_native_theme(&app, &state.theme);
    Ok(state)
}

#[tauri::command]
async fn save_settings(settings: SettingsDraft) -> Result<SaveResult, String> {
    let guard = acquire_save_guard("Settings are already being saved.")?;
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
    #[serde(default)]
    keep_existing_key: bool,
    #[serde(default)]
    ip: Option<String>,
    port: Option<u16>,
}

#[tauri::command]
async fn configure_connection(connection: ConnectionDraft) -> Result<SaveResult, String> {
    let guard = acquire_save_guard("Wait for the current save to finish.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        configure_connection_blocking(connection)
    })
    .await
    .map_err(|_| "The connection task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn get_updater_state() -> Result<updater::UpdaterState, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        with_startup_state(updater::get_state(&app_data, &install, &runtime)?)
    })
    .await
    .map_err(|_| "The updater status task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn save_updater_settings(settings: updater::UpdaterSettingsDraft) -> Result<updater::UpdaterState, String> {
    let (save_guard, updater_guard) = acquire_save_and_updater_guards()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _save_guard = save_guard;
        let _updater_guard = updater_guard;
        let app_data = app_data_directory();
        let default_install = resolve_default_install_directory(&app_data)?;
        let previous_start_on_login = autostart::is_enabled()?;
        let start_on_login = autostart::set_enabled(settings.start_on_login)?;
        match updater::save_settings(&app_data, settings, &default_install) {
            Ok(mut state) => {
                state.start_on_login = start_on_login;
                Ok(state)
            }
            Err(error) => match autostart::set_enabled(previous_start_on_login) {
                Ok(_) => Err(error),
                Err(rollback) => Err(format!("{error} The previous startup setting could not be restored: {rollback}")),
            },
        }
    })
    .await
    .map_err(|_| "The updater settings task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn select_anytype_folder(
    app: tauri::AppHandle,
    current_directory: Option<String>,
) -> Result<Option<String>, String> {
    #[cfg(windows)]
    let owner = app
        .get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|handle| handle.0 as isize);
    #[cfg(not(windows))]
    let owner = {
        let _ = app;
        None
    };
    tauri::async_runtime::spawn_blocking(move || {
        folder_dialog::select_anytype_folder(current_directory, owner)
    })
    .await
    .map_err(|_| "The folder selection dialog stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn check_for_updates() -> Result<updater::UpdaterState, String> {
    let guard = acquire_updater_guard()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        with_startup_state(updater::check_for_updates(&app_data, &install, &runtime)?)
    })
    .await
    .map_err(|_| "The update check task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn apply_update(
    app: tauri::AppHandle,
    automatic: Option<bool>,
) -> Result<updater::UpdaterState, String> {
    let (save_guard, updater_guard) = acquire_save_and_updater_guards()?;
    let start_on_login = autostart::is_enabled()?;
    let state = tauri::async_runtime::spawn_blocking(move || {
        let _save_guard = save_guard;
        let _updater_guard = updater_guard;
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        updater::apply_update(&app_data, &install, &runtime, automatic.unwrap_or(false))
    })
    .await
    .map_err(|_| "The update installation task stopped unexpectedly.".to_string())??;
    let mut state = state;
    state.start_on_login = start_on_login;
    if state.status == "restarting" {
        // The installer has been verified and started. Exit only on this
        // explicit handoff state so every earlier failure leaves Settings open.
        app.exit(0);
    }
    Ok(state)
}

#[tauri::command]
fn simulate_update() -> Result<updater::UpdaterState, String> {
    let app_data = app_data_directory();
    let install = resolve_install_directory(&app_data)?;
    let runtime = resolve_runtime_directory_for_install(&install)?;
    with_startup_state(updater::simulate_update(&app_data, &install, &runtime)?)
}

#[tauri::command]
fn uninstall_blobui(app: tauri::AppHandle) -> Result<(), String> {
    let (_save, _updater) = acquire_save_and_updater_guards()?;
    let install = resolve_install_directory(&app_data_directory())?;
    let executable = env::current_exe().map_err(|e| e.to_string())?;
    let directory = updater::resolve_app_directory(&install, Some(&executable), None)?;
    let installer = directory.join("blobui-installer.exe");
    storage::reject_reparse_path(&installer)?;
    if !installer.is_file() {
        return Err("The uninstaller is missing. Run the latest BlobUI installer first.".into());
    }
    let mut child = std::process::Command::new(&installer)
        .arg("--uninstall")
        .arg("--app-directory")
        .arg(&directory)
        .arg("--anytype-directory")
        .arg(&install)
        .arg("--wait-pid")
        .arg(std::process::id().to_string())
        .spawn()
        .map_err(|e| format!("Could not start the uninstaller: {e}"))?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    match child.try_wait() {
        Ok(Some(status)) => {
            return Err(format!(
                "The uninstaller exited before starting ({status})."
            ))
        }
        Ok(None) => {}
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "Could not verify that the uninstaller started: {error}"
            ));
        }
    }
    match autostart::is_enabled() {
        Ok(true) => {
            if let Err(error) = autostart::set_enabled(false) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
        Ok(false) => {}
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }
    app.exit(0);
    Ok(())
}

fn with_startup_state(mut state: updater::UpdaterState) -> Result<updater::UpdaterState, String> {
    state.start_on_login = autostart::is_enabled()?;
    Ok(state)
}

fn configure_connection_blocking(connection: ConnectionDraft) -> Result<SaveResult, String> {
    let mut connection = normalize_connection(connection)?;
    let mut api_key = SecretString::new(std::mem::take(&mut connection.api_key));
    let install = resolve_install_directory(&app_data_directory())?;
    let (_, data_path, css_path, credential_path) = save_paths(&install)?;
    let runtime = resolve_runtime_directory_for_install(&install)?;
    let mut data = read_json_file(&data_path)?;
    if !data.is_object() {
        return Err("BlobUI data.json must contain a JSON object.".into());
    }

    let current_settings = validate_settings(settings_from_data(&data))?;
    apply_settings(&mut data, &install, &current_settings);
    let port = connection.port.unwrap_or(31009);
    let ip = connection.ip.as_deref().unwrap_or(DEFAULT_API_IP);
    let mut protected_key = None;
    let calendar_status;
    if connection.enabled {
        if connection.keep_existing_key {
            api_key = SecretString::new(read_saved_api_key_at(&install)?);
        }
        let connection_result = (|| {
            api::verify_connection(&api_key, ip, port)?;
            if !connection.keep_existing_key {
                protected_key = Some(connection::encrypt_api_key(&api_key)?);
            }
            Ok::<_, String>(api::calendar_colors(&api_key, ip, port))
        })();
        let colors_result = connection_result?;
        match colors_result {
            Ok(colors) => {
                data["calendarObjectColors"] = colors;
                calendar_status = "refreshed";
            }
            Err(_) => {
                // A connection change must not carry calendar data forward
                // from a different API identity or an earlier permission set.
                data["calendarObjectColors"] = json!({});
                calendar_status = "cached";
            }
        }
    } else {
        data["calendarObjectColors"] = json!({});
        calendar_status = "disabled";
    }

    data["apiConnection"] = json!({
        "enabled": connection.enabled,
        "ip": ip,
        "port": port,
    });
    let css = css::compile(&runtime, &install, &data)?;
    let serialized = serialize_data(&data)?;
    let credential = match protected_key.as_deref() {
        Some(value) => storage::CredentialChange::Replace(value),
        None if !connection.enabled => storage::CredentialChange::Remove,
        None => storage::CredentialChange::Keep,
    };
    storage::commit_settings(
        &css_path,
        &data_path,
        &credential_path,
        css.as_bytes(),
        &serialized,
        credential,
    )?;
    Ok(SaveResult { calendar_status: calendar_status.into() })
}

fn normalize_connection(mut connection: ConnectionDraft) -> Result<ConnectionDraft, String> {
    if connection.port == Some(0) {
        return Err("Port must be between 1 and 65535.".into());
    }
    connection.ip = Some(normalize_api_ip(connection.ip.as_deref())?);
    connection.api_key = connection.api_key.trim().to_string();
    if connection.enabled
        && !connection.keep_existing_key
        && (connection.api_key.is_empty()
            || connection.api_key.len() > 4096
            || connection.api_key.chars().any(char::is_control))
    {
        return Err("Enter your Anytype API v2 key.".into());
    }
    if connection.keep_existing_key {
        connection.api_key.clear();
    }
    if !connection.enabled {
        connection.api_key.clear();
        connection.keep_existing_key = false;
    }
    Ok(connection)
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
    if SAVE_IN_PROGRESS.load(Ordering::Acquire) || UPDATER_IN_PROGRESS.load(Ordering::Acquire) {
        return Err("Wait for the current save or updater operation to finish before quitting.".into());
    }
    app.exit(0);
    Ok(())
}

fn load_app_state() -> Result<AppState, String> {
    let app_data = app_data_directory();
    let install_directory = resolve_install_directory(&app_data)?;
    let data_path = install_directory.join("blob").join("data.json");
    let theme = read_anytype_theme(&install_directory.join("devconfig.json"));

    let mut onboarding_complete = false;
    let mut api_port = 31009;
    let mut api_ip = String::new();
    let mut has_saved_api_key = false;
    let (ready, settings, message) = if data_path.is_file() {
        let data = read_json_file(&data_path)?;
        let api_connection = data.pointer("/apiConnection");
        let credential_path = install_directory
            .join("blob")
            .join("credentials")
            .join("anytype-api-key.dpapi");
        let credential_exists = credential_path.is_file();
        let configured_api_state = data
            .pointer("/apiConnection/enabled")
            .and_then(Value::as_bool);
        onboarding_complete = configured_api_state.is_some() || credential_exists;
        api_port = api_connection
            .and_then(|connection| connection.get("port"))
            .and_then(Value::as_u64)
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port > 0)
            .unwrap_or(31009);
        api_ip = api_connection
            .and_then(|connection| read_string(Some(connection), "ip"))
            .filter(|ip| ip != DEFAULT_API_IP)
            .unwrap_or_default();
        has_saved_api_key = credential_exists && configured_api_state.unwrap_or(true);
        let settings = settings_from_data(&data);
        (true, settings, None)
    } else {
        (
            false,
            SettingsDraft::default(),
            Some(format!(
                "BlobUI settings were not found at {}. Install BlobUI into the selected Anytype folder, then reopen Settings.",
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
        default_banner_preview: resolve_runtime_directory_for_install(&install_directory)
            .ok()
            .and_then(|runtime| css::default_banner_preview(&runtime).ok())
            .unwrap_or_default(),
        message,
        hotkey_error: hotkey::error(),
        onboarding_complete: onboarding_complete && !SHOW_ONBOARDING.load(Ordering::Acquire),
        api_port,
        api_ip,
        has_saved_api_key,
    })
}

fn save_settings_blocking(settings: SettingsDraft) -> Result<SaveResult, String> {
    let settings = validate_settings(settings)?;
    let install = resolve_install_directory(&app_data_directory())?;
    let (_, data_path, css_path, credential_path) = save_paths(&install)?;
    let runtime = resolve_runtime_directory_for_install(&install)?;
    let mut data = read_json_file(&data_path)?;
    if !data.is_object() {
        return Err("BlobUI data.json must contain a JSON object.".into());
    }
    apply_settings(&mut data, &install, &settings);

    let mut calendar_status = "cached";
    let api_connection = data.get("apiConnection");
    let api_enabled = api_connection
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        // Older installs had no apiConnection object and relied on a stored
        // key plus the legacy selected-space settings. Preserve their cache.
        .unwrap_or(true);
    if !api_enabled {
        data["calendarObjectColors"] = json!({});
        calendar_status = "disabled";
    } else if credential_path.is_file() {
        let ip = api_connection
            .and_then(|value| read_string(Some(value), "ip"))
            .map(|value| normalize_api_ip(Some(&value)))
            .transpose()?
            .unwrap_or_else(|| DEFAULT_API_IP.into());
        let port = api_connection
            .and_then(|value| value.get("port"))
            .and_then(Value::as_u64)
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port > 0)
            .unwrap_or(31009);
        if let Ok(api_key) = read_saved_api_key_at(&install).map(SecretString::new) {
            if let Ok(colors) = api::calendar_colors(&api_key, &ip, port) {
                data["calendarObjectColors"] = colors;
                calendar_status = "refreshed";
            }
        }
    }

    let css = css::compile(&runtime, &install, &data)?;
    let serialized = serialize_data(&data)?;
    storage::commit_settings(
        &css_path,
        &data_path,
        &credential_path,
        css.as_bytes(),
        &serialized,
        storage::CredentialChange::Keep,
    )?;
    Ok(SaveResult { calendar_status: calendar_status.into() })
}

fn save_paths(install: &Path) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf), String> {
    ensure_not_release_path(install)?;
    let blob = install.join("blob");
    let data = blob.join("data.json");
    let css = install.join("custom.css");
    let credential = blob.join("credentials").join("anytype-api-key.dpapi");
    for path in [install, blob.as_path(), data.as_path(), css.as_path(), credential.as_path()] {
        reject_reparse_component(path)?;
    }
    if !data.is_file() {
        return Err(format!(
            "BlobUI settings were not found at {}. Install BlobUI into the selected Anytype folder, then reopen Settings.",
            data.display()
        ));
    }
    if !install.is_dir() {
        return Err(format!("The selected Anytype folder does not exist: {}", install.display()));
    }
    Ok((blob, data, css, credential))
}

fn apply_settings(data: &mut Value, install: &Path, settings: &SettingsDraft) {
    data["installDirectory"] = Value::String(install.to_string_lossy().into_owned());
    let accent = if settings.accent_mode == "native" {
        "var(--color-system-accent-100)"
    } else {
        settings.accent_color.as_str()
    };
    let values = json!({
        "fontFamily": settings.font_family,
        "codeFontFamily": settings.code_font_family,
        "accentColor": accent,
        "bannerImage": settings.banner_image,
        "reducedMotion": settings.reduced_motion,
        "tabHoverEffect": settings.tab_hover_effect,
        "linkHoverEffect": settings.link_hover_effect,
        "objectMentionHoverEffect": settings.object_mention_hover_effect,
    });
    if !data.get("settings").is_some_and(Value::is_object) {
        data["settings"] = json!({});
    }
    if let (Some(target), Some(values)) = (data["settings"].as_object_mut(), values.as_object()) {
        for (name, value) in values {
            target.insert(name.clone(), value.clone());
        }
    }
    data["settingsV2"] = json!({
        "version": 2,
        "settings": {
            "fontFamily": settings.font_family,
            "codeFontFamily": settings.code_font_family,
            "accentColor": accent,
            "accentMode": settings.accent_mode,
            "accentPreset": settings.accent_preset,
            "bannerImage": settings.banner_image,
            "reducedMotion": settings.reduced_motion,
            "tabHoverEffect": settings.tab_hover_effect,
            "linkHoverEffect": settings.link_hover_effect,
            "objectMentionHoverEffect": settings.object_mention_hover_effect,
        }
    });
}

fn serialize_data(data: &Value) -> Result<Vec<u8>, String> {
    let mut serialized = serde_json::to_vec_pretty(data)
        .map_err(|error| format!("Could not encode BlobUI settings: {error}"))?;
    serialized.push(b'\n');
    Ok(serialized)
}

fn read_saved_api_key_at(install: &Path) -> Result<String, String> {
    let credential = install.join("blob/credentials/anytype-api-key.dpapi");
    reject_reparse_component(&credential)?;
    let encrypted = fs::read_to_string(&credential)
        .map_err(|_| "Could not read the saved API key. Enter a new key in Advanced settings.".to_string())?;
    connection::decrypt_saved_key(encrypted.trim())
}

fn validate_settings(mut settings: SettingsDraft) -> Result<SettingsDraft, String> {
    settings.font_family = validate_font_family(&settings.font_family, "Interface font")?;
    settings.code_font_family = validate_font_family(&settings.code_font_family, "Code font")?;
    settings.accent_color = settings.accent_color.trim().to_string();
    settings.accent_preset = settings.accent_preset.trim().to_string();
    settings.banner_image = css::validate_banner_image(&settings.banner_image)?;

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
        .unwrap_or(false);
    let tab_hover_effect = read_bool(local, "tabHoverEffect")
        .or_else(|| read_bool(Some(normalized), "tabHoverEffect"))
        .unwrap_or(false);
    let link_hover_effect = read_bool(local, "linkHoverEffect")
        .or_else(|| read_bool(Some(normalized), "linkHoverEffect"))
        .unwrap_or(false);
    let object_mention_hover_effect = read_bool(local, "objectMentionHoverEffect")
        .or_else(|| read_bool(Some(normalized), "objectMentionHoverEffect"))
        .unwrap_or(false);
    let banner_image = read_string(local, "bannerImage")
        .or_else(|| read_string(Some(normalized), "bannerImage"))
        .filter(|value| css::validate_banner_image(value).is_ok())
        .unwrap_or_default();

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
        banner_image,
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

fn normalize_api_ip(value: Option<&str>) -> Result<String, String> {
    let value = value.unwrap_or_default().trim();
    if value.is_empty() {
        return Ok(DEFAULT_API_IP.into());
    }
    value
        .parse::<IpAddr>()
        .map(|address| address.to_string())
        .map_err(|_| "API key IP must be a valid IPv4 or IPv6 address.".into())
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
    let saved = updater::saved_install_directory(&updater::preferences_path(app_data))?;
    resolve_install_directory_with_saved(
        app_data,
        saved,
        env::var_os("BLOBUI_INSTALL_DIRECTORY"),
        argument_value("--install-directory"),
        env::current_exe().ok().as_deref(),
    )
}

fn resolve_install_directory_with_saved(
    app_data: &Path,
    saved: Option<PathBuf>,
    environment_path: Option<std::ffi::OsString>,
    argument_path: Option<String>,
    executable: Option<&Path>,
) -> Result<PathBuf, String> {
    // The settings choice is persisted independently of the Setup shortcut's
    // original --install-directory argument, so a later launch keeps the user's
    // newly selected Anytype data folder.
    if let Some(path) = saved {
        return updater::validate_install_directory(&path);
    }
    let path = resolve_install_directory_from(
        app_data,
        environment_path,
        argument_path,
        executable,
    )?;
    reject_reparse_component(&path)?;
    Ok(path)
}

fn resolve_default_install_directory(app_data: &Path) -> Result<PathBuf, String> {
    let path = resolve_install_directory_from(
        app_data,
        env::var_os("BLOBUI_INSTALL_DIRECTORY"),
        argument_value("--install-directory"),
        env::current_exe().ok().as_deref(),
    )?;
    reject_reparse_component(&path)?;
    Ok(path)
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
            if has_runtime_files(ancestor) {
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

fn has_runtime_files(root: &Path) -> bool {
    root.join("Dev").is_dir() && root.join("assets").is_dir()
}

fn resolve_runtime_directory_for_install(install: &Path) -> Result<PathBuf, String> {
    let source_override = non_empty_path(env::var_os("BLOBUI_SOURCE_DIRECTORY"));
    let executable = env::current_exe().ok();
    resolve_runtime_directory_for_install_from(
        install,
        source_override.as_deref(),
        executable.as_deref(),
    )
}

fn resolve_runtime_directory_for_install_from(
    install: &Path,
    source_override: Option<&Path>,
    executable: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(root) = source_override {
        return validate_runtime_source_override(root);
    }

    let executable_runtime = executable_runtime_directory(executable);
    if executable_runtime
        .as_deref()
        .is_some_and(|root| !is_installed_runtime_directory(root))
    {
        return Ok(executable_runtime.unwrap());
    }

    // Installed Settings builds keep using the selected Anytype folder's runtime,
    // which is also where update checks and updates read their package metadata.
    let installed = install.join("blob/runtime");
    if has_runtime_files(&installed) {
        reject_reparse_component(&installed)?;
        return Ok(installed);
    }

    if let Some(root) = executable_runtime {
        if is_installed_runtime_directory(&root) {
            reject_reparse_component(&root)?;
        }
        return Ok(root);
    }

    Err("BlobUI's CSS source and assets could not be found beside the app or in its source folder.".into())
}

fn validate_runtime_source_override(root: &Path) -> Result<PathBuf, String> {
    if has_runtime_files(root) {
        Ok(root.to_path_buf())
    } else {
        Err("BLOBUI_SOURCE_DIRECTORY does not contain Dev and assets folders.".into())
    }
}

fn executable_runtime_directory(executable: Option<&Path>) -> Option<PathBuf> {
    executable?
        .ancestors()
        .skip(1)
        .find(|ancestor| has_runtime_files(ancestor))
        .map(Path::to_path_buf)
}

fn is_installed_runtime_directory(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("runtime"))
        && path.parent().is_some_and(|parent| {
            parent
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("blob"))
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

fn configure_webview_profile() {
    let Some(profile) = env::var_os("BLOBUI_WEBVIEW_DATA_DIR") else {
        return;
    };
    let path = PathBuf::from(profile);
    if fs::create_dir_all(&path).is_ok() {
        env::set_var("WEBVIEW2_USER_DATA_FOLDER", path);
    }
}

fn consume_pending_onboarding_request() -> bool {
    let Ok(executable) = env::current_exe() else {
        return false;
    };
    let Some(directory) = executable.parent() else {
        return false;
    };
    match fs::remove_file(directory.join(".blobui-onboarding-next")) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

fn main() {
    configure_webview_profile();
    SHOW_ONBOARDING.store(
        env::args().any(|argument| argument == "--onboarding" || argument == "-n"),
        Ordering::Release,
    );
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            let onboarding_argument = arguments
                .iter()
                .any(|argument| argument == "--onboarding" || argument == "-n");
            let pending_onboarding = consume_pending_onboarding_request();
            if onboarding_argument || pending_onboarding {
                SHOW_ONBOARDING.store(true, Ordering::Release);
                let _ = app.emit("blobui-show-onboarding", ());
            }
            if !arguments.iter().any(|argument| argument == "--background") {
                show_settings_window(app);
            }
        }))
        .invoke_handler(tauri::generate_handler![
            reveal_api_key,
            configure_connection,
            get_app_state,
            save_settings,
            get_updater_state,
            save_updater_settings,
            select_anytype_folder,
            check_for_updates,
            apply_update,
            simulate_update,
            uninstall_blobui,
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
            if consume_pending_onboarding_request() {
                SHOW_ONBOARDING.store(true, Ordering::Release);
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
            banner_image: String::new(),
            reduced_motion: false,
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
        assert!(!defaults.reduced_motion);
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
    fn saved_install_folder_overrides_stale_setup_launch_arguments_after_restart() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = workspace
            .join(".test-run")
            .join(format!("saved-install-resolution-{}-{nonce}", std::process::id()));
        let selected = root.join("chosen Anytype data");
        fs::create_dir_all(selected.join("blob")).unwrap();
        fs::write(
            selected.join("blob/data.json"),
            serde_json::to_vec(&json!({"installDirectory": selected})).unwrap(),
        )
        .unwrap();

        let resolved = resolve_install_directory_with_saved(
            &root.join("APPDATA/anytype"),
            Some(selected.clone()),
            Some(std::ffi::OsString::from(r"C:\Setup-selected\anytype")),
            Some(r"D:\Setup-selected\anytype".into()),
            None,
        )
        .unwrap();
        assert_eq!(resolved, selected.canonicalize().unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_runtime_wins_over_stale_installed_hover_css() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = workspace
            .join(".test-run")
            .join(format!("source-runtime-resolution-{}-{nonce}", std::process::id()));
        let source_runtime = root.join("workspace-runtime");
        let install = root.join("Anytype data");
        let installed_runtime = install.join("blob/runtime");
        fs::create_dir_all(source_runtime.join("Dev")).unwrap();
        fs::create_dir_all(source_runtime.join("assets/spacebanner")).unwrap();
        fs::create_dir_all(installed_runtime.join("Dev")).unwrap();
        fs::create_dir_all(installed_runtime.join("assets/spacebanner")).unwrap();
        fs::write(
            source_runtime.join("Dev/60-tab-hover-preview.css"),
            "/* @blobui-if-not tabHoverEffect */\n#preview.previewWrapper:has(.previewTab) { display:none!important; }\n/* @blobui-endif-not tabHoverEffect */",
        )
        .unwrap();
        fs::write(
            source_runtime.join("Dev/60-accessibility.css"),
            "/* @blobui-if-not linkHoverEffect */\n#preview.previewWrapper:has(.previewLink):not(:has(.previewTab)) { display:none!important; }\n/* @blobui-endif-not linkHoverEffect */\n/* @blobui-if-not objectMentionHoverEffect */\n#preview.previewWrapper:has(.previewDefault, .previewObject):not(:has(.previewTab)) { display:none!important; }\n/* @blobui-endif-not objectMentionHoverEffect */",
        )
        .unwrap();
        fs::write(
            installed_runtime.join("Dev/60-tab-hover-preview.css"),
            "/* stale-installed-tab-hover-rule */\n#preview.previewWrapper { display:none!important; }",
        )
        .unwrap();
        fs::write(source_runtime.join("assets/spacebanner/banner.png"), b"fixture").unwrap();
        fs::write(installed_runtime.join("assets/spacebanner/banner.png"), b"fixture").unwrap();

        let executable = source_runtime.join("settings-app/dist/blobui-settings.exe");
        let runtime = resolve_runtime_directory_for_install_from(&install, None, Some(&executable))
            .unwrap();
        assert_eq!(runtime, source_runtime);

        let mut data = json!({
            "installDirectory": install,
            "settings": {
                "fontFamily": "Arial",
                "codeFontFamily": "Consolas",
                "accentColor": "#3656D4",
                "reducedMotion": true,
                "tabHoverEffect": true,
                "linkHoverEffect": true,
                "objectMentionHoverEffect": true,
                "bannerImage": ""
            },
            "apiConnection": { "enabled": false }
        });
        let enabled = css::compile(&runtime, &install, &data).unwrap();
        assert!(!enabled.contains("stale-installed-tab-hover-rule"));
        assert!(!enabled.contains("#preview.previewWrapper:has(.previewTab)"));
        assert!(!enabled.contains("#preview.previewWrapper:has(.previewLink)"));
        assert!(!enabled.contains("#preview.previewWrapper:has(.previewDefault, .previewObject)"));

        data["settings"]["tabHoverEffect"] = json!(false);
        data["settings"]["linkHoverEffect"] = json!(false);
        data["settings"]["objectMentionHoverEffect"] = json!(false);
        let disabled = css::compile(&runtime, &install, &data).unwrap();
        assert!(!disabled.contains("stale-installed-tab-hover-rule"));
        assert!(disabled.contains("#preview.previewWrapper:has(.previewTab)"));
        assert!(disabled.contains("#preview.previewWrapper:has(.previewLink)"));
        assert!(disabled.contains("#preview.previewWrapper:has(.previewDefault, .previewObject)"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_runtime_override_precedes_source_and_installed_runtimes() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = workspace
            .join(".test-run")
            .join(format!("runtime-override-resolution-{}-{nonce}", std::process::id()));
        let source_runtime = root.join("workspace-runtime");
        let override_runtime = root.join("explicit-runtime");
        let install = root.join("Anytype data");
        fs::create_dir_all(source_runtime.join("Dev")).unwrap();
        fs::create_dir_all(source_runtime.join("assets")).unwrap();
        fs::create_dir_all(override_runtime.join("Dev")).unwrap();
        fs::create_dir_all(override_runtime.join("assets")).unwrap();
        fs::create_dir_all(install.join("blob/runtime/Dev")).unwrap();
        fs::create_dir_all(install.join("blob/runtime/assets")).unwrap();

        let executable = source_runtime.join("settings-app.exe");
        assert_eq!(
            resolve_runtime_directory_for_install_from(
                &install,
                Some(&override_runtime),
                Some(&executable),
            )
            .unwrap(),
            override_runtime
        );
        let missing = root.join("missing-runtime");
        assert!(resolve_runtime_directory_for_install_from(
            &install,
            Some(&missing),
            Some(&executable),
        )
        .unwrap_err()
        .contains("BLOBUI_SOURCE_DIRECTORY"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_executable_keeps_using_selected_install_runtime() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = workspace
            .join(".test-run")
            .join(format!("installed-runtime-resolution-{}-{nonce}", std::process::id()));
        let install = root.join("Anytype data");
        let selected_runtime = install.join("blob/runtime");
        let executable_runtime = root.join("previous-install/blob/runtime");
        fs::create_dir_all(selected_runtime.join("Dev")).unwrap();
        fs::create_dir_all(selected_runtime.join("assets")).unwrap();
        fs::create_dir_all(executable_runtime.join("Dev")).unwrap();
        fs::create_dir_all(executable_runtime.join("assets")).unwrap();

        let executable = executable_runtime.join("settings-app.exe");
        assert_eq!(
            resolve_runtime_directory_for_install_from(&install, None, Some(&executable)).unwrap(),
            selected_runtime
        );
        fs::remove_dir_all(root).unwrap();
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

    #[test]
    fn empty_api_ip_defaults_to_this_pc_loopback() {
        assert_eq!(normalize_api_ip(None).unwrap(), "127.0.0.1");
        assert_eq!(normalize_api_ip(Some("  ")).unwrap(), "127.0.0.1");
        assert_eq!(normalize_api_ip(Some("127.0.0.1")).unwrap(), "127.0.0.1");
    }

    #[test]
    fn api_ip_accepts_ipv4_and_ipv6_and_rejects_hostnames() {
        assert_eq!(normalize_api_ip(Some("192.168.1.12")).unwrap(), "192.168.1.12");
        assert_eq!(normalize_api_ip(Some("0:0:0:0:0:0:0:1")).unwrap(), "::1");
        assert!(normalize_api_ip(Some("anytype.local")).is_err());
    }

    #[test]
    fn preserving_a_saved_key_never_puts_key_text_in_the_connection_request() {
        let connection = normalize_connection(ConnectionDraft {
            enabled: true,
            api_key: String::new(),
            keep_existing_key: true,
            ip: None,
            port: None,
        })
        .unwrap();

        assert!(connection.enabled);
        assert!(connection.keep_existing_key);
        assert!(connection.api_key.is_empty());
        assert_eq!(connection.ip.as_deref(), Some("127.0.0.1"));
    }

    #[test]
    fn app_state_exposes_key_presence_without_serializing_the_key() {
        let state = AppState {
            ready: true,
            install_directory: String::new(),
            app_data_directory: String::new(),
            theme: "dark".into(),
            settings: SettingsDraft::default(),
            default_banner_preview: String::new(),
            message: None,
            hotkey_error: None,
            onboarding_complete: true,
            api_port: 31009,
            api_ip: String::new(),
            has_saved_api_key: true,
        };
        let serialized = serde_json::to_value(state).unwrap();
        assert_eq!(serialized["hasSavedApiKey"], true);
        assert!(serialized.get("apiKey").is_none());
    }
}
