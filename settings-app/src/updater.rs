use chrono::{SecondsFormat, Utc};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{css, storage};

const OWNER: &str = "PainterFlow";
const REPOSITORY: &str = "Anytype-BlobUI";
const DEFAULT_RELEASE_VERSION: &str = "1.1.0";
const MAX_RELEASE_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MAX_EXPANDED_BYTES: usize = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 20_000;
const PACKAGE_SCRIPTS: &[&str] = &[
    "BlobUI.Core.psm1",
    "Get-Data.ps1",
    "Compile.ps1",
    "Install.ps1",
    "Rebuild.ps1",
    "Update.ps1",
    "Uninstall.ps1",
    "Settings.ps1",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdaterPreferences {
    pub install_directory: Option<String>,
    pub automatic_updates_enabled: bool,
    pub automatic_checks_enabled: bool,
    pub last_checked_at: Option<String>,
    pub latest_version: Option<String>,
    pub last_check_status: Option<String>,
    pub last_message: Option<String>,
}

impl Default for UpdaterPreferences {
    fn default() -> Self {
        Self {
            install_directory: None,
            automatic_updates_enabled: false,
            automatic_checks_enabled: true,
            last_checked_at: None,
            latest_version: None,
            last_check_status: None,
            last_message: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterSettingsDraft {
    pub install_directory: String,
    pub automatic_updates_enabled: bool,
    pub automatic_checks_enabled: bool,
    pub start_on_login: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterState {
    pub install_directory: String,
    pub automatic_updates_enabled: bool,
    pub automatic_checks_enabled: bool,
    pub start_on_login: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub last_checked_at: Option<String>,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
struct ReleaseInfo {
    tag: String,
    version: semver::Version,
}

#[derive(Clone, Debug)]
struct PreviousManifest {
    release_version: Option<String>,
    managed_files: Vec<String>,
}

pub fn preferences_path(app_data_directory: &Path) -> PathBuf {
    app_data_directory
        .parent()
        .unwrap_or(app_data_directory)
        .join("BlobUI")
        .join("updater.json")
}

pub fn load_preferences(path: &Path) -> Result<UpdaterPreferences, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            reject_reparse_path(path)?;
            if !metadata.is_file() {
                return Err("The saved BlobUI updater settings are not a regular file.".into());
            }
            let text = fs::read_to_string(path)
                .map_err(|_| "The saved BlobUI updater settings could not be read.".to_string())?;
            serde_json::from_str(&text)
                .map_err(|_| "The saved BlobUI updater settings are not valid JSON.".to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(UpdaterPreferences::default())
        }
        Err(_) => Err("The saved BlobUI updater settings could not be inspected safely.".into()),
    }
}

fn save_preferences(path: &Path, preferences: &UpdaterPreferences) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(preferences)
        .map_err(|_| "Could not encode the BlobUI updater settings.".to_string())?;
    bytes.push(b'\n');
    storage::write_atomic(path, &bytes)
}

pub fn saved_install_directory(path: &Path) -> Result<Option<PathBuf>, String> {
    let preferences = load_preferences(path)?;
    Ok(preferences
        .install_directory
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from))
}

pub fn save_settings(
    app_data_directory: &Path,
    draft: UpdaterSettingsDraft,
    resolved_default: &Path,
) -> Result<UpdaterState, String> {
    let path = preferences_path(app_data_directory);
    let mut preferences = load_preferences(&path)?;
    let requested = draft.install_directory.trim();
    preferences.install_directory = if requested.is_empty() {
        None
    } else {
        Some(
            validate_install_directory(Path::new(requested))?
                .to_string_lossy()
                .into_owned(),
        )
    };
    preferences.automatic_updates_enabled = draft.automatic_updates_enabled;
    preferences.automatic_checks_enabled = draft.automatic_checks_enabled;
    save_preferences(&path, &preferences)?;
    let install = preferences
        .install_directory
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| resolved_default.to_path_buf());
    state_for(&install, &preferences, None)
}

pub fn get_state(
    app_data_directory: &Path,
    install_directory: &Path,
    fallback_runtime: &Path,
) -> Result<UpdaterState, String> {
    let preferences = load_preferences(&preferences_path(app_data_directory))?;
    state_for(install_directory, &preferences, Some(fallback_runtime))
}

fn state_for(
    install_directory: &Path,
    preferences: &UpdaterPreferences,
    fallback_runtime: Option<&Path>,
) -> Result<UpdaterState, String> {
    let current = current_version(install_directory, fallback_runtime)?;
    let latest = preferences
        .latest_version
        .as_deref()
        .and_then(parse_version);
    let current_parsed = parse_version(&current);
    let update_available = preferences.last_check_status.as_deref() != Some("error")
        && latest
            .as_ref()
            .zip(current_parsed.as_ref())
            .is_some_and(|(latest, current)| latest > current);
    let status = if preferences.last_check_status.as_deref() == Some("error") {
        "error"
    } else if update_available {
        "available"
    } else {
        preferences.last_check_status.as_deref().unwrap_or("idle")
    };
    let message = if update_available {
        Some(format!("BlobUI version {} is available.", latest.unwrap()))
    } else {
        preferences.last_message.clone()
    };
    Ok(UpdaterState {
        install_directory: install_directory.to_string_lossy().into_owned(),
        automatic_updates_enabled: preferences.automatic_updates_enabled,
        automatic_checks_enabled: preferences.automatic_checks_enabled,
        start_on_login: false,
        current_version: current,
        latest_version: preferences.latest_version.clone(),
        update_available,
        last_checked_at: preferences.last_checked_at.clone(),
        status: status.into(),
        message,
    })
}

pub fn check_for_updates(
    app_data_directory: &Path,
    install_directory: &Path,
    fallback_runtime: &Path,
) -> Result<UpdaterState, String> {
    let path = preferences_path(app_data_directory);
    let mut preferences = load_preferences(&path)?;
    let checked_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    preferences.last_checked_at = Some(checked_at);
    let current = current_version(install_directory, Some(fallback_runtime))?;
    match fetch_latest_release() {
        Ok(None) => {
            preferences.latest_version = None;
            preferences.last_check_status = Some("current".into());
            preferences.last_message = Some("No published BlobUI release is available yet.".into());
        }
        Ok(Some(release)) => {
            preferences.latest_version = Some(release.version.to_string());
            let current_version = parse_version(&current)
                .ok_or_else(|| "The installed BlobUI release version is invalid.".to_string())?;
            if release.version > current_version {
                preferences.last_check_status = Some("available".into());
                preferences.last_message =
                    Some(format!("BlobUI version {} is available.", release.version));
            } else {
                preferences.last_check_status = Some("current".into());
                preferences.last_message = Some("BlobUI is up to date.".into());
            }
        }
        Err(error) => {
            preferences.last_check_status = Some("error".into());
            preferences.last_message = Some(error);
        }
    }
    save_preferences(&path, &preferences)?;
    state_for(install_directory, &preferences, Some(fallback_runtime))
}

pub fn simulate_update(
    app_data_directory: &Path,
    install_directory: &Path,
    fallback_runtime: &Path,
) -> Result<UpdaterState, String> {
    let preferences = load_preferences(&preferences_path(app_data_directory))?;
    let mut state = state_for(install_directory, &preferences, Some(fallback_runtime))?;
    state.latest_version = Some("Simulation".into());
    state.update_available = true;
    state.status = "simulated".into();
    state.message = Some("Simulated update only. No files were downloaded or changed.".into());
    Ok(state)
}

pub fn apply_update(
    app_data_directory: &Path,
    install_directory: &Path,
    fallback_runtime: &Path,
) -> Result<UpdaterState, String> {
    let path = preferences_path(app_data_directory);
    let mut preferences = load_preferences(&path)?;
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    preferences.last_checked_at = Some(now);
    let current = current_version(install_directory, Some(fallback_runtime))?;
    let release = match fetch_latest_release() {
        Ok(Some(release)) => release,
        Ok(None) => {
            preferences.latest_version = None;
            preferences.last_check_status = Some("current".into());
            preferences.last_message =
                Some("No published BlobUI release is available to install.".into());
            save_preferences(&path, &preferences)?;
            return state_for(install_directory, &preferences, Some(fallback_runtime));
        }
        Err(error) => {
            preferences.last_check_status = Some("error".into());
            preferences.last_message = Some(error);
            save_preferences(&path, &preferences)?;
            return state_for(install_directory, &preferences, Some(fallback_runtime));
        }
    };
    preferences.latest_version = Some(release.version.to_string());
    let current_version = parse_version(&current)
        .ok_or_else(|| "The installed BlobUI release version is invalid.".to_string())?;
    if release.version <= current_version {
        preferences.last_check_status = Some("current".into());
        preferences.last_message = Some("BlobUI is already up to date.".into());
        save_preferences(&path, &preferences)?;
        return state_for(install_directory, &preferences, Some(fallback_runtime));
    }

    match download_source_archive(&release.tag).and_then(|archive| {
        apply_archive_to_install(install_directory, &archive, &release.version.to_string())
    }) {
        Ok(version) => {
            preferences.latest_version = Some(version.clone());
            preferences.last_check_status = Some("updated".into());
            preferences.last_message = Some(format!(
                "BlobUI was updated to version {version}. Reload Anytype with Ctrl+R."
            ));
        }
        Err(error) => {
            preferences.last_check_status = Some("error".into());
            preferences.last_message = Some(error);
        }
    }
    save_preferences(&path, &preferences)?;
    state_for(install_directory, &preferences, Some(fallback_runtime))
}

fn current_version(
    install_directory: &Path,
    fallback_runtime: Option<&Path>,
) -> Result<String, String> {
    let blob = install_directory.join("blob");
    let manifest_path = blob.join(".blobui-managed-files.json");
    if fs::symlink_metadata(&manifest_path).is_ok() {
        reject_reparse_path(&manifest_path)?;
        let manifest = read_manifest(&manifest_path)?;
        if let Some(version) = manifest.release_version {
            let version = parse_version(&version).ok_or_else(|| {
                "The saved BlobUI install manifest has an invalid release version.".to_string()
            })?;
            return Ok(version.to_string());
        }
    }
    let installed_runtime = blob.join("runtime");
    for root in [Some(installed_runtime.as_path()), fallback_runtime]
        .into_iter()
        .flatten()
    {
        if let Some(version) = version_from_runtime(root) {
            return Ok(version.to_string());
        }
    }
    Ok(DEFAULT_RELEASE_VERSION.into())
}

fn version_from_runtime(runtime: &Path) -> Option<semver::Version> {
    let script = runtime.join("scripts").join("BlobUI.Core.psm1");
    let text = fs::read_to_string(script).ok()?;
    let captures = regex::Regex::new(r"(?m)^\s*\$script:BlobUiReleaseVersion\s*=\s*'([^']+)'\s*$")
        .ok()?
        .captures(&text)?;
    parse_version(captures.get(1)?.as_str())
}

fn parse_version(value: &str) -> Option<semver::Version> {
    let value = value.trim().strip_prefix('v').unwrap_or(value.trim());
    semver::Version::parse(value).ok()
}

fn fetch_latest_release() -> Result<Option<ReleaseInfo>, String> {
    let response = http_get(
        "api.github.com",
        &format!("/repos/{OWNER}/{REPOSITORY}/releases/latest"),
        "application/vnd.github+json",
        MAX_RELEASE_JSON_BYTES,
    )?;
    if response.status == 404 {
        return Err("The BlobUI release source is unavailable. Make the repository and a published release publicly accessible, then try again.".into());
    }
    if response.status != 200 {
        return Err(format!(
            "GitHub release check returned HTTP {}.",
            response.status
        ));
    }
    let release: Value = serde_json::from_slice(&response.body)
        .map_err(|_| "GitHub returned an invalid release record.".to_string())?;
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "GitHub's release record has no tag name.".to_string())?;
    let version = parse_version(tag).ok_or_else(|| {
        "The latest GitHub release tag is not a supported semantic version.".to_string()
    })?;
    Ok(Some(ReleaseInfo {
        tag: tag.to_string(),
        version,
    }))
}

fn download_source_archive(tag: &str) -> Result<Vec<u8>, String> {
    let version = parse_version(tag)
        .ok_or_else(|| "The GitHub release tag is not a safe semantic version.".to_string())?;
    let normalized_tag = if tag.starts_with('v') {
        tag.to_string()
    } else {
        version.to_string()
    };
    let response = http_get(
        "github.com",
        &format!("/{OWNER}/{REPOSITORY}/archive/refs/tags/{normalized_tag}.tar.gz"),
        "application/vnd.github+json",
        MAX_ARCHIVE_BYTES,
    )?;
    if response.status != 200 {
        return Err(format!(
            "GitHub's BlobUI source archive returned HTTP {}.",
            response.status
        ));
    }
    Ok(response.body)
}

struct HttpResponse {
    status: u32,
    body: Vec<u8>,
}

#[cfg(windows)]
fn http_get(
    host: &str,
    path: &str,
    accept: &str,
    max_bytes: usize,
) -> Result<HttpResponse, String> {
    use std::ptr;
    use windows_sys::Win32::Networking::WinHttp::*;

    struct Handle(*mut std::ffi::c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    let user_agent = wide("BlobUI Settings updater");
    let host_wide = wide(host);
    let path_wide = wide(path);
    let get = wide("GET");
    let headers = wide(&format!(
        "Accept: {accept}\r\nUser-Agent: BlobUI-Settings\r\n"
    ));
    let session = Handle(unsafe {
        WinHttpOpen(
            user_agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            ptr::null(),
            ptr::null(),
            0,
        )
    });
    if session.0.is_null() {
        return Err("Windows could not start the BlobUI updater connection.".into());
    }
    if unsafe { WinHttpSetTimeouts(session.0, 15_000, 15_000, 15_000, 30_000) } == 0 {
        return Err(
            "Windows could not set a safe timeout for the BlobUI updater connection.".into(),
        );
    }
    let connection = Handle(unsafe { WinHttpConnect(session.0, host_wide.as_ptr(), 443, 0) });
    if connection.0.is_null() {
        return Err("Windows could not connect to GitHub for a BlobUI update.".into());
    }
    let request = Handle(unsafe {
        WinHttpOpenRequest(
            connection.0,
            get.as_ptr(),
            path_wide.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    });
    if request.0.is_null() {
        return Err("Windows could not prepare the GitHub update request.".into());
    }
    let sent = unsafe {
        WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            u32::MAX,
            ptr::null_mut(),
            0,
            0,
            0,
        )
    };
    if sent == 0 || unsafe { WinHttpReceiveResponse(request.0, ptr::null_mut()) } == 0 {
        return Err("Could not reach GitHub to check for BlobUI updates.".into());
    }
    let mut status = 0u32;
    let mut status_bytes = std::mem::size_of::<u32>() as u32;
    if unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut status_bytes,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err("GitHub did not return an HTTP status for the update request.".into());
    }
    let mut body = Vec::new();
    loop {
        let mut available = 0u32;
        if unsafe { WinHttpQueryDataAvailable(request.0, &mut available) } == 0 {
            return Err("GitHub's update response could not be read.".into());
        }
        if available == 0 {
            break;
        }
        if body.len().saturating_add(available as usize) > max_bytes {
            return Err("GitHub's update response is larger than the supported limit.".into());
        }
        let start = body.len();
        body.resize(start + available as usize, 0);
        let mut read = 0u32;
        if unsafe {
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr().cast(),
                available,
                &mut read,
            )
        } == 0
        {
            return Err("GitHub's update response could not be read.".into());
        }
        body.truncate(start + read as usize);
    }
    Ok(HttpResponse { status, body })
}

#[cfg(not(windows))]
fn http_get(_: &str, _: &str, _: &str, _: usize) -> Result<HttpResponse, String> {
    Err("The BlobUI in-app updater is supported on Windows.".into())
}

pub fn apply_archive_to_install(
    install_directory: &Path,
    archive: &[u8],
    expected_version: &str,
) -> Result<String, String> {
    let install = validate_install_directory(install_directory)?;
    let expected_version = parse_version(expected_version)
        .ok_or_else(|| "The downloaded BlobUI release version is invalid.".to_string())?;
    if archive.len() > MAX_ARCHIVE_BYTES {
        return Err(
            "The downloaded BlobUI source archive is larger than the supported limit.".into(),
        );
    }
    let blob = install.join("blob");
    let staging_root = blob.join("staging");
    reject_reparse_path(&staging_root)?;
    fs::create_dir_all(&staging_root)
        .map_err(|_| "Could not prepare the BlobUI update staging folder.".to_string())?;
    let stage = staging_root.join(format!(
        "rust-update-{}-{}",
        std::process::id(),
        unique_nonce()
    ));
    fs::create_dir(&stage)
        .map_err(|_| "Could not create the BlobUI update staging folder.".to_string())?;
    let guard = RemoveTree(stage.clone());
    let source_root = stage.join("source");
    fs::create_dir(&source_root)
        .map_err(|_| "Could not prepare the downloaded BlobUI source.".to_string())?;
    extract_source_archive(archive, &source_root)?;

    let packaged_version = version_from_runtime(&source_root).ok_or_else(|| {
        "The source archive does not contain a valid BlobUI release version.".to_string()
    })?;
    if packaged_version != expected_version {
        return Err("The source archive version does not match the GitHub release tag.".into());
    }
    let previous_manifest_path = blob.join(".blobui-managed-files.json");
    let previous_manifest = if fs::symlink_metadata(&previous_manifest_path).is_ok() {
        read_manifest(&previous_manifest_path)?
    } else {
        PreviousManifest {
            release_version: None,
            managed_files: Vec::new(),
        }
    };
    let current = current_version(&install, Some(&install.join("blob/runtime")))?;
    let current_version = parse_version(&current)
        .ok_or_else(|| "The installed BlobUI release version is invalid.".to_string())?;
    if packaged_version <= current_version {
        return Err("The installed BlobUI theme is already at this release or newer.".into());
    }

    let data_path = blob.join("data.json");
    reject_reparse_path(&data_path)?;
    let data: Value = serde_json::from_slice(
        &fs::read(&data_path)
            .map_err(|_| "BlobUI settings could not be read for the update.".to_string())?,
    )
    .map_err(|_| "BlobUI data.json is not valid JSON.".to_string())?;
    if !data.is_object() || !data_matches_install(&data, &install) {
        return Err("BlobUI settings do not match the selected Anytype installation.".into());
    }

    let runtime_destination = blob.join("runtime");
    let compile_root = stage.join("compile-source");
    copy_regular_tree(
        &source_root.join("Dev"),
        &compile_root.join("Dev"),
        Some("css"),
    )?;
    copy_regular_tree(
        &source_root.join("assets"),
        &compile_root.join("assets"),
        None,
    )?;
    let release_dev = source_root.join("Dev");
    let local_dev = runtime_destination.join("Dev");
    preserve_unowned_css(
        &release_dev,
        &local_dev,
        &compile_root.join("Dev"),
        &previous_manifest.managed_files,
    )?;
    let normalized_settings = super::validate_settings(super::settings_from_data(&data))?;
    let mut compile_data = data.clone();
    super::apply_settings(&mut compile_data, &install, &normalized_settings);
    let css = css::compile(&compile_root, &install, &compile_data)?;
    let changes = prepare_file_changes(
        &install,
        &blob,
        &source_root,
        &css,
        &previous_manifest,
        &packaged_version.to_string(),
    )?;
    storage::commit_update(changes)?;
    let version = packaged_version.to_string();
    drop(guard);
    Ok(version)
}

struct RemoveTree(PathBuf);
impl Drop for RemoveTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn prepare_file_changes(
    install: &Path,
    blob: &Path,
    source_root: &Path,
    compiled_css: &str,
    previous: &PreviousManifest,
    version: &str,
) -> Result<Vec<storage::FileChange>, String> {
    let mut changes = Vec::new();
    let mut desired = HashMap::<String, Vec<u8>>::new();
    collect_package_files(
        &source_root.join("Dev"),
        &mut desired,
        "runtime/Dev",
        Some("css"),
    )?;
    collect_package_files(
        &source_root.join("assets"),
        &mut desired,
        "runtime/assets",
        None,
    )?;
    collect_package_files(&source_root.join("assets"), &mut desired, "assets", None)?;
    for name in PACKAGE_SCRIPTS {
        let source = source_root.join("scripts").join(name);
        let bytes = fs::read(&source)
            .map_err(|_| format!("The source archive is missing packaged script {name}."))?;
        desired.insert(format!("runtime/scripts/{name}"), bytes);
    }
    if desired.keys().all(|path| !path.starts_with("runtime/Dev/")) {
        return Err("The source archive contains no BlobUI CSS files.".into());
    }
    if !desired.contains_key("runtime/assets/spacebanner/banner.png")
        || !desired.contains_key("assets/spacebanner/banner.png")
    {
        return Err("The source archive is missing BlobUI's default banner asset.".into());
    }

    let banner_path = blob.join("assets/spacebanner/banner.png");
    reject_reparse_path(&banner_path)?;
    let default_banner = desired
        .remove("assets/spacebanner/banner.png")
        .ok_or_else(|| {
            "The source archive is missing BlobUI's default banner asset.".to_string()
        })?;
    match fs::symlink_metadata(&banner_path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err("The installed BlobUI banner path is not a regular file.".into());
            }
            // This file can contain a user's custom banner, so preserve it.
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            changes.push(storage::FileChange {
                path: banner_path,
                contents: Some(default_banner),
            });
        }
        Err(_) => {
            return Err("The installed BlobUI banner path could not be inspected safely.".into())
        }
    }

    let previous_keys = previous
        .managed_files
        .iter()
        .map(|key| key.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let binary_path = blob.join("runtime/settings-app.exe");
    if previous_keys.contains("runtime/settings-app.exe") && binary_path.is_file() {
        // Keep ownership metadata for cleanup, but never replace the running app.
    }
    let mut desired_keys = desired
        .keys()
        .map(|key| key.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    if previous_keys.contains("runtime/settings-app.exe") && binary_path.is_file() {
        desired_keys.insert("runtime/settings-app.exe".into());
    }
    let mut managed_files = desired.keys().cloned().collect::<Vec<_>>();
    if previous_keys.contains("runtime/settings-app.exe") && binary_path.is_file() {
        managed_files.push("runtime/settings-app.exe".into());
    }
    for key in &previous.managed_files {
        if !desired_keys.contains(&key.to_ascii_lowercase()) {
            changes.push(storage::FileChange {
                path: managed_destination(blob, key)?,
                contents: None,
            });
        }
    }
    for (key, bytes) in desired {
        changes.push(storage::FileChange {
            path: managed_destination(blob, &key)?,
            contents: Some(bytes),
        });
    }
    changes.push(storage::FileChange {
        path: install.join("custom.css"),
        contents: Some(compiled_css.as_bytes().to_vec()),
    });

    // Keep an existing managed app binary in the ownership manifest without replacing it.
    managed_files.sort_by_key(|key| key.to_ascii_lowercase());
    managed_files.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    let manifest = json!({
        "manifestVersion": 1,
        "releaseVersion": version,
        "managedFiles": managed_files,
    });
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| "Could not prepare the BlobUI update manifest.".to_string())?;
    bytes.push(b'\n');
    changes.push(storage::FileChange {
        path: blob.join(".blobui-managed-files.json"),
        contents: Some(bytes),
    });
    Ok(changes)
}

fn collect_package_files(
    root: &Path,
    output: &mut HashMap<String, Vec<u8>>,
    key_prefix: &str,
    required_extension: Option<&str>,
) -> Result<(), String> {
    let mut files = Vec::new();
    collect_regular_files(root, &mut files)?;
    for file in files {
        if required_extension.is_some_and(|extension| {
            !file
                .extension()
                .is_some_and(|found| found.eq_ignore_ascii_case(extension))
        }) {
            continue;
        }
        let relative = file
            .strip_prefix(root)
            .map_err(|_| "A packaged BlobUI file escaped its source folder.".to_string())?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        let key = format!("{key_prefix}/{relative}");
        validate_managed_key(&key)?;
        output.insert(
            key,
            fs::read(&file)
                .map_err(|_| "A packaged BlobUI source file could not be read.".to_string())?,
        );
    }
    Ok(())
}

fn preserve_unowned_css(
    release_dev: &Path,
    installed_dev: &Path,
    compile_dev: &Path,
    previous_managed: &[String],
) -> Result<(), String> {
    let managed = previous_managed
        .iter()
        .map(|key| key.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let mut release_files = Vec::new();
    collect_regular_files(release_dev, &mut release_files)?;
    let collisions = release_files
        .iter()
        .filter_map(|path| path.strip_prefix(release_dev).ok())
        .map(|path| {
            path.to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase()
        })
        .collect::<HashSet<_>>();
    if !installed_dev.is_dir() {
        return Ok(());
    }
    let mut local_files = Vec::new();
    collect_regular_files(installed_dev, &mut local_files)?;
    for file in local_files {
        if !file
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("css"))
        {
            continue;
        }
        let relative = file
            .strip_prefix(installed_dev)
            .map_err(|_| "An installed local CSS file escaped the Dev folder.".to_string())?;
        let relative_key = relative.to_string_lossy().replace('\\', "/");
        let managed_key = format!("runtime/Dev/{relative_key}").to_ascii_lowercase();
        if managed.contains(&managed_key) || collisions.contains(&relative_key.to_ascii_lowercase())
        {
            continue;
        }
        let destination = compile_dev.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                "Could not preserve a local BlobUI CSS folder for update compilation.".to_string()
            })?;
        }
        fs::copy(&file, &destination).map_err(|_| {
            "Could not preserve local BlobUI CSS for update compilation.".to_string()
        })?;
    }
    Ok(())
}

fn copy_regular_tree(
    source: &Path,
    destination: &Path,
    extension: Option<&str>,
) -> Result<(), String> {
    let mut files = Vec::new();
    collect_regular_files(source, &mut files)?;
    for file in files {
        if extension.is_some_and(|extension| {
            !file
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        }) {
            continue;
        }
        let relative = file
            .strip_prefix(source)
            .map_err(|_| "A BlobUI source file escaped its package folder.".to_string())?;
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| "Could not prepare the BlobUI update compiler input.".to_string())?;
        }
        fs::copy(&file, target)
            .map_err(|_| "Could not prepare the BlobUI update compiler input.".to_string())?;
    }
    Ok(())
}

fn collect_regular_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(directory)
        .map_err(|_| "Could not read a BlobUI update source folder.".to_string())?;
    for entry in entries {
        let entry =
            entry.map_err(|_| "Could not inspect a BlobUI update source file.".to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| "Could not inspect a BlobUI update source file.".to_string())?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            continue;
        }
        if metadata.is_dir() {
            collect_regular_files(&path, output)?;
        } else if metadata.is_file() {
            output.push(path);
        }
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<PreviousManifest, String> {
    reject_reparse_path(path)?;
    let bytes = fs::read(path)
        .map_err(|_| "The saved BlobUI install manifest could not be read.".to_string())?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "The saved BlobUI install manifest is invalid.".to_string())?;
    if value.get("manifestVersion").and_then(Value::as_u64) != Some(1) {
        return Err("The saved BlobUI install manifest has an unsupported format.".into());
    }
    let list = value
        .get("managedFiles")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "The saved BlobUI install manifest is missing its managed file list.".to_string()
        })?;
    let mut seen = HashSet::new();
    let mut managed_files = Vec::new();
    for item in list {
        let raw = item.as_str().ok_or_else(|| {
            "The saved BlobUI install manifest contains an invalid file path.".to_string()
        })?;
        if let Some(key) = validate_managed_key(raw)? {
            if seen.insert(key.to_ascii_lowercase()) {
                managed_files.push(key);
            }
        }
    }
    let release_version = value
        .get("releaseVersion")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(PreviousManifest {
        release_version,
        managed_files,
    })
}

fn validate_managed_key(raw: &str) -> Result<Option<String>, String> {
    let key = raw.replace('\\', "/");
    if key.trim().is_empty()
        || key.starts_with('/')
        || key.contains(':')
        || key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("The BlobUI install manifest contains an unsafe file path.".into());
    }
    if key.eq_ignore_ascii_case("assets/spacebanner/banner.png") {
        return Ok(None);
    }
    if !(key.starts_with("assets/")
        || key.starts_with("runtime/scripts/")
        || key.starts_with("runtime/Dev/")
        || key.starts_with("runtime/assets/")
        || key == "runtime/settings-app.exe")
    {
        return Err("The BlobUI install manifest contains an unsupported file path.".into());
    }
    Ok(Some(key))
}

fn managed_destination(blob: &Path, key: &str) -> Result<PathBuf, String> {
    let key = validate_managed_key(key)?
        .ok_or_else(|| "The BlobUI update tried to replace a user banner.".to_string())?;
    let destination = blob.join(key.replace('/', std::path::MAIN_SEPARATOR_STR));
    let root = blob.canonicalize().unwrap_or_else(|_| blob.to_path_buf());
    let full = destination
        .canonicalize()
        .unwrap_or_else(|_| destination.clone());
    if !full.starts_with(&root) {
        return Err("A BlobUI update file escaped the Anytype blob folder.".into());
    }
    reject_reparse_path(&destination)?;
    Ok(destination)
}

pub fn validate_install_directory(path: &Path) -> Result<PathBuf, String> {
    reject_reparse_path(path)?;
    let install = path
        .canonicalize()
        .map_err(|_| "The selected Anytype installation folder does not exist.".to_string())?;
    if !install.is_dir() {
        return Err("The selected Anytype installation folder does not exist.".into());
    }
    ensure_not_release_path(&install)?;
    reject_reparse_path(&install)?;
    let data_path = install.join("blob/data.json");
    reject_reparse_path(&data_path)?;
    let bytes = fs::read(&data_path).map_err(|_| {
        "BlobUI settings were not found in the selected Anytype folder.".to_string()
    })?;
    let data: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "BlobUI data.json in the selected Anytype folder is invalid.".to_string())?;
    if !data_matches_install(&data, &install) {
        return Err("BlobUI settings do not match the selected Anytype folder.".into());
    }
    Ok(install)
}

fn data_matches_install(data: &Value, install: &Path) -> bool {
    let Some(saved) = data.get("installDirectory").and_then(Value::as_str) else {
        return false;
    };
    let saved = Path::new(saved)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(saved));
    same_path(&saved, install)
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn ensure_not_release_path(path: &Path) -> Result<(), String> {
    if path.components().any(|component| match component {
        Component::Normal(part) => part.eq_ignore_ascii_case("Releases"),
        _ => false,
    }) {
        return Err("BlobUI will not update an installation inside a Releases folder.".into());
    }
    Ok(())
}

fn reject_reparse_path(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() || is_reparse_point(&metadata) => {
                return Err(
                    "A BlobUI update path contains a symbolic link or reparse point.".into(),
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("A BlobUI update path could not be inspected safely.".into()),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

fn extract_source_archive(archive: &[u8], destination: &Path) -> Result<(), String> {
    if archive.len() > MAX_ARCHIVE_BYTES {
        return Err(
            "The downloaded BlobUI source archive is larger than the supported limit.".into(),
        );
    }
    let decoder = GzDecoder::new(archive);
    let mut tar = Vec::new();
    decoder
        .take((MAX_EXPANDED_BYTES + 1) as u64)
        .read_to_end(&mut tar)
        .map_err(|_| "The downloaded BlobUI source archive is damaged.".to_string())?;
    if tar.len() > MAX_EXPANDED_BYTES {
        return Err(
            "The expanded BlobUI source archive is larger than the supported limit.".into(),
        );
    }
    unpack_selected_tar_entries(&tar, destination)
}

fn unpack_selected_tar_entries(tar: &[u8], destination: &Path) -> Result<(), String> {
    let mut offset = 0usize;
    let mut entry_count = 0usize;
    let mut expanded_bytes = 0usize;
    let mut root_prefix: Option<String> = None;
    let mut pending_pax_path: Option<String> = None;
    let mut seen_paths = HashSet::new();
    while offset + 512 <= tar.len() {
        let header = &tar[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        entry_count += 1;
        if entry_count > MAX_ARCHIVE_ENTRIES {
            return Err("The BlobUI source archive contains too many files.".into());
        }
        verify_tar_checksum(header)?;
        let size = parse_tar_octal(&header[124..136])?;
        let size =
            usize::try_from(size).map_err(|_| "A BlobUI archive file is too large.".to_string())?;
        let typeflag = header[156];
        let data_start = offset + 512;
        let data_end = data_start
            .checked_add(size)
            .ok_or_else(|| "The BlobUI source archive is invalid.".to_string())?;
        if data_end > tar.len() {
            return Err("The BlobUI source archive is truncated.".into());
        }
        if matches!(typeflag, b'g' | b'x') && size > 1024 * 1024 {
            return Err("The BlobUI source archive contains oversized PAX metadata.".into());
        }
        let padded_size = size
            .checked_add(511)
            .ok_or_else(|| "The BlobUI source archive is invalid.".to_string())?
            / 512
            * 512;
        offset = data_start
            .checked_add(padded_size)
            .ok_or_else(|| "The BlobUI source archive is invalid.".to_string())?;
        if offset > tar.len() {
            return Err("The BlobUI source archive is truncated.".into());
        }
        // `git archive` emits global PAX metadata before its files. It contains
        // no filesystem entries and must not be mistaken for the archive root.
        if typeflag == b'g' {
            continue;
        }
        if typeflag == b'x' {
            let pax_data = &tar[data_start..data_end];
            if let Some(path) = parse_pax_path(pax_data)? {
                pending_pax_path = Some(path);
            }
            continue;
        }
        let name = tar_field(&header[0..100])?;
        let prefix = tar_field(&header[345..500])?;
        let header_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let full_name = pending_pax_path.take().unwrap_or(header_name);
        let path = clean_archive_path(&full_name)?;
        let (root, relative) = split_archive_root(&path)?;
        if let Some(previous_root) = &root_prefix {
            if previous_root != &root {
                return Err("The BlobUI archive contains inconsistent root folders.".into());
            }
        } else {
            root_prefix = Some(root);
        }
        let Some(relative) = relative else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target_kind = selected_archive_kind(&relative);
        if target_kind.is_none() {
            continue;
        }
        if !matches!(typeflag, 0 | b'0' | b'5') {
            return Err(
                "The BlobUI source archive contains an unsupported link or special file.".into(),
            );
        }
        if typeflag == b'5' {
            continue;
        }
        if !relative.extension().is_some_and(|extension| {
            target_kind != Some("Dev") || extension.eq_ignore_ascii_case("css")
        }) && target_kind == Some("Dev")
        {
            continue;
        }
        expanded_bytes = expanded_bytes.saturating_add(size);
        if expanded_bytes > MAX_EXPANDED_BYTES {
            return Err("The BlobUI source archive is larger than the supported limit.".into());
        }
        let relative_text = relative.to_string_lossy().replace('\\', "/");
        if !seen_paths.insert(relative_text.to_ascii_lowercase()) {
            return Err("The BlobUI source archive contains duplicate file paths.".into());
        }
        let root = root_prefix.as_ref().expect("root prefix set above");
        let target = destination.join(root).join(&relative);
        reject_reparse_path(&target)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| "Could not stage the downloaded BlobUI source.".to_string())?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|_| "Could not stage a file from the BlobUI source archive.".to_string())?;
        file.write_all(&tar[data_start..data_end])
            .map_err(|_| "Could not stage a file from the BlobUI source archive.".to_string())?;
    }
    if entry_count == 0 {
        return Err("The downloaded BlobUI source archive is empty.".into());
    }
    let root =
        root_prefix.ok_or_else(|| "The downloaded BlobUI source archive is empty.".to_string())?;
    let top = destination.join(root);
    let dev = top.join("Dev");
    let assets = top.join("assets/spacebanner/banner.png");
    let core = top.join("scripts/BlobUI.Core.psm1");
    if !dev.is_dir() || !assets.is_file() || !core.is_file() {
        return Err(
            "The downloaded source archive does not contain a complete BlobUI runtime.".into(),
        );
    }
    // Move the selected tree to a stable layout for the compiler and installer.
    for name in ["Dev", "assets", "scripts"] {
        let source = top.join(name);
        let target = destination.join(name);
        if source.exists() {
            fs::rename(&source, &target)
                .map_err(|_| "Could not prepare the downloaded BlobUI source.".to_string())?;
        }
    }
    let _ = fs::remove_dir_all(&top);
    Ok(())
}

fn selected_archive_kind(relative: &Path) -> Option<&'static str> {
    let mut components = relative.components();
    let first = components.next()?;
    match first {
        Component::Normal(name) if name == "Dev" => Some("Dev"),
        Component::Normal(name) if name == "assets" => Some("assets"),
        Component::Normal(name) if name == "scripts" => Some("scripts"),
        _ => None,
    }
}

fn split_archive_root(path: &Path) -> Result<(String, Option<PathBuf>), String> {
    let mut components = path.components();
    let root = match components.next() {
        Some(Component::Normal(root)) => root.to_string_lossy().into_owned(),
        _ => return Err("The BlobUI source archive contains an unsafe path.".into()),
    };
    if root.is_empty() || root.contains(':') {
        return Err("The BlobUI source archive contains an unsafe path.".into());
    }
    let relative = components.collect::<PathBuf>();
    Ok((root, Some(relative)))
}

fn clean_archive_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value.contains('\0')
    {
        return Err("The BlobUI source archive contains an unsafe path.".into());
    }
    let path = Path::new(value);
    for component in path.components() {
        match component {
            Component::Normal(part) if !part.is_empty() => {}
            _ => return Err("The BlobUI source archive contains an unsafe path.".into()),
        }
    }
    Ok(path.to_path_buf())
}

fn parse_pax_path(data: &[u8]) -> Result<Option<String>, String> {
    let mut offset = 0usize;
    let mut path = None;
    while offset < data.len() {
        let space = data[offset..]
            .iter()
            .position(|byte| *byte == b' ')
            .map(|position| offset + position)
            .ok_or_else(|| {
                "The BlobUI source archive contains invalid PAX metadata.".to_string()
            })?;
        let length_text = std::str::from_utf8(&data[offset..space])
            .map_err(|_| "The BlobUI source archive contains invalid PAX metadata.".to_string())?;
        let record_length = length_text
            .parse::<usize>()
            .map_err(|_| "The BlobUI source archive contains invalid PAX metadata.".to_string())?;
        let end = offset
            .checked_add(record_length)
            .filter(|end| *end <= data.len() && *end > space + 1)
            .ok_or_else(|| {
                "The BlobUI source archive contains invalid PAX metadata.".to_string()
            })?;
        if data[end - 1] != b'\n' {
            return Err("The BlobUI source archive contains invalid PAX metadata.".into());
        }
        let record = std::str::from_utf8(&data[space + 1..end - 1])
            .map_err(|_| "The BlobUI source archive contains invalid PAX metadata.".to_string())?;
        if let Some(value) = record.strip_prefix("path=") {
            path = Some(value.to_string());
        }
        offset = end;
    }
    Ok(path)
}

fn verify_tar_checksum(header: &[u8]) -> Result<(), String> {
    let stored = parse_tar_octal(&header[148..156])?;
    let checksum = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                b' ' as u64
            } else {
                *byte as u64
            }
        })
        .sum::<u64>();
    if stored != checksum {
        return Err("The downloaded BlobUI source archive failed its checksum.".into());
    }
    Ok(())
}

fn parse_tar_octal(bytes: &[u8]) -> Result<u64, String> {
    let text = bytes
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .filter(|byte| *byte != b' ')
        .collect::<Vec<_>>();
    if text.is_empty() {
        return Ok(0);
    }
    if text.iter().any(|byte| !(b'0'..=b'7').contains(byte)) {
        return Err("The downloaded BlobUI source archive has an invalid file size.".into());
    }
    let text = std::str::from_utf8(&text)
        .map_err(|_| "The downloaded BlobUI source archive is invalid.".to_string())?;
    u64::from_str_radix(text, 8)
        .map_err(|_| "The downloaded BlobUI source archive is invalid.".into())
}

fn tar_field(bytes: &[u8]) -> Result<String, String> {
    let bytes = &bytes[..bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len())];
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "The downloaded BlobUI source archive contains a non-text path.".to_string())
}

fn unique_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs;
    use std::path::Path;

    fn fixture_root(label: &str) -> PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run")
            .join(format!(
                "updater-{label}-{}-{}",
                std::process::id(),
                unique_nonce()
            ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn tar_entry(output: &mut Vec<u8>, name: &str, contents: &[u8], kind: u8) {
        let mut header = [0u8; 512];
        header[..name.len()].copy_from_slice(name.as_bytes());
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        let size = format!("{:011o}\0", contents.len());
        header[124..136].copy_from_slice(size.as_bytes());
        header[136..148].copy_from_slice(b"00000000000\0");
        header[148..156].fill(b' ');
        header[156] = kind;
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let checksum = header.iter().map(|byte| *byte as u64).sum::<u64>();
        let checksum_text = format!("{:06o}\0 ", checksum);
        header[148..156].copy_from_slice(checksum_text.as_bytes());
        output.extend_from_slice(&header);
        output.extend_from_slice(contents);
        let padding = (512 - contents.len() % 512) % 512;
        output.resize(output.len() + padding, 0);
    }

    fn archive(entries: &[(&str, &[u8], u8)]) -> Vec<u8> {
        let mut tar = Vec::new();
        for (name, contents, kind) in entries {
            tar_entry(&mut tar, name, contents, *kind);
        }
        tar.extend_from_slice(&[0; 1024]);
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        gzip.write_all(&tar).unwrap();
        gzip.finish().unwrap()
    }

    fn pax_path_record(path: &str) -> Vec<u8> {
        let payload = format!("path={path}\n");
        let mut length = payload.len() + 2;
        loop {
            let next = payload.len() + length.to_string().len() + 1;
            if next == length {
                break;
            }
            length = next;
        }
        format!("{length} {payload}").into_bytes()
    }

    fn source_archive(version: &str) -> Vec<u8> {
        let script = format!("$script:BlobUiReleaseVersion = '{version}'\n");
        let css_path = pax_path_record("Anytype-BlobUI-test/Dev/theme.css");
        archive(&[
            ("pax_global_header", b"comment=git archive fixture\n", b'g'),
            ("Anytype-BlobUI-test/pax-header", &css_path, b'x'),
            (
                "Anytype-BlobUI-test/Dev/truncated.css",
                b".release-theme { color: purple; }",
                b'0',
            ),
            (
                "Anytype-BlobUI-test/assets/spacebanner/banner.png",
                b"release-default-banner",
                b'0',
            ),
            (
                "Anytype-BlobUI-test/scripts/BlobUI.Core.psm1",
                script.as_bytes(),
                b'0',
            ),
            (
                "Anytype-BlobUI-test/scripts/Settings.ps1",
                b"# launcher",
                b'0',
            ),
            ("Anytype-BlobUI-test/scripts/Get-Data.ps1", b"# get", b'0'),
            (
                "Anytype-BlobUI-test/scripts/Compile.ps1",
                b"# compile",
                b'0',
            ),
            (
                "Anytype-BlobUI-test/scripts/Install.ps1",
                b"# install",
                b'0',
            ),
            (
                "Anytype-BlobUI-test/scripts/Rebuild.ps1",
                b"# rebuild",
                b'0',
            ),
            ("Anytype-BlobUI-test/scripts/Update.ps1", b"# update", b'0'),
            (
                "Anytype-BlobUI-test/scripts/Uninstall.ps1",
                b"# uninstall",
                b'0',
            ),
        ])
    }

    fn install_fixture(root: &Path) -> (PathBuf, Vec<u8>, Vec<u8>, Vec<u8>) {
        let install = root.join("Anytype data");
        let blob = install.join("blob");
        let data = json!({
            "installDirectory": install,
            "settings": {
                "fontFamily": "Inter", "codeFontFamily": "Consolas", "accentColor": "#2678B2",
                "bannerImage": "", "reducedMotion": false, "tabHoverEffect": false,
                "linkHoverEffect": false, "objectMentionHoverEffect": false
            },
            "apiConnection": { "enabled": false },
            "calendarObjectColors": {}
        });
        let data_bytes = serde_json::to_vec_pretty(&data).unwrap();
        let credential_bytes = b"dpapi-credential-sentinel".to_vec();
        let custom_banner = b"custom-banner-sentinel".to_vec();
        fs::create_dir_all(blob.join("runtime/Dev")).unwrap();
        fs::create_dir_all(blob.join("runtime/assets/spacebanner")).unwrap();
        fs::create_dir_all(blob.join("runtime/scripts")).unwrap();
        fs::create_dir_all(blob.join("assets/spacebanner")).unwrap();
        fs::create_dir_all(blob.join("credentials")).unwrap();
        fs::write(blob.join("data.json"), &data_bytes).unwrap();
        fs::write(
            blob.join("credentials/anytype-api-key.dpapi"),
            &credential_bytes,
        )
        .unwrap();
        fs::write(install.join("custom.css"), b"old-generated-css").unwrap();
        fs::write(blob.join("assets/spacebanner/banner.png"), &custom_banner).unwrap();
        fs::write(
            blob.join("runtime/Dev/local.css"),
            b".local-theme { color: teal; }",
        )
        .unwrap();
        fs::write(
            blob.join("runtime/Dev/old-managed.css"),
            b".obsolete { color: red; }",
        )
        .unwrap();
        fs::write(blob.join("runtime/scripts/old.ps1"), b"old managed script").unwrap();
        fs::write(blob.join("runtime/settings-app.exe"), b"running executable").unwrap();
        fs::write(blob.join("assets/obsolete.png"), b"obsolete managed asset").unwrap();
        let manifest = json!({
            "manifestVersion": 1,
            "releaseVersion": "1.0.0",
            "managedFiles": [
                "runtime/Dev/old-managed.css", "runtime/scripts/old.ps1", "assets/obsolete.png",
                "runtime/settings-app.exe"
            ]
        });
        fs::write(
            blob.join(".blobui-managed-files.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        (install, data_bytes, credential_bytes, custom_banner)
    }

    #[test]
    fn source_update_keeps_user_data_credentials_custom_banner_and_local_css() {
        let root = fixture_root("preserve");
        let (install, original_data, original_credential, original_banner) = install_fixture(&root);
        let version =
            apply_archive_to_install(&install, &source_archive("1.2.0"), "v1.2.0").unwrap();
        let blob = install.join("blob");
        assert_eq!(version, "1.2.0");
        assert_eq!(fs::read(blob.join("data.json")).unwrap(), original_data);
        assert_eq!(
            fs::read(blob.join("credentials/anytype-api-key.dpapi")).unwrap(),
            original_credential
        );
        assert_eq!(
            fs::read(blob.join("assets/spacebanner/banner.png")).unwrap(),
            original_banner
        );
        assert!(fs::read_to_string(install.join("custom.css"))
            .unwrap()
            .contains(".release-theme"));
        assert!(fs::read_to_string(install.join("custom.css"))
            .unwrap()
            .contains(".local-theme"));
        assert!(!blob.join("runtime/Dev/old-managed.css").exists());
        assert!(blob.join("runtime/Dev/local.css").is_file());
        assert!(!blob.join("runtime/scripts/old.ps1").exists());
        assert!(!blob.join("assets/obsolete.png").exists());
        assert_eq!(
            fs::read(blob.join("runtime/settings-app.exe")).unwrap(),
            b"running executable"
        );
        let manifest: Value =
            serde_json::from_slice(&fs::read(blob.join(".blobui-managed-files.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["releaseVersion"], "1.2.0");
        assert!(!manifest["managedFiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "runtime/Dev/local.css"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsafe_archive_paths_before_any_install_file_changes() {
        let root = fixture_root("traversal");
        let (install, original_data, original_credential, _) = install_fixture(&root);
        let malicious = archive(&[("Anytype-BlobUI-test/../escape.css", b"escape", b'0')]);
        assert!(apply_archive_to_install(&install, &malicious, "1.2.0").is_err());
        let blob = install.join("blob");
        assert_eq!(fs::read(blob.join("data.json")).unwrap(), original_data);
        assert_eq!(
            fs::read(blob.join("credentials/anytype-api-key.dpapi")).unwrap(),
            original_credential
        );
        assert_eq!(
            fs::read(install.join("custom.css")).unwrap(),
            b"old-generated-css"
        );
        assert!(!root.join("escape.css").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn updater_defaults_keep_auto_updates_off_and_persist_install_path() {
        let root = fixture_root("settings");
        let path = root.join("updater.json");
        let defaults = load_preferences(&path).unwrap();
        assert!(!defaults.automatic_updates_enabled);
        assert!(defaults.automatic_checks_enabled);
        let mut changed = defaults;
        changed.automatic_updates_enabled = true;
        changed.install_directory = Some("D:\\Anytype data".into());
        save_preferences(&path, &changed).unwrap();
        assert_eq!(
            load_preferences(&path)
                .unwrap()
                .install_directory
                .as_deref(),
            Some("D:\\Anytype data")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simulated_update_changes_only_returned_display_state() {
        let root = fixture_root("simulation");
        let app_data = root.join("APPDATA/anytype");
        let (install, original_data, original_credential, original_banner) = install_fixture(&root);
        let preferences = UpdaterPreferences {
            install_directory: Some(install.to_string_lossy().into_owned()),
            ..UpdaterPreferences::default()
        };
        let preferences_path = preferences_path(&app_data);
        save_preferences(&preferences_path, &preferences).unwrap();
        let original_preferences = fs::read(&preferences_path).unwrap();

        let state = simulate_update(&app_data, &install, &root).unwrap();
        assert_eq!(state.status, "simulated");
        assert!(state.update_available);
        assert_eq!(state.latest_version.as_deref(), Some("Simulation"));
        assert_eq!(fs::read(&preferences_path).unwrap(), original_preferences);
        assert_eq!(
            fs::read(install.join("blob/data.json")).unwrap(),
            original_data
        );
        assert_eq!(
            fs::read(install.join("blob/credentials/anytype-api-key.dpapi")).unwrap(),
            original_credential
        );
        assert_eq!(
            fs::read(install.join("custom.css")).unwrap(),
            b"old-generated-css"
        );
        assert_eq!(
            fs::read(install.join("blob/assets/spacebanner/banner.png")).unwrap(),
            original_banner
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_link_entries_in_selected_runtime_paths() {
        let root = fixture_root("links");
        let (install, _, _, _) = install_fixture(&root);
        let malicious = archive(&[("Anytype-BlobUI-test/Dev/link.css", b"target", b'2')]);
        assert!(apply_archive_to_install(&install, &malicious, "1.2.0").is_err());
        assert_eq!(
            fs::read(install.join("custom.css")).unwrap(),
            b"old-generated-css"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
