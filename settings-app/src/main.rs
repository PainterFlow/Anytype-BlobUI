#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod metadata_cache;
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
mod backup;
mod css;
mod connection;
mod folder_dialog;
mod calculator;
mod kanban;
mod calendar;
mod codex;
mod codex_projects;
mod codex_identity;
mod anyblob_scope;
mod anyblob;
mod anyblob_blobui;
mod anyblob_content;
mod anyblob_creation;
mod anyblob_mentions;
mod marketplace;
mod category_identity;
mod kanban_api;
mod kanban_css;
mod kanban_snapshot;
mod hotkey;
mod storage;
mod settings_store;
mod updater;

static SAVE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static UPDATER_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static SHOW_ONBOARDING: AtomicBool = AtomicBool::new(false);
static ONBOARDING_READY: AtomicBool = AtomicBool::new(false);
static SETTINGS_VISIBLE: AtomicBool = AtomicBool::new(false);
static BACKGROUND_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

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

const DEFAULT_FONT: &str = "Inter";
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
    #[serde(default, deserialize_with = "codex::deserialize")]
    codex_integration: codex::Configuration,
    font_family: String,
    code_font_family: String,
    accent_color: String,
    accent_mode: String,
    accent_preset: String,
    #[serde(default = "default_include_selections")]
    include_selections: bool,
    #[serde(default)]
    banner_image: String,
    reduced_motion: bool,
    tab_hover_effect: bool,
    link_hover_effect: bool,
    object_mention_hover_effect: bool,
    #[serde(default = "default_kanban_style")]
    kanban_view_style: String,
    #[serde(default)]
    collapsed_categories: bool,
    #[serde(default = "default_calendar_colors_enabled")]
    calendar_colors_enabled: bool,
    #[serde(default, deserialize_with = "deserialize_sidebar_resize_mode")]
    sidebar_resize_mode: SidebarResizeMode,
    #[serde(default)]
    background_experiment: BackgroundExperiment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum BackgroundExperiment {
    #[default]
    Disabled,
    Normal,
    AutostartBackground,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum SidebarResizeMode {
    #[default]
    Off,
    Standard,
    Pro,
}

fn deserialize_sidebar_resize_mode<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<SidebarResizeMode, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(match value.as_str() {
        Some("standard") => SidebarResizeMode::Standard,
        Some("pro") => SidebarResizeMode::Pro,
        _ => SidebarResizeMode::Off,
    })
}

fn default_include_selections() -> bool {
    true
}

fn default_calendar_colors_enabled() -> bool {
    true
}

fn default_kanban_style() -> String {
    "default".into()
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
    info: Vec<String>,
}

#[tauri::command]
async fn reveal_api_key() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(read_saved_api_key)
        .await
        .map_err(|_| "Could not read the saved API key.".to_string())?
}

fn read_saved_api_key() -> Result<String, String> {
    let install = resolve_install_directory(&app_data_directory())?;
    let credential = api_credential_path(&install)?;
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
async fn get_saved_state_snapshot() -> Result<Value, String> {
    let guard = acquire_save_guard("Wait for the current save before reloading Settings.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        // One save guard keeps configuration and revision tokens from different
        // publications out of the same frontend baseline.
        Ok(json!({
            "app": load_app_state()?,
            "updater": with_startup_state(updater::get_state(&app_data, &install, &runtime)?)?,
            "formulas": kanban::load_kanban_formulas()?,
            "calendar": calendar::load_calendar_rules()?,
        }))
    }).await.map_err(|_| "Saved settings reload stopped. Your pending edits are kept.".to_string())?
}

#[tauri::command]
fn get_keyboard_shortcuts() -> Vec<hotkey::Shortcut> {
    hotkey::get_shortcuts()
}

#[tauri::command]
fn save_keyboard_shortcuts(shortcuts: Vec<hotkey::Shortcut>) -> Result<Vec<hotkey::Shortcut>, String> {
    let app_data = app_data_directory();
    #[cfg(not(test))]
    let install = resolve_install_directory(&app_data)?;
    #[cfg(test)]
    {
        let path = app_data.parent().unwrap_or(&app_data).join("BlobUI/shortcuts.json");
        return hotkey::save_shortcuts(&path, shortcuts);
    }
    #[cfg(not(test))]
    {
        let _ = settings_data_path(&install)?;
        let root = settings_root_for_install(&install)?;
        let mut data = settings_store::load(&root)?;
        let before = data.clone();
        hotkey::validate_shortcuts(&shortcuts)?;
        data["keybinds"] = json!(shortcuts);
        settings_store::save(&root, &data)?;
        match hotkey::set_shortcuts(shortcuts) {
            Ok(saved) => Ok(saved),
            Err(error) => {
                if let Err(rollback) = settings_store::save(&root, &before) {
                    return Err(format!("{error} The saved Settings shortcut list could not be restored: {rollback}"));
                }
                Err(error)
            }
        }
    }
}

#[tauri::command]
fn set_shortcut_capture(active: bool) {
    hotkey::set_capture_active(active);
}

#[tauri::command]
fn get_anytype_version() -> String {
    let mut roots = Vec::new();
    for variable in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env::var_os(variable) {
            let root = PathBuf::from(root);
            roots.push(root.join("Programs/Anytype"));
            roots.push(root.join("Anytype"));
        }
    }
    roots.extend([app_data_directory(), default_install_directory(&app_data_directory())]);
    #[cfg(windows)]
    {
        for root in &roots {
            if let Some(version) = anytype_executable_version(&root.join("Anytype.exe")) {
                return format!("Anytype {version}");
            }
        }
        for variable in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = env::var_os(variable).map(PathBuf::from) {
                for executable in [
                    root.join("Programs/Anytype/Anytype.exe"),
                    root.join("Anytype/Anytype.exe"),
                ] {
                    if let Some(version) = anytype_executable_version(&executable) {
                        return format!("Anytype {version}");
                    }
                }
            }
        }
    }
    for root in roots {
        for manifest in [
            root.join("resources/app/package.json"),
            root.join("resources/app/package-lock.json"),
            root.join("package.json"),
        ] {
            let Ok(contents) = fs::read_to_string(manifest) else { continue };
            let Ok(value) = serde_json::from_str::<Value>(&contents) else { continue };
            if let Some(version) = value.get("version").and_then(Value::as_str).filter(|version| !version.trim().is_empty()) {
                return format!("Anytype {version}");
            }
        }
    }
    "Anytype unknown".into()
}

#[cfg(windows)]
fn anytype_executable_version(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let mut ignored = 0;
        let size = GetFileVersionInfoSizeW(path.as_ptr(), &mut ignored);
        if size == 0 { return None; }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 { return None; }
        let root: [u16; 2] = ['\\' as u16, 0];
        let mut value = std::ptr::null_mut();
        let mut length = 0;
        if VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), &mut value, &mut length) == 0
            || value.is_null() || length < std::mem::size_of::<VS_FIXEDFILEINFO>() as u32 {
            return None;
        }
        let info = &*(value.cast::<VS_FIXEDFILEINFO>());
        let major = info.dwFileVersionMS >> 16;
        let minor = info.dwFileVersionMS & 0xffff;
        let patch = info.dwFileVersionLS >> 16;
        let build = info.dwFileVersionLS & 0xffff;
        Some(if build == 0 { format!("{major}.{minor}.{patch}") } else { format!("{major}.{minor}.{patch}.{build}") })
    }
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
async fn get_release_notes() -> Result<Option<updater::ReleaseNotes>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        updater::get_release_notes(&app_data, &install, &runtime)
    })
    .await
    .map_err(|_| "The release notes task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn get_latest_release_notes() -> Result<Option<updater::ReleaseNotes>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        updater::get_latest_release_notes(&install, &runtime)
    })
    .await
    .map_err(|_| "The latest release notes task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn get_adjacent_release_notes(version: String, older: bool) -> Result<Option<updater::ReleaseNotes>, String> {
    tauri::async_runtime::spawn_blocking(move || updater::get_adjacent_release_notes(&version, older))
        .await
        .map_err(|_| "The release history task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn get_release_note_history_image(tag: String, source: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || updater::release_note_history_image(&tag, &source))
        .await
        .map_err(|_| "The historical release note image task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn get_release_note_image(source: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        updater::release_note_image(&runtime, &source)
    })
    .await
    .map_err(|_| "The release note image task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn get_release_note_video(source: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        updater::release_note_video(&runtime, &source)
    }).await.map_err(|_| "The release note video task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn get_release_note_history_video(tag: String, source: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || updater::release_note_history_video(&tag, &source))
        .await.map_err(|_| "The historical video task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn acknowledge_release_notes(version: String) -> Result<bool, String> {
    let guard = acquire_updater_guard()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let app_data = app_data_directory();
        let install = resolve_install_directory(&app_data)?;
        let runtime = resolve_runtime_directory_for_install(&install)?;
        updater::acknowledge_release_notes(&app_data, &install, &runtime, &version)
    })
    .await
    .map_err(|_| "The release notes acknowledgement task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn open_external_link(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        open_external_link_in_default_browser(&url)
    })
    .await
    .map_err(|_| "The external link task stopped unexpectedly.".to_string())?
}

fn open_external_link_in_default_browser(value: &str) -> Result<(), String> {
    let parsed = validate_external_link(value)?;
    #[cfg(windows)]
    {
        let operation: Vec<u16> = "open\0".encode_utf16().collect();
        let url: Vec<u16> = format!("{}\0", parsed.as_str()).encode_utf16().collect();
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
                "Could not open the link in your default browser, Windows error {result}."
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = parsed;
        Err("Opening external links is supported on Windows only.".into())
    }
}

fn validate_external_link(value: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(value.trim())
        .map_err(|_| "The external link is invalid.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || uses_local_app_host(&parsed)
    {
        return Err("Only ordinary HTTP and HTTPS links can be opened.".into());
    }
    Ok(parsed)
}

fn uses_local_app_host(url: &url::Url) -> bool {
    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https")
            && url.host_str() == Some("tauri.localhost"))
}

fn is_local_app_url(url: &url::Url) -> bool {
    uses_local_app_host(url)
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(url.path(), "/" | "/index.html")
}

fn handle_webview_navigation(url: &url::Url) -> bool {
    if is_local_app_url(url) {
        return true;
    }
    if !uses_local_app_host(url) && matches!(url.scheme(), "http" | "https") {
        let _ = open_external_link_in_default_browser(url.as_str());
    }
    false
}

fn handle_webview_new_window(url: &url::Url) {
    if !uses_local_app_host(url) && matches!(url.scheme(), "http" | "https") {
        let _ = open_external_link_in_default_browser(url.as_str());
    }
}

#[tauri::command]
async fn save_updater_settings(settings: updater::UpdaterSettingsDraft) -> Result<updater::UpdaterState, String> {
    let (save_guard, updater_guard) = acquire_save_and_updater_guards()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _save_guard = save_guard;
        let _updater_guard = updater_guard;
        let app_data = app_data_directory();
        let default_install = resolve_default_install_directory(&app_data)?;
        let previous_startup = autostart::snapshot()?;
        let startup_mode = autostart::set_mode(settings.startup_mode)?;
        match updater::save_settings(&app_data, settings, &default_install) {
            Ok(mut state) => {
                state.start_on_login = startup_mode != autostart::AutostartMode::Disabled;
                state.startup_mode = startup_mode;
                Ok(state)
            }
            Err(error) => match autostart::restore(&previous_startup) {
                Ok(_) => Err(error),
                Err(rollback) => Err(format!("{error} The previous startup setting could not be restored: {rollback}")),
            },
        }
    })
    .await
    .map_err(|_| "The updater settings task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn recover_install_directory(install_directory: String) -> Result<(), String> {
    let (save_guard, updater_guard) = acquire_save_and_updater_guards()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _save_guard = save_guard;
        let _updater_guard = updater_guard;
        updater::recover_install_directory(
            &app_data_directory(),
            Path::new(&install_directory),
        )
    })
    .await
    .map_err(|_| "The profile recovery task stopped unexpectedly.".to_string())?
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
async fn select_blob_backup_destination(
    app: tauri::AppHandle,
    current_directory: Option<String>,
) -> Result<Option<String>, String> {
    select_blob_folder(app, current_directory, "Export BlobUI Backup", "Export").await
}

#[tauri::command]
async fn select_blob_backup_folder(
    app: tauri::AppHandle,
    current_directory: Option<String>,
) -> Result<Option<String>, String> {
    select_blob_folder(app, current_directory, "Import BlobUI Backup", "Import").await
}

async fn select_blob_folder(
    app: tauri::AppHandle,
    current_directory: Option<String>,
    title: &'static str,
    button: &'static str,
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
        folder_dialog::select_folder(current_directory, title.into(), button.into(), owner)
    })
    .await
    .map_err(|_| "The folder selection dialog stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn create_blob_backup(destination_directory: String) -> Result<String, String> {
    let (save_guard, updater_guard) = acquire_save_and_updater_guards()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _save_guard = save_guard;
        let _updater_guard = updater_guard;
        let install = resolve_install_directory(&app_data_directory())?;
        let _ = settings_data_path(&install)?;
        let settings_root = settings_root_for_install(&install)?;
        kanban::prepare_backup(&install)?;
        backup::create_backup(&settings_root, Path::new(&destination_directory))
            .map(|path| path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|_| "The BlobUI backup task stopped unexpectedly.".to_string())?
}

#[tauri::command]
async fn import_blob_backup(backup_directory: String) -> Result<(), String> {
    let (save_guard, updater_guard) = acquire_save_and_updater_guards()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _save_guard = save_guard;
        let _updater_guard = updater_guard;
        let install = resolve_install_directory(&app_data_directory())?;
        let _ = settings_data_path(&install)?;
        let settings_root = settings_root_for_install(&install)?;
        backup::import_backup(&settings_root, &install, Path::new(&backup_directory))?;
        let data = settings_store::load(&settings_root)?;
        if let Some(shortcuts) = data.get("keybinds") {
            let shortcuts: Vec<hotkey::Shortcut> = serde_json::from_value(shortcuts.clone())
                .map_err(|_| "The imported keyboard shortcut settings are invalid.".to_string())?;
            let _ = hotkey::set_shortcuts(shortcuts);
        } else {
            let _ = hotkey::set_shortcuts(hotkey::default_shortcuts());
        }
        Ok(())
    })
    .await
    .map_err(|_| "The BlobUI restore task stopped unexpectedly.".to_string())?
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
    let startup_mode = autostart::get_mode()?;
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
    state.start_on_login = startup_mode != autostart::AutostartMode::Disabled;
    state.startup_mode = startup_mode;
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
    let directory = updater::resolve_app_directory(
        &install,
        Some(&executable),
        env::var_os("LOCALAPPDATA").as_deref().map(Path::new),
    )?;
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
    state.startup_mode = autostart::get_mode()?;
    state.start_on_login = state.startup_mode != autostart::AutostartMode::Disabled;
    Ok(state)
}

#[tauri::command]
async fn get_calendar_api_availability() -> String {
    tauri::async_runtime::spawn_blocking(|| {
        let Ok(install) = resolve_install_directory(&app_data_directory()) else { return "unavailable"; };
        calendar_api_availability_at(&install)
    }).await.unwrap_or("unavailable").to_string()
}

fn calendar_api_availability_at(install: &Path) -> &'static str {
    let Ok(data) = load_settings_data(install) else { return "unavailable"; };
    let Ok(credential) = api_credential_path(install) else { return "unavailable"; };
    if data.pointer("/apiConnection/enabled").and_then(Value::as_bool) == Some(false)
        || !credential.is_file() {
        return "missing";
    }
    let result = (|| {
        let key = SecretString::new(read_saved_api_key_at(install)?);
        let ip = normalize_api_ip(data.pointer("/apiConnection/ip").and_then(Value::as_str))?;
        let port = data.pointer("/apiConnection/port").and_then(Value::as_u64)
            .and_then(|port| u16::try_from(port).ok()).filter(|port| *port > 0).unwrap_or(31009);
        api::verify_connection_quick(&key, &ip, port)
    })();
    if result.is_ok() { "available" } else { "unavailable" }
}

fn configure_connection_blocking(connection: ConnectionDraft) -> Result<SaveResult, String> {
    let mut connection = normalize_connection(connection)?;
    let mut api_key = SecretString::new(std::mem::take(&mut connection.api_key));
    let install = resolve_install_directory(&app_data_directory())?;
    let (_, data_path, css_path, credential_path) = save_paths(&install)?;
    let runtime = resolve_runtime_directory_for_install(&install)?;
    let mut data = load_settings_data(&install)?;
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
        let same_endpoint = data.pointer("/apiConnection/ip").and_then(Value::as_str).unwrap_or(DEFAULT_API_IP) == ip
            && data.pointer("/apiConnection/port").and_then(Value::as_u64).unwrap_or(31009) == u64::from(port);
        // Changing the connection invalidates cached colors even while coloring is off.
        if !connection.keep_existing_key || !same_endpoint {
            data["calendarObjectColors"] = json!({});
            data["calendarViewColors"] = json!([]);
            data["calendarColorInfo"] = json!([]);
        }
        let connection_result = (|| {
            api::verify_connection(&api_key, ip, port)?;
            if !connection.keep_existing_key {
                protected_key = Some(connection::encrypt_api_key(&api_key)?);
            }
            Ok::<_, String>(if current_settings.calendar_colors_enabled {
                calendar::snapshot(&api_key, ip, port, &calendar::configuration(&data)?).map(Some)
            } else { Ok(None) })
        })();
        let colors_result = connection_result?;
        match colors_result {
            Ok(Some(colors)) => {
                data["calendarObjectColors"] = colors["base"].clone();
                data["calendarViewColors"] = colors["overrides"].clone();
                data["calendarColorInfo"] = colors["info"].clone();
                calendar_status = "refreshed";
            }
            Ok(None) => { calendar_status = "off"; }
            Err(_) => {
                calendar_status = "cached";
            }
        }
    } else {
        data["calendarObjectColors"] = json!({});
        data["calendarViewColors"] = json!([]);
        calendar_status = "disabled";
    }

    data["apiConnection"] = json!({
        "metadataRevision": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos().to_string(),
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
    anyblob::invalidate();
    commit_settings_with_store(
        &install,
        &css_path,
        &data_path,
        &credential_path,
        css.as_bytes(),
        &serialized,
        credential,
    )?;
    Ok(SaveResult { calendar_status: calendar_status.into(), info: data["calendarColorInfo"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect() })
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

const BACKGROUND_TRAY_ID: &str = "blobui-background";

fn deliver_pending_onboarding(app: &tauri::AppHandle) {
    if SETTINGS_VISIBLE.load(Ordering::Acquire)
        && ONBOARDING_READY.load(Ordering::Acquire)
        && SHOW_ONBOARDING.swap(false, Ordering::AcqRel)
    {
        if app.emit("blobui-show-onboarding", ()).is_err() {
            SHOW_ONBOARDING.store(true, Ordering::Release);
        }
    }
}

#[tauri::command]
fn onboarding_ready(app: tauri::AppHandle) {
    ONBOARDING_READY.store(true, Ordering::Release);
    deliver_pending_onboarding(&app);
    if let Some(error) = BACKGROUND_ERROR.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = app.emit("blobui-background-error", error);
    }
}

fn show_settings_window(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || show_settings_window_on_main(&handle));
}

fn show_settings_window_on_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.show().is_err() { return; }
        SETTINGS_VISIBLE.store(true, Ordering::Release);
        let _ = window.unminimize();
        let _ = window.set_focus();
        app.remove_tray_by_id(BACKGROUND_TRAY_ID);
        deliver_pending_onboarding(app);
    }
}

fn enter_background(app: &tauri::AppHandle) -> Result<(), String> {
    let handle = app.clone();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || { let _ = sender.send(enter_background_on_main(&handle)); })
        .map_err(|e| e.to_string())?;
    receiver.recv().map_err(|e| e.to_string())?
}

fn enter_background_on_main(app: &tauri::AppHandle) -> Result<(), String> {
    let result = (|| -> Result<(), String> {
        if app.tray_by_id(BACKGROUND_TRAY_ID).is_none() {
            let open = tauri::menu::MenuItem::with_id(app, "open-settings", "Open Settings", true, None::<&str>).map_err(|e| e.to_string())?;
            let quit = tauri::menu::MenuItem::with_id(app, "quit-blobui", "Quit BlobUI", true, None::<&str>).map_err(|e| e.to_string())?;
            let menu = tauri::menu::Menu::with_items(app, &[&open, &quit]).map_err(|e| e.to_string())?;
            let icon = app.default_window_icon().cloned().ok_or("The BlobUI tray icon is missing.")?;
            tauri::tray::TrayIconBuilder::with_id(BACKGROUND_TRAY_ID)
                .icon(icon).tooltip("BlobUI Settings").menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open-settings" => show_settings_window(app),
                    "quit-blobui" => if let Err(error) = quit_app(app.clone()) {
                        show_settings_window(app);
                        let _ = app.emit("blobui-background-error", error);
                    },
                    _ => {}
                })
                .build(app).map_err(|e| e.to_string())?;
        }
        app.get_webview_window("main").ok_or("The Settings window is unavailable.")?
            .hide().map_err(|e| e.to_string())?;
        SETTINGS_VISIBLE.store(false, Ordering::Release);
        Ok(())
    })();
    if let Err(error) = &result {
        show_settings_window(app);
        eprintln!("Could not run BlobUI in background: {error}");
        if ONBOARDING_READY.load(Ordering::Acquire) {
            let _ = app.emit("blobui-background-error", error);
        } else {
            *BACKGROUND_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = Some(error.clone());
        }
    }
    result
}

#[tauri::command]
fn run_in_background(app: tauri::AppHandle) -> Result<(), String> {
    enter_background(&app)
}

#[tauri::command]
fn minimize_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
fn open_component_catalog(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("component-catalog") {
        window.show().map_err(|error| error.to_string())?;
        window.unminimize().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }
    let url = tauri::WebviewUrl::App("index.html?window=component-catalog".into());
    let window = tauri::WebviewWindowBuilder::new(&app, "component-catalog", url)
        .title("BlobUI component catalog")
        .inner_size(1120.0, 820.0)
        .min_inner_size(880.0, 640.0)
        .center()
        .resizable(true)
        .decorations(false)
        .visible(false)
        .on_navigation(handle_webview_navigation)
        .on_new_window(|url, _features| {
            handle_webview_new_window(&url);
            tauri::webview::NewWindowResponse::Deny
        })
        .build()
        .map_err(|error| format!("Could not open the component catalog: {error}"))?;
    let theme = read_anytype_theme(&app_data_directory().join("devconfig.json"));
    window.set_theme(native_theme(&theme)).map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) -> Result<(), String> {
    enter_background(&app)
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
    open_github_link("https://github.com/PainterFlow/Anytype-BlobUI")
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
    let data_path = settings_data_path(&install_directory)?;
    let theme = read_anytype_theme(&install_directory.join("devconfig.json"));

    let mut onboarding_complete = false;
    let mut api_port = 31009;
    let mut api_ip = String::new();
    let mut has_saved_api_key = false;
    let (ready, settings, message) = if data_path.is_file() {
        let data = load_settings_data(&install_directory)?;
        let api_connection = data.pointer("/apiConnection");
        let credential_path = api_credential_path(&install_directory)?;
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
        onboarding_complete,
        api_port,
        api_ip,
        has_saved_api_key,
    })
}

fn save_settings_blocking(settings: SettingsDraft) -> Result<SaveResult, String> {
    save_settings_with_calendar_rules(settings, None)
}

fn save_settings_with_calendar_rules(settings: SettingsDraft, rules: Option<calendar::Configuration>) -> Result<SaveResult, String> {
    let settings = validate_settings(settings)?;
    let install = resolve_install_directory(&app_data_directory())?;
    let (_, data_path, css_path, credential_path) = save_paths(&install)?;
    let runtime = resolve_runtime_directory_for_install(&install)?;
    let mut data = load_settings_data(&install)?;
    if !data.is_object() {
        return Err("BlobUI data.json must contain a JSON object.".into());
    }
    apply_settings(&mut data, &install, &settings);

    if let Some(rules) = &rules { data["calendarColorRules"] = json!(rules); }
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
        data["calendarViewColors"] = json!([]);
        calendar_status = "disabled";
    } else if !settings.calendar_colors_enabled {
        calendar_status = "off";
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
            if let Ok(colors) = calendar::snapshot(&api_key, &ip, port, &calendar::configuration(&data)?) {
                data["calendarObjectColors"] = colors["base"].clone();
                data["calendarViewColors"] = colors["overrides"].clone();
                data["calendarColorInfo"] = colors["info"].clone();
                calendar_status = "refreshed";
            }
        }
    }

    if rules.is_some() && calendar_status == "cached" { return Err("Calendar colors could not be refreshed. Your rules and previous snapshot are unchanged.".into()); }
    let css = css::compile(&runtime, &install, &data)?;
    let serialized = serialize_data(&data)?;
    anyblob::invalidate();
    commit_settings_with_store(
        &install,
        &css_path,
        &data_path,
        &credential_path,
        css.as_bytes(),
        &serialized,
        storage::CredentialChange::Keep,
    )?;
    Ok(SaveResult { calendar_status: calendar_status.into(), info: data["calendarColorInfo"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect() })
}

#[cfg(test)]
fn assert_fixture_path(path: &Path) -> Result<(), String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixtures = workspace.join(".test-run").canonicalize().map_err(|e| e.to_string())?;
    let resolved = path.canonicalize().map_err(|e| e.to_string())?;
    if !resolved.starts_with(&fixtures) {
        return Err("Native tests refuse to access profiles or credentials outside workspace .test-run fixtures.".into());
    }
    reject_reparse_component(path)?;
    Ok(())
}

fn save_paths(install: &Path) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf), String> {
    #[cfg(test)]
    assert_fixture_path(install)?;
    ensure_not_release_path(install)?;
    let blob = install.join("blob");
    let data = blob.join("data.json");
    let css = install.join("custom.css");
    let credential = api_credential_path(install)?;
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

pub(crate) fn settings_root_for_install(install: &Path) -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let _ = install;
        return Ok(install.to_path_buf());
    }
    #[cfg(not(test))]
    {
        let expected_install = updater::validate_install_directory(install)?;
        let located = updater::locate_app_install(
            env::current_exe().ok().as_deref(),
            env::var_os("LOCALAPPDATA").as_deref().map(|value| Path::new(value)),
        );
        match located {
            Ok((app, resolved_install)) => {
                let matches = {
                    #[cfg(windows)]
                    { resolved_install.to_string_lossy().eq_ignore_ascii_case(&expected_install.to_string_lossy()) }
                    #[cfg(not(windows))]
                    { resolved_install == expected_install }
                };
                if !matches {
                    return Err("The installed BlobUI app belongs to a different Anytype folder.".into());
                }
                Ok(app)
            }
            Err(error) => {
                if error != "The installed BlobUI app folder could not be found. Run the installer once before using Settings." {
                    return Err(error);
                }
                let Some(app) = workspace_app_root_from_executable(env::current_exe().ok().as_deref()) else {
                    return Err(error);
                };
                reject_reparse_component(&app)?;
                Ok(app)
            }
        }
    }
}

fn workspace_app_root_from_executable(current_exe: Option<&Path>) -> Option<PathBuf> {
    let executable = current_exe?;
    if !executable
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("blobui-settings.exe"))
    {
        return None;
    }
    let dist = executable.parent()?;
    let project = dist.parent()?;
    let workspace = project.parent()?;
    if !dist.file_name().is_some_and(|name| name.eq_ignore_ascii_case("dist"))
        || !project.file_name().is_some_and(|name| name.eq_ignore_ascii_case("settings-app"))
        || !project.join("Cargo.toml").is_file()
        || !workspace.join("blobui-release.json").is_file()
        || !workspace.join("Dev").is_dir()
        || !executable.is_file()
    {
        return None;
    }
    dist.canonicalize().ok()
}

pub(crate) fn settings_data_path(install: &Path) -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        return Ok(install.join("blob/data.json"));
    }
    #[cfg(not(test))]
    {
        let root = settings_root_for_install(install)?;
        let legacy_blob = install.join("blob");
        let migrated = ensure_settings_store_on_launch(&root, &legacy_blob)?;
        if migrated {
            let app_data = app_data_directory();
            if let Ok(preferences) = updater::load_preferences(&updater::preferences_path(&app_data)) {
                let preferences = json!({
                    "installDirectory": preferences.install_directory,
                    "autoplayVideos": preferences.autoplay_videos,
                    "automaticUpdatesEnabled": preferences.automatic_updates_enabled,
                    "automaticChecksEnabled": preferences.automatic_checks_enabled,
                });
                settings_store::save_updater_preferences(&root, &preferences)?;
            }
            let shortcuts_path = app_data.parent().unwrap_or(&app_data).join("BlobUI/shortcuts.json");
            if let Ok(bytes) = fs::read(&shortcuts_path) {
                if let Ok(shortcuts) = parse_settings_json(&bytes) {
                    let mut data = settings_store::load(&root)?;
                    data["keybinds"] = shortcuts;
                    settings_store::save(&root, &data)?;
                }
            }
        }
        let settings_directory = root.join("Settings");
        if !settings_directory.is_dir() {
            return Ok(legacy_blob.join("data.json"));
        }
        Ok(settings_directory.join("data.json"))
    }
}

fn ensure_settings_store_on_launch(root: &Path, legacy_blob: &Path) -> Result<bool, String> {
    reject_reparse_component(root)?;
    let settings_directory = root.join("Settings");
    match fs::symlink_metadata(&settings_directory) {
        Ok(metadata) => {
            reject_reparse_component(&settings_directory)?;
            if !metadata.is_dir() {
                return Err("The BlobUI Settings path is not a regular folder.".into());
            }
            return Ok(false);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Could not inspect the Settings folder: {error}")),
    }

    let legacy_data = legacy_blob.join("data.json");
    reject_reparse_component(legacy_blob)?;
    match fs::symlink_metadata(&legacy_data) {
        Ok(metadata)
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && !is_windows_reparse_point(&metadata) =>
        {
            settings_store::migrate_legacy_blob(legacy_blob, root)?;
            Ok(true)
        }
        Ok(_) => Err("The legacy BlobUI settings file is not a supported regular file.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("Could not inspect legacy BlobUI settings: {error}")),
    }
}

pub(crate) fn load_settings_data(install: &Path) -> Result<Value, String> {
    #[cfg(test)]
    {
        return read_json_file(&install.join("blob/data.json"));
    }
    #[cfg(not(test))]
    {
        let _ = settings_data_path(install)?;
        let root = settings_root_for_install(install)?;
        settings_store::load(&root)
    }
}

fn normalize_shortcuts_on_launch(install: &Path) -> Option<Vec<hotkey::Shortcut>> {
    let mut data = load_settings_data(install).ok()?;
    let saved = data.get("keybinds")?.clone();
    let normalized_value = hotkey::normalize_shortcut_value(&saved).ok()?;
    let normalized: Vec<hotkey::Shortcut> = serde_json::from_value(normalized_value.clone()).ok()?;
    if saved != normalized_value {
        data["keybinds"] = normalized_value;
        match settings_root_for_install(install).and_then(|root| settings_store::save(&root, &data)) {
            Ok(()) => {}
            Err(error) => eprintln!("Could not save the keyboard shortcut migration: {error}"),
        }
    }
    Some(normalized)
}

pub(crate) fn persist_split_settings(install: &Path, data: &Value) -> Result<(), String> {
    #[cfg(test)]
    {
        let _ = (install, data);
        return Ok(());
    }
    #[cfg(not(test))]
    {
        let _ = settings_data_path(install)?;
        let root = settings_root_for_install(install)?;
        settings_store::save(&root, data)
    }
}

pub(crate) fn persist_split_settings_with_formula(
    install: &Path,
    data: &Value,
    formula_configuration: &Value,
) -> Result<(), String> {
    #[cfg(test)]
    {
        let _ = (install, data, formula_configuration);
        return Ok(());
    }
    #[cfg(not(test))]
    {
        let _ = settings_data_path(install)?;
        let root = settings_root_for_install(install)?;
        settings_store::save_with_formula(&root, data, formula_configuration)
    }
}

pub(crate) fn api_credential_path(install: &Path) -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        return Ok(install.join("blob/credentials/anytype-api-key.dpapi"));
    }
    #[cfg(not(test))]
    {
        let root = settings_root_for_install(install)?;
        Ok(root.join("Settings/DO-NOT-SHARE/anytype-api-key.dpapi"))
    }
}

fn commit_settings_with_store(
    install: &Path,
    css_path: &Path,
    data_path: &Path,
    credential_path: &Path,
    css: &[u8],
    data_bytes: &[u8],
    credential: storage::CredentialChange<'_>,
) -> Result<(), String> {
    #[cfg(test)]
    {
        let _ = install;
        return storage::commit_settings(css_path, data_path, credential_path, css, data_bytes, credential);
    }
    #[cfg(not(test))]
    {
        let metadata_path = data_path
            .parent()
            .ok_or("The BlobUI settings file has no parent folder.")?
            .join("blob.meta.json");
        let before = [
            storage::snapshot(css_path, false)?,
            storage::snapshot(data_path, false)?,
            storage::snapshot(&metadata_path, false)?,
            storage::snapshot(credential_path, true)?,
        ];
        storage::commit_settings(css_path, data_path, credential_path, css, data_bytes, credential)?;
        let root = settings_root_for_install(install)?;
        let aggregate = parse_settings_json(data_bytes)
            .map_err(|_| "BlobUI data.json is not valid JSON.".to_string())?;
        if let Err(error) = settings_store::save(&root, &aggregate) {
            let mut rollback_errors = Vec::new();
            for snapshot in before.iter().rev() {
                if let Err(rollback) = storage::restore(snapshot) {
                    rollback_errors.push(rollback);
                }
            }
            if rollback_errors.is_empty() {
                return Err(error);
            }
            return Err(format!("{error} The compatibility files could not all be restored: {}", rollback_errors.join("; ")));
        }
        Ok(())
    }
}

fn load_settings_data_for_path(path: &Path) -> Result<Value, String> {
    #[cfg(not(test))]
    if path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("data.json"))
        && path.parent().and_then(Path::file_name).is_some_and(|name| name.eq_ignore_ascii_case("blob"))
    {
        if let Some(install) = path.parent().and_then(Path::parent) {
            return load_settings_data(install);
        }
    }
    read_json_file_raw(path)
}

fn apply_settings(data: &mut Value, install: &Path, settings: &SettingsDraft) {
    data["installDirectory"] = Value::String(install.to_string_lossy().into_owned());
    let accent = if settings.accent_mode == "native" {
        "var(--color-system-accent-100)"
    } else {
        settings.accent_color.as_str()
    };
    let mut values = json!({
        "fontFamily": settings.font_family,
        "codeFontFamily": settings.code_font_family,
        "accentColor": accent,
        "includeSelections": settings.include_selections,
        "bannerImage": settings.banner_image,
        "reducedMotion": settings.reduced_motion,
        "tabHoverEffect": settings.tab_hover_effect,
        "linkHoverEffect": settings.link_hover_effect,
        "objectMentionHoverEffect": settings.object_mention_hover_effect,
        "kanbanViewStyle": settings.kanban_view_style,
        "collapsedCategories": settings.collapsed_categories,
        "codexIntegration": settings.codex_integration,
        "calendarColorsEnabled": settings.calendar_colors_enabled,
        "sidebarResizeMode": settings.sidebar_resize_mode,
        "backgroundExperiment": settings.background_experiment,
    });
    let mut codex_values = json!(settings.codex_integration);
    preserve_unknown_object_fields(
        data.pointer("/settings/codexIntegration"),
        &mut codex_values,
    );
    preserve_unknown_object_fields(
        data.pointer("/settingsV2/settings/codexIntegration"),
        &mut codex_values,
    );
    if let Some(values) = values.as_object_mut() {
        values.insert("codexIntegration".into(), codex_values.clone());
    }
    if !data.get("settings").is_some_and(Value::is_object) {
        data["settings"] = json!({});
    }
    if let (Some(target), Some(values)) = (data["settings"].as_object_mut(), values.as_object()) {
        for (name, value) in values {
            target.insert(name.clone(), value.clone());
        }
    }
    let mut v2_values = json!({
            "fontFamily": settings.font_family,
            "codeFontFamily": settings.code_font_family,
            "accentColor": accent,
            "includeSelections": settings.include_selections,
            "accentMode": settings.accent_mode,
            "accentPreset": settings.accent_preset,
            "bannerImage": settings.banner_image,
            "reducedMotion": settings.reduced_motion,
            "tabHoverEffect": settings.tab_hover_effect,
            "linkHoverEffect": settings.link_hover_effect,
            "objectMentionHoverEffect": settings.object_mention_hover_effect,
            "kanbanViewStyle": settings.kanban_view_style,
            "collapsedCategories": settings.collapsed_categories,
            "codexIntegration": settings.codex_integration,
        "calendarColorsEnabled": settings.calendar_colors_enabled,
        "sidebarResizeMode": settings.sidebar_resize_mode,
        "backgroundExperiment": settings.background_experiment,
    });
    if let Some(values) = v2_values.as_object_mut() {
        values.insert("codexIntegration".into(), codex_values);
    }
    let mut settings_v2 = data
        .get("settingsV2")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    match settings_v2.get("version").and_then(Value::as_u64) {
        Some(version) if version > 2 => {}
        Some(_) | None => {
            settings_v2.insert("version".into(), json!(2));
        }
    }
    let mut v2_settings = settings_v2
        .get("settings")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(values) = v2_values.as_object() {
        for (name, value) in values {
            v2_settings.insert(name.clone(), value.clone());
        }
    }
    settings_v2.insert("settings".into(), Value::Object(v2_settings));
    data["settingsV2"] = Value::Object(settings_v2);
}

fn preserve_unknown_object_fields(original: Option<&Value>, current: &mut Value) {
    let (Some(original), Some(current)) = (original.and_then(Value::as_object), current.as_object_mut()) else {
        return;
    };
    for (key, original_value) in original {
        match current.get_mut(key) {
            Some(current_value) => preserve_unknown_object_fields(Some(original_value), current_value),
            None => {
                current.insert(key.clone(), original_value.clone());
            }
        }
    }
}

fn serialize_data(data: &Value) -> Result<Vec<u8>, String> {
    let mut serialized = serde_json::to_vec_pretty(data)
        .map_err(|error| format!("Could not encode BlobUI settings: {error}"))?;
    serialized.push(b'\n');
    Ok(serialized)
}

fn read_saved_api_key_at(install: &Path) -> Result<String, String> {
    #[cfg(test)]
    assert_fixture_path(install)?;
    let credential = api_credential_path(install)?;
    reject_reparse_component(&credential)?;
    let encrypted = fs::read_to_string(&credential)
        .map_err(|_| "Could not read the saved API key. Enter a new key in Advanced settings.".to_string())?;
    connection::decrypt_saved_key(encrypted.trim())
}

fn validate_settings(mut settings: SettingsDraft) -> Result<SettingsDraft, String> {
    if !matches!(settings.kanban_view_style.as_str(), "default" | "balanced" | "compact") {
        return Err("Kanban view style must be Default, Balanced, or Compact.".into());
    }
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

    let kanban_view_style = read_string(local, "kanbanViewStyle")
        .or_else(|| read_string(Some(normalized), "kanbanViewStyle"))
        .filter(|value| matches!(value.as_str(), "default" | "balanced" | "compact"))
        .unwrap_or_else(default_kanban_style);
    let collapsed_categories = read_bool(local, "collapsedCategories")
        .or_else(|| read_bool(Some(normalized), "collapsedCategories"))
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
        codex_integration: codex::normalize(local.and_then(|v| v.get("codexIntegration")).or_else(|| normalized.get("codexIntegration")).unwrap_or(&Value::Null)),
        font_family,
        code_font_family,
        accent_color,
        accent_mode,
        accent_preset,
        include_selections: read_bool(local, "includeSelections")
            .or_else(|| read_bool(Some(normalized), "includeSelections"))
            .unwrap_or(true),
        banner_image,
        reduced_motion,
        tab_hover_effect,
        link_hover_effect,
        object_mention_hover_effect,
        kanban_view_style,
        collapsed_categories,
        background_experiment: match read_string(local, "backgroundExperiment").or_else(|| read_string(Some(normalized), "backgroundExperiment")).as_deref() {
            Some("normal") => BackgroundExperiment::Normal,
            Some("autostart-background") => BackgroundExperiment::AutostartBackground,
            _ => BackgroundExperiment::Disabled,
        },
        sidebar_resize_mode: match read_string(local, "sidebarResizeMode").or_else(|| read_string(Some(normalized), "sidebarResizeMode")).as_deref() {
            Some("standard") => SidebarResizeMode::Standard,
            Some("pro") => SidebarResizeMode::Pro,
            _ => SidebarResizeMode::Off,
        },
        calendar_colors_enabled: read_bool(local, "calendarColorsEnabled")
            .or_else(|| read_bool(Some(normalized), "calendarColorsEnabled"))
            .unwrap_or(true),
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
    load_settings_data_for_path(path)
}

fn read_json_file_raw(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "Could not read BlobUI settings at {}: {error}",
            path.display()
        )
    })?;
    parse_settings_json(&bytes).map_err(|_| "BlobUI data.json is not valid JSON.".into())
}

fn parse_settings_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
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
    #[cfg(test)]
    assert_fixture_path(app_data.parent().unwrap_or(app_data))?;
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
        .is_some_and(|root| {
            !is_installed_runtime_directory(root) && !is_installed_app_directory(root)
        })
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

fn is_installed_app_directory(path: &Path) -> bool {
    path.join(".blobui-install.json").is_file()
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

fn consume_pending_launch_request(name: &str) -> bool {
    let Ok(executable) = env::current_exe() else {
        return false;
    };
    let Some(directory) = executable.parent() else {
        return false;
    };
    consume_pending_launch_request_in(directory, name)
}

fn consume_pending_launch_request_in(directory: &Path, name: &str) -> bool {
    match fs::remove_file(directory.join(name)) {
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
            let pending_onboarding = consume_pending_launch_request(".blobui-onboarding-next");
            if onboarding_argument || pending_onboarding {
                SHOW_ONBOARDING.store(true, Ordering::Release);
            }
            let pending_background = consume_pending_launch_request(".blobui-background-next");
            if !pending_background && !arguments.iter().any(|argument| argument == "--background") {
                show_settings_window(app);
            }
        }))
        .invoke_handler(tauri::generate_handler![
            codex::discover_codex_spaces,
            codex::discover_anyblob_types,
            codex::discover_codex_sources,
            codex_projects::discover_codex_projects,
            anyblob::anyblob_status,
            marketplace::anyblob_installation_status,
            marketplace::install_anyblob_marketplace,
            anyblob::configure_anyblob,
            calendar::load_calendar_rules,
            calendar::discover_calendar_colors,
            calendar::save_calendar_rules,
            kanban::load_kanban_formulas,
            kanban::discover_kanban_views,
            kanban::evaluate_kanban_formula,
            kanban::save_kanban_formulas,
            kanban::refresh_kanban_results,
            reveal_api_key,
            get_calendar_api_availability,
            configure_connection,
            get_app_state,
            get_saved_state_snapshot,
            save_settings,
            get_updater_state,
            get_release_notes,
            get_latest_release_notes,
            get_adjacent_release_notes,
            get_release_note_history_image,
            get_anytype_version,
            get_release_note_image,
            get_release_note_video,
            get_release_note_history_video,
            acknowledge_release_notes,
            open_external_link,
            save_updater_settings,
            recover_install_directory,
            select_anytype_folder,
            select_blob_backup_destination,
            select_blob_backup_folder,
            create_blob_backup,
            import_blob_backup,
            check_for_updates,
            apply_update,
            simulate_update,
            uninstall_blobui,
            minimize_window,
            open_component_catalog,
            hide_window,
            run_in_background,
            onboarding_ready,
            start_window_drag,
            open_project_repository,
            open_author_profile,
            get_keyboard_shortcuts,
            save_keyboard_shortcuts,
            set_shortcut_capture,
            quit_app
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "component-catalog" {
                    return;
                }
                api.prevent_close();
                let _ = enter_background(window.app_handle());
            }
        })
        .setup(|app| {
            let main_window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|config| config.label == "main")
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "The main Settings window is not configured.",
                    )
                })?;
            let main_window = tauri::WebviewWindowBuilder::from_config(app, main_window_config)?
                .on_navigation(handle_webview_navigation)
                .on_new_window(|url, _features| {
                    handle_webview_new_window(&url);
                    tauri::webview::NewWindowResponse::Deny
                })
                .build()?;
            let theme = read_anytype_theme(&app_data_directory().join("devconfig.json"));
            main_window.set_theme(native_theme(&theme))?;
            if consume_pending_launch_request(".blobui-onboarding-next") {
                SHOW_ONBOARDING.store(true, Ordering::Release);
            }
            let app_data = app_data_directory();
            if let Ok(install_directory) = resolve_install_directory(&app_data) {
                let _ = std::thread::Builder::new()
                    .name("blobui-staging-cleanup".into())
                    .spawn(move || updater::cleanup_staged_installers(&install_directory));
            }
            // Resolve the store before starting the shortcut service. This call
            // migrates an existing Anytype blob into app-root Settings on the
            // first launch after an update.
            let launch_storage = resolve_install_directory(&app_data)
                .ok()
                .and_then(|install| settings_data_path(&install).ok().map(|path| (install, path)));
            let stored_shortcuts = launch_storage
                .as_ref()
                .and_then(|(install, _)| normalize_shortcuts_on_launch(install));
            if let Some(shortcuts) = stored_shortcuts {
                hotkey::start_with_saved(app.handle().clone(), Some(shortcuts));
            } else if launch_storage
                .as_ref()
                .is_some_and(|(_, path)| path.is_file())
            {
                // A valid Settings store can intentionally have no saved keybinds.
                hotkey::start_with_saved(app.handle().clone(), None);
            } else {
                let shortcuts = app_data.parent().unwrap_or(&app_data).join("BlobUI").join("shortcuts.json");
                hotkey::start(app.handle().clone(), &shortcuts);
            }
            anyblob::start(app.handle().clone());
            let pending_background = consume_pending_launch_request(".blobui-background-next");
            if !pending_background && !env::args().any(|argument| argument == "--background") {
                show_settings_window(app.handle());
            } else {
                let _ = enter_background(app.handle());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running BlobUI Settings");
}

impl Default for SettingsDraft {
    fn default() -> Self {
        Self {
            codex_integration: codex::Configuration::default(),
            font_family: DEFAULT_FONT.into(),
            code_font_family: DEFAULT_CODE_FONT.into(),
            accent_color: DEFAULT_ACCENT.into(),
            accent_mode: "preset".into(),
            accent_preset: "blob-pink".into(),
            include_selections: true,
            banner_image: String::new(),
            reduced_motion: false,
            tab_hover_effect: false,
            link_hover_effect: false,
            object_mention_hover_effect: false,
            kanban_view_style: default_kanban_style(),
            collapsed_categories: false,
            calendar_colors_enabled: true,
            sidebar_resize_mode: SidebarResizeMode::Off,
            background_experiment: BackgroundExperiment::Disabled,
        }
    }
}

#[cfg(test)]
mod save_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_migrates_legacy_settings_once_and_preserves_the_source() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let root = workspace.join(".test-run").join(format!(
            "launch-settings-migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace_root = root.join("checkout");
        let settings_project = workspace_root.join("settings-app");
        let app_root = settings_project.join("dist");
        let legacy_blob = root.join("Anytype/blob");
        fs::create_dir_all(&app_root).unwrap();
        fs::create_dir_all(workspace_root.join("Dev")).unwrap();
        fs::write(settings_project.join("Cargo.toml"), b"[package]\n").unwrap();
        fs::write(workspace_root.join("blobui-release.json"), b"{}\n").unwrap();
        let executable = app_root.join("blobui-settings.exe");
        fs::write(&executable, b"fixture executable").unwrap();
        assert_eq!(
            workspace_app_root_from_executable(Some(&executable)).unwrap(),
            app_root.canonicalize().unwrap()
        );
        assert!(workspace_app_root_from_executable(Some(&root.join("other/blobui-settings.exe"))).is_none());

        fs::create_dir_all(legacy_blob.join("assets")).unwrap();
        let legacy_data = json!({
            "installDirectory": root.join("Anytype").to_string_lossy(),
            "futureField": {"preserve": true},
            "settings": {"fontFamily": "Inter"}
        });
        let legacy_bytes = serde_json::to_vec_pretty(&legacy_data).unwrap();
        fs::write(legacy_blob.join("data.json"), &legacy_bytes).unwrap();
        fs::write(legacy_blob.join("assets/banner.png"), [0x89, b'P', b'N', b'G']).unwrap();

        assert!(ensure_settings_store_on_launch(&app_root, &legacy_blob).unwrap());
        let migrated = settings_store::load(&app_root).unwrap();
        assert_eq!(migrated["settings"]["fontFamily"], "Inter");
        assert_eq!(migrated["futureField"]["preserve"], true);
        assert_eq!(
            fs::read(app_root.join("Settings/assets/banner.png")).unwrap(),
            [0x89, b'P', b'N', b'G']
        );

        let mut changed = migrated.clone();
        changed["settings"]["fontFamily"] = json!("Serif");
        settings_store::save(&app_root, &changed).unwrap();
        assert!(!ensure_settings_store_on_launch(&app_root, &legacy_blob).unwrap());
        assert_eq!(
            settings_store::load(&app_root).unwrap()["settings"]["fontFamily"],
            "Serif"
        );
        assert_eq!(fs::read(legacy_blob.join("data.json")).unwrap(), legacy_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_launch_requests_are_independent_and_consumed_once() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(".test-run").join(format!(
            "pending-launch-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        for name in [".blobui-background-next", ".blobui-onboarding-next"] {
            fs::write(directory.join(name), "").unwrap();
        }
        assert!(consume_pending_launch_request_in(&directory, ".blobui-background-next"));
        assert!(!consume_pending_launch_request_in(&directory, ".blobui-background-next"));
        assert!(consume_pending_launch_request_in(&directory, ".blobui-onboarding-next"));
        assert!(!consume_pending_launch_request_in(&directory, ".blobui-onboarding-next"));
        // A directory is an invalid request and must not be removed as a file.
        fs::create_dir(directory.join(".blobui-background-next")).unwrap();
        assert!(!consume_pending_launch_request_in(&directory, ".blobui-background-next"));
        assert!(directory.join(".blobui-background-next").is_dir());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn background_experiment_defaults_and_persists_all_modes() {
        assert_eq!(SettingsDraft::default().background_experiment, BackgroundExperiment::Disabled);
        let mut legacy = serde_json::to_value(SettingsDraft::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("backgroundExperiment");
        assert_eq!(serde_json::from_value::<SettingsDraft>(legacy).unwrap().background_experiment, BackgroundExperiment::Disabled);
        assert_eq!(settings_from_data(&json!({"settings": {}})).background_experiment, BackgroundExperiment::Disabled);
        for (name, mode) in [("disabled", BackgroundExperiment::Disabled), ("normal", BackgroundExperiment::Normal), ("autostart-background", BackgroundExperiment::AutostartBackground)] {
            let draft = SettingsDraft { background_experiment: mode, ..SettingsDraft::default() };
            let mut data = json!({});
            apply_settings(&mut data, Path::new("fixture"), &draft);
            let saved = serialize_data(&data).unwrap();
            let saved: Value = serde_json::from_slice(&saved).unwrap();
            assert_eq!(saved["settings"]["backgroundExperiment"], name);
            assert_eq!(saved["settingsV2"]["settings"]["backgroundExperiment"], name);
            assert_eq!(settings_from_data(&saved).background_experiment, mode);
            assert_eq!(serde_json::from_value::<SettingsDraft>(serde_json::to_value(&draft).unwrap()).unwrap().background_experiment, mode);
            assert_eq!(settings_from_data(&json!({"settings": {"backgroundExperiment": name}})).background_experiment, mode);
        }
        assert_eq!(settings_from_data(&json!({"settings": {"backgroundExperiment": "normal"}, "settingsV2": {"settings": {"backgroundExperiment": "autostart-background"}}})).background_experiment, BackgroundExperiment::AutostartBackground);
        for invalid in [json!("unknown"), json!(42), Value::Null] {
            assert_eq!(settings_from_data(&json!({"settings": {"backgroundExperiment": invalid.clone()}})).background_experiment, BackgroundExperiment::Disabled);
            assert_eq!(settings_from_data(&json!({"settingsV2": {"settings": {"backgroundExperiment": invalid.clone()}}})).background_experiment, BackgroundExperiment::Disabled);
            let mut draft = serde_json::to_value(SettingsDraft::default()).unwrap();
            draft["backgroundExperiment"] = invalid;
            assert!(serde_json::from_value::<SettingsDraft>(draft).is_err());
        }
    }

    #[test]
    fn native_tests_reject_real_profile_paths_before_credential_access() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(assert_fixture_path(workspace).is_err());
        assert!(assert_fixture_path(&workspace.join(".test-run")).is_ok());
        assert!(save_paths(workspace).unwrap_err().contains("outside workspace"));
        assert!(read_saved_api_key_at(workspace).unwrap_err().contains("outside workspace"));
    }

    #[test]
    fn sidebar_resize_mode_defaults_and_round_trip() {
        assert_eq!(SettingsDraft::default().sidebar_resize_mode, SidebarResizeMode::Off);
        for (name, mode) in [("off", SidebarResizeMode::Off), ("standard", SidebarResizeMode::Standard), ("pro", SidebarResizeMode::Pro)] {
            let settings = SettingsDraft { sidebar_resize_mode: mode, ..SettingsDraft::default() };
            let mut data = json!({});
            apply_settings(&mut data, Path::new("fixture"), &settings);
            assert_eq!(settings_from_data(&data).sidebar_resize_mode, mode);
            assert_eq!(data["settings"]["sidebarResizeMode"], name);
            assert_eq!(data["settingsV2"]["settings"]["sidebarResizeMode"], name);
        }
        for invalid in [json!("invalid"), json!(42), Value::Null, json!({})] {
            let mut draft = serde_json::to_value(SettingsDraft::default()).unwrap();
            draft["sidebarResizeMode"] = invalid.clone();
            assert_eq!(serde_json::from_value::<SettingsDraft>(draft).unwrap().sidebar_resize_mode, SidebarResizeMode::Off);
            assert_eq!(settings_from_data(&json!({"settings": {"sidebarResizeMode": invalid}})).sidebar_resize_mode, SidebarResizeMode::Off);
        }
        assert_eq!(settings_from_data(&json!({"settings": {}})).sidebar_resize_mode, SidebarResizeMode::Off);
    }

    #[test]
    fn calendar_settings_default_and_round_trip() {
        assert!(SettingsDraft::default().calendar_colors_enabled);
        assert!(settings_from_data(&json!({"settings": {}})).calendar_colors_enabled);
        let mut old = serde_json::to_value(SettingsDraft::default()).unwrap();
        old.as_object_mut().unwrap().remove("calendarColorsEnabled");
        assert!(serde_json::from_value::<SettingsDraft>(old).unwrap().calendar_colors_enabled);
        for enabled in [false, true] {
            let settings = SettingsDraft { calendar_colors_enabled: enabled, ..SettingsDraft::default() };
            let mut data = json!({"unrelated": "keep"});
            apply_settings(&mut data, Path::new("fixture"), &settings);
            assert_eq!(settings_from_data(&data).calendar_colors_enabled, enabled);
            assert_eq!(data["settings"]["calendarColorsEnabled"], enabled);
            assert_eq!(data["unrelated"], "keep");
        }
    }

    #[test]
    fn selection_settings_migrate_and_preserve_explicit_values() {
        assert!(SettingsDraft::default().include_selections);
        assert!(settings_from_data(&json!({"settings": {}})).include_selections);
        let mut legacy = serde_json::to_value(SettingsDraft::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("includeSelections");
        assert!(serde_json::from_value::<SettingsDraft>(legacy).unwrap().include_selections);
        for enabled in [false, true] {
            let draft = SettingsDraft { include_selections: enabled, ..SettingsDraft::default() };
            let mut data = json!({});
            apply_settings(&mut data, Path::new("fixture"), &draft);
            assert_eq!(data["settings"]["includeSelections"], enabled);
            assert_eq!(data["settingsV2"]["settings"]["includeSelections"], enabled);
            assert_eq!(settings_from_data(&data).include_selections, enabled);
            assert_eq!(serde_json::from_value::<SettingsDraft>(serde_json::to_value(draft).unwrap()).unwrap().include_selections, enabled);
        }
        assert!(!settings_from_data(&json!({"settings": {"includeSelections": false}})).include_selections);
        assert!(!settings_from_data(&json!({"settings": {"includeSelections": true}, "settingsV2": {"settings": {"includeSelections": false}}})).include_selections);
    }

    #[test]
    fn kanban_settings_default_validate_and_restore() {
        let defaults = SettingsDraft::default();
        assert_eq!(defaults.kanban_view_style, "default");
        assert!(!defaults.collapsed_categories);
        for style in ["default", "balanced", "compact"] {
            let settings = SettingsDraft {
                accent_mode: "custom".into(),
                accent_preset: String::new(),
                kanban_view_style: style.into(),
                collapsed_categories: true,
                ..SettingsDraft::default()
            };
            assert!(validate_settings(settings.clone()).is_ok());
            let mut data = json!({});
            apply_settings(&mut data, Path::new("fixture"), &settings);
            let restored = settings_from_data(&data);
            assert_eq!(restored.kanban_view_style, style);
            assert!(restored.collapsed_categories);
        }
        let legacy = settings_from_data(&json!({"settings": {"kanbanViewStyle": "balanced", "collapsedCategories": true}}));
        assert_eq!(legacy.kanban_view_style, "balanced");
        assert!(legacy.collapsed_categories);
        let invalid = settings_from_data(&json!({"settingsV2": {"settings": {"kanbanViewStyle": "bad", "collapsedCategories": "true"}}}));
        assert_eq!(invalid.kanban_view_style, "default");
        assert!(!invalid.collapsed_categories);
        assert!(validate_settings(SettingsDraft { kanban_view_style: "bad".into(), ..SettingsDraft::default() }).is_err());
        let old_draft = serde_json::to_value(SettingsDraft::default()).unwrap();
        let mut old_draft = old_draft.as_object().unwrap().clone();
        old_draft.remove("kanbanViewStyle");
        old_draft.remove("collapsedCategories");
        let restored: SettingsDraft = serde_json::from_value(Value::Object(old_draft)).unwrap();
        assert_eq!(restored.kanban_view_style, "default");
        assert!(!restored.collapsed_categories);
    }

    #[test]
    fn settings_save_preserves_unknown_fields_and_future_settings_versions() {
        let mut data = json!({
            "futureRootField": {"kept": true},
            "settings": {"futureLegacyField": "kept"},
            "settingsV2": {
                "version": 9,
                "futureEnvelopeField": ["kept"],
                "settings": {
                    "futureSetting": {"kept": true},
                    "codexIntegration": {
                        "futureCodexField": {"kept": true},
                        "spaces": {"existing": {"access": "read"}}
                    }
                }
            }
        });
        let settings = validate_settings(SettingsDraft::default()).unwrap();

        apply_settings(&mut data, Path::new("fixture"), &settings);

        assert_eq!(data["futureRootField"]["kept"], true);
        assert_eq!(data["settings"]["futureLegacyField"], "kept");
        assert_eq!(data["settingsV2"]["version"], 9);
        assert_eq!(data["settingsV2"]["futureEnvelopeField"][0], "kept");
        assert_eq!(data["settingsV2"]["settings"]["futureSetting"]["kept"], true);
        assert_eq!(data["settingsV2"]["settings"]["codexIntegration"]["futureCodexField"]["kept"], true);
        assert_eq!(data["settingsV2"]["settings"]["codexIntegration"]["spaces"]["existing"]["access"], "read");
    }

    #[test]
    fn reads_bom_prefixed_legacy_settings_json() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let root = workspace.join(".test-run").join(format!(
            "settings-json-bom-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("data.json");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(br#"{"settings":{"fontFamily":"Inter"}}"#);
        fs::write(&path, bytes).unwrap();

        assert_eq!(
            read_json_file(&path).unwrap()["settings"]["fontFamily"],
            "Inter"
        );

        fs::remove_dir_all(root).unwrap();
    }

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
    fn external_links_are_limited_to_plain_http_and_https_urls() {
        assert_eq!(
            validate_external_link("https://example.com/a?b=c")
                .unwrap()
                .scheme(),
            "https"
        );
        assert!(validate_external_link("javascript:alert(1)").is_err());
        assert!(validate_external_link("file:///C:/secret.txt").is_err());
        assert!(validate_external_link("https://user@example.com/").is_err());
        assert!(validate_external_link("https://").is_err());
        assert!(validate_external_link("https://tauri.localhost/").is_err());
    }

    #[test]
    fn local_navigation_only_allows_the_settings_document() {
        for value in [
            "tauri://localhost/",
            "tauri://localhost/index.html#release-notes",
            "http://tauri.localhost/index.html?tab=docs#intro",
            "https://tauri.localhost/",
        ] {
            assert!(is_local_app_url(&url::Url::parse(value).unwrap()), "{value}");
        }

        for value in [
            "https://user@tauri.localhost/",
            "http://tauri.localhost:8080/",
            "https://tauri.localhost/docs.md",
            "https://tauri.localhost.evil.example/",
            "http://ipc.localhost/",
            "https://localhost/",
            "tauri://localhost/docs.md",
        ] {
            assert!(!is_local_app_url(&url::Url::parse(value).unwrap()), "{value}");
        }
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

