use chrono::{SecondsFormat, Utc};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage;

const OWNER: &str = "PainterFlow";
const REPOSITORY: &str = "Anytype-BlobUI";
const RELEASE_METADATA: &str = include_str!("../../blobui-release.json");
const MAX_RELEASE_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_INSTALLER_BYTES: usize = 64 * 1024 * 1024;
const MAX_RELEASE_NOTES_BYTES: u64 = 512 * 1024;
const MAX_RELEASE_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdaterPreferences {
    pub install_directory: Option<String>,
    pub automatic_updates_enabled: bool,
    pub automatic_checks_enabled: bool,
    pub last_checked_at: Option<String>,
    pub latest_version: Option<String>,
    pub last_failed_install_version: Option<String>,
    pub last_check_status: Option<String>,
    pub last_message: Option<String>,
    pub pending_changelog_version: Option<String>,
}

impl Default for UpdaterPreferences {
    fn default() -> Self {
        Self {
            install_directory: None,
            automatic_updates_enabled: false,
            automatic_checks_enabled: true,
            last_checked_at: None,
            latest_version: None,
            last_failed_install_version: None,
            last_check_status: None,
            last_message: None,
            pending_changelog_version: None,
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
    pub automatic_update_blocked: bool,
    pub last_checked_at: Option<String>,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNotes {
    pub version: String,
    pub tag: String,
    pub markdown: String,
    pub image_manifest: String,
}

#[derive(Clone, Debug)]
struct ReleaseInfo {
    tag: String,
    version: semver::Version,
    installer_url: String,
    installer_size: u64,
    installer_digest: Option<String>,
}

#[derive(Clone, Debug)]
struct PreviousManifest {
    release_version: Option<String>,
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

pub fn recover_install_directory(
    app_data_directory: &Path,
    selected_directory: &Path,
) -> Result<(), String> {
    let install_directory = validate_install_directory(selected_directory)?;
    let path = preferences_path(app_data_directory);
    let mut preferences = match load_preferences(&path) {
        Ok(preferences) => preferences,
        Err(error) if error == "The saved BlobUI updater settings are not valid JSON." => {
            UpdaterPreferences::default()
        }
        Err(error) => return Err(error),
    };
    preferences.install_directory = Some(install_directory.to_string_lossy().into_owned());
    save_preferences(&path, &preferences)
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
    let automatic_update_blocked = update_available
        && latest.as_ref().is_some_and(|version| {
            preferences.last_failed_install_version.as_deref() == Some(version.to_string().as_str())
        });
    let status = if preferences.last_check_status.as_deref() == Some("error") {
        "error"
    } else if update_available {
        "available"
    } else {
        preferences.last_check_status.as_deref().unwrap_or("idle")
    };
    let message = if automatic_update_blocked {
        Some(format!(
            "BlobUI version {} is available. Automatic retry is paused after the previous install failed; choose Update now to retry.",
            latest.as_ref().unwrap()
        ))
    } else if update_available {
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
        automatic_update_blocked,
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

pub fn get_release_notes(
    app_data_directory: &Path,
    install_directory: &Path,
    runtime_directory: &Path,
) -> Result<Option<ReleaseNotes>, String> {
    let preferences = load_preferences(&preferences_path(app_data_directory))?;
    let Some(version) = preferences.pending_changelog_version else {
        return Ok(None);
    };
    if parse_version(&version).is_none() || current_version(install_directory, Some(runtime_directory))? != version {
        return Ok(None);
    }
    let markdown = read_release_text(&runtime_directory.join("Changelogs.md"), MAX_RELEASE_NOTES_BYTES)?;
    let image_manifest = read_release_text(&runtime_directory.join("Changelogs.yaml"), MAX_RELEASE_NOTES_BYTES)?;
    Ok(Some(ReleaseNotes { tag: version.clone(), version, markdown, image_manifest }))
}

pub fn get_latest_release_notes(
    install_directory: &Path,
    runtime_directory: &Path,
) -> Result<Option<ReleaseNotes>, String> {
    let runtime_directory = if runtime_directory.join("Changelogs.md").is_file() {
        runtime_directory
    } else {
        return Ok(None);
    };
    let markdown = read_release_text(&runtime_directory.join("Changelogs.md"), MAX_RELEASE_NOTES_BYTES)?;
    if markdown.trim().is_empty() {
        return Ok(None);
    }
    let manifest_path = runtime_directory.join("Changelogs.yaml");
    let image_manifest = if manifest_path.is_file() {
        read_release_text(&manifest_path, MAX_RELEASE_NOTES_BYTES)?
    } else {
        "images: []\n".into()
    };
    let version = current_version(install_directory, Some(runtime_directory))?;
    Ok(Some(ReleaseNotes { tag: version.clone(), version, markdown, image_manifest }))
}

pub fn get_adjacent_release_notes(version: &str, older: bool) -> Result<Option<ReleaseNotes>, String> {
    let Some(current) = parse_version(version) else {
        return Err("The release notes version is invalid.".into());
    };
    let mut releases = Vec::new();
    for page in 1..=10 {
        let response = http_get(
            "api.github.com",
            &format!("/repos/{OWNER}/{REPOSITORY}/releases?per_page=100&page={page}"),
            "application/vnd.github+json",
            MAX_RELEASE_JSON_BYTES,
        )?;
        if response.status != 200 {
            return Err(format!("GitHub release history returned HTTP {}.", response.status));
        }
        let page_releases: Value = serde_json::from_slice(&response.body)
            .map_err(|_| "GitHub returned an invalid release history.".to_string())?;
        let page_releases = page_releases.as_array()
            .ok_or_else(|| "GitHub returned an invalid release history.".to_string())?;
        let count = page_releases.len();
        releases.extend(page_releases.iter().cloned());
        if count < 100 { break; }
    }
    let mut versions = releases.iter().filter_map(|release| {
        let tag = release.get("tag_name")?.as_str()?;
        let parsed = parse_version(tag)?;
        if parsed.pre.is_empty()
            && tag.len() <= 64
            && tag.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'v'))
        {
            Some((parsed, tag.to_string()))
        } else {
            None
        }
    }).collect::<Vec<_>>();
    versions.sort_by(|left, right| left.0.cmp(&right.0));
    let selected = if older {
        versions.iter().filter(|(candidate, _)| candidate < &current).max_by(|left, right| left.0.cmp(&right.0))
    } else {
        versions.iter().filter(|(candidate, _)| candidate > &current).min_by(|left, right| left.0.cmp(&right.0))
    };
    let Some((_, tag)) = selected else { return Ok(None); };
    let release = releases.iter().find(|release| release.get("tag_name").and_then(Value::as_str) == Some(tag.as_str()));
    fetch_release_notes_for_tag(tag, release).map(Some)
}

pub fn release_note_history_image(tag: &str, source: &str) -> Result<String, String> {
    if tag.is_empty() || tag.len() > 64 || parse_version(tag).is_none()
        || !tag.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'v'))
    { return Err("The release notes version is invalid.".into()); }
    let relative = release_image_relative_path(source)?;
    let response = http_get(
        "raw.githubusercontent.com",
        &format!("/{OWNER}/{REPOSITORY}/{tag}/assets/changelogs/{relative}"),
        "image/png,image/jpeg,image/gif,image/webp",
        MAX_RELEASE_IMAGE_BYTES as usize,
    )?;
    if response.status != 200 {
        return Err("The release note image could not be read from GitHub.".into());
    }
    encode_release_image(&response.body, &relative)
}

fn fetch_release_notes_for_tag(tag: &str, release: Option<&Value>) -> Result<ReleaseNotes, String> {
    if tag.is_empty() || tag.len() > 64
        || !tag.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'v'))
    {
        return Err("The GitHub release tag is unsafe.".into());
    }
    let markdown = fetch_raw_release_file(tag, "Changelogs.md", MAX_RELEASE_NOTES_BYTES as usize)
        .or_else(|_| release.and_then(|item| item.get("body").and_then(Value::as_str).map(str::to_string))
            .ok_or_else(|| "The GitHub release notes could not be read.".to_string()))?;
    if markdown.trim().is_empty() {
        return Err("The GitHub release notes are empty.".into());
    }
    let image_manifest = fetch_raw_release_file(tag, "Changelogs.yaml", MAX_RELEASE_NOTES_BYTES as usize)
        .unwrap_or_else(|_| "images: []\n".into());
    Ok(ReleaseNotes { version: parse_version(tag).unwrap().to_string(), tag: tag.to_string(), markdown, image_manifest })
}

fn fetch_raw_release_file(tag: &str, filename: &str, max_bytes: usize) -> Result<String, String> {
    let response = http_get(
        "raw.githubusercontent.com",
        &format!("/{OWNER}/{REPOSITORY}/{tag}/{filename}"),
        "text/plain",
        max_bytes,
    )?;
    if response.status != 200 {
        return Err(format!("GitHub returned HTTP {} for the release notes.", response.status));
    }
    String::from_utf8(response.body).map_err(|_| "The GitHub release notes are not valid UTF-8.".into())
}

fn release_image_relative_path(source: &str) -> Result<&str, String> {
    let relative = source.strip_prefix("assets/changelogs/")
        .ok_or_else(|| "The release note image source is outside the changelog image folder.".to_string())?;
    let segments = relative.split('/').collect::<Vec<_>>();
    if segments.is_empty() || segments.iter().any(|segment| segment.is_empty() || *segment == "." || *segment == ".."
        || !segment.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    {
        return Err("The release note image source contains an unsafe path.".into());
    }
    Ok(relative)
}

fn encode_release_image(bytes: &[u8], filename: &str) -> Result<String, String> {
    let extension = filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => "image/png",
        "jpg" | "jpeg" if bytes.starts_with(&[0xff, 0xd8, 0xff]) => "image/jpeg",
        "gif" if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") => "image/gif",
        "webp" if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" => "image/webp",
        _ => return Err("The release note image bytes do not match a supported image format.".into()),
    };
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

pub fn release_note_image(runtime_directory: &Path, source: &str) -> Result<String, String> {
    let relative = release_image_relative_path(source)?;
    let path = runtime_directory.join("assets/changelogs").join(relative);
    storage::reject_reparse_path(&path)?;
    let metadata = fs::metadata(&path)
        .map_err(|_| "The release note image could not be read.".to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_RELEASE_IMAGE_BYTES {
        return Err("The release note image has an unsupported size or file type.".into());
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "The release note image format is unsupported.".to_string())?;
    let bytes = fs::read(&path).map_err(|_| "The release note image could not be read.".to_string())?;
    encode_release_image(&bytes, &extension)
}

pub fn acknowledge_release_notes(
    app_data_directory: &Path,
    install_directory: &Path,
    runtime_directory: &Path,
    version: &str,
) -> Result<bool, String> {
    let Some(version) = parse_version(version).map(|version| version.to_string()) else {
        return Err("The release notes version is invalid.".into());
    };
    if current_version(install_directory, Some(runtime_directory))? != version {
        return Ok(false);
    }
    let path = preferences_path(app_data_directory);
    let mut preferences = load_preferences(&path)?;
    if preferences.pending_changelog_version.as_deref() != Some(version.as_str()) {
        return Ok(false);
    }
    preferences.pending_changelog_version = None;
    save_preferences(&path, &preferences)?;
    Ok(true)
}

fn read_release_text(path: &Path, limit: u64) -> Result<String, String> {
    storage::reject_reparse_path(path)?;
    let metadata = fs::metadata(path)
        .map_err(|_| "The installed release notes are missing or unreadable.".to_string())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err("The installed release notes exceed the supported size.".into());
    }
    fs::read_to_string(path).map_err(|_| "The installed release notes are not valid UTF-8.".into())
}

pub fn apply_update(
    app_data_directory: &Path,
    install_directory: &Path,
    fallback_runtime: &Path,
    automatic: bool,
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
    let release_version = release.version.to_string();
    if automatic && preferences.last_failed_install_version.as_deref() == Some(&release_version) {
        preferences.last_check_status = Some("available".into());
        preferences.last_message = Some(format!(
            "BlobUI version {release_version} is available. Automatic retry is paused after the previous install failed; choose Update now to retry."
        ));
        save_preferences(&path, &preferences)?;
        return state_for(install_directory, &preferences, Some(fallback_runtime));
    }
    let current_version = parse_version(&current)
        .ok_or_else(|| "The installed BlobUI release version is invalid.".to_string())?;
    if release.version <= current_version {
        preferences.last_check_status = Some("current".into());
        preferences.last_message = Some("BlobUI is already up to date.".into());
        save_preferences(&path, &preferences)?;
        return state_for(install_directory, &preferences, Some(fallback_runtime));
    }
    if !automatic {
        preferences.last_failed_install_version = None;
    }

    let result = (|| {
        let app = resolve_app_directory(
            install_directory,
            std::env::current_exe().ok().as_deref(),
            std::env::var_os("LOCALAPPDATA").as_deref().map(Path::new),
        )?;
        let installer_bytes = download_release_installer(&release)?;
        let mut staged = stage_installer(install_directory, &installer_bytes)?;
        preferences.last_check_status = Some("installing".into());
        preferences.last_message = Some(format!(
            "BlobUI Settings is restarting to install version {}.",
            release.version
        ));
        save_preferences(&path, &preferences)?;
        // Construct the complete response before starting the installer. Once
        // it is running, the app must have no remaining fallible work before
        // it can exit and release the executable for replacement.
        let mut state = state_for(install_directory, &preferences, Some(fallback_runtime))?;
        state.status = "restarting".into();
        state.message = Some(format!(
            "BlobUI Settings is restarting to install version {}.",
            release.version
        ));
        launch_update_installer(&staged.executable, &app, install_directory, &release.tag)?;
        staged.retain = true;
        Ok(state)
    })();
    match result {
        Ok(state) => Ok(state),
        Err(error) => {
            preferences.latest_version = Some(release.version.to_string());
            preferences.last_check_status = Some("error".into());
            preferences.last_message = Some(error);
            save_preferences(&path, &preferences)?;
            state_for(install_directory, &preferences, Some(fallback_runtime))
        }
    }
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
    version_from_metadata(RELEASE_METADATA)
        .map(|version| version.to_string())
        .ok_or_else(|| "The built-in BlobUI release metadata is invalid.".into())
}

fn version_from_runtime(runtime: &Path) -> Option<semver::Version> {
    version_from_metadata(&fs::read_to_string(runtime.join("blobui-release.json")).ok()?)
}

fn version_from_metadata(text: &str) -> Option<semver::Version> {
    let metadata: Value = serde_json::from_str(text).ok()?;
    if metadata.get("formatVersion")?.as_u64()? != 1 {
        return None;
    }
    parse_version(metadata.get("version")?.as_str()?)
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
    release_info_from_record(&release).map(Some)
}

fn release_info_from_record(release: &Value) -> Result<ReleaseInfo, String> {
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "GitHub's release record has no tag name.".to_string())?;
    let version = parse_version(tag).ok_or_else(|| {
        "The latest GitHub release tag is not a supported semantic version.".to_string()
    })?;
    if tag.is_empty()
        || tag.len() > 64
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err("The latest GitHub release tag is unsafe.".into());
    }
    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| "The BlobUI release has no installer assets.".to_string())?;
    let mut candidates = assets
        .iter()
        .filter(|asset| asset.get("name").and_then(Value::as_str) == Some("blobui-installer.exe"));
    let asset = candidates
        .next()
        .ok_or_else(|| "The BlobUI release does not include blobui-installer.exe.".to_string())?;
    if candidates.next().is_some() {
        return Err("The BlobUI release contains duplicate installer assets.".into());
    }
    let installer_url = asset
        .get("browser_download_url")
        .and_then(Value::as_str)
        .ok_or_else(|| "The BlobUI release installer URL is missing.".to_string())?;
    validate_installer_url(installer_url, tag)?;
    let installer_size = asset
        .get("size")
        .and_then(Value::as_u64)
        .filter(|size| *size > 0 && *size <= MAX_INSTALLER_BYTES as u64)
        .ok_or_else(|| "The BlobUI release installer has an invalid size.".to_string())?;
    let installer_digest = parse_asset_digest(asset.get("digest"))?;
    Ok(ReleaseInfo {
        tag: tag.to_string(),
        version,
        installer_url: installer_url.to_string(),
        installer_size,
        installer_digest,
    })
}

fn validate_installer_url(value: &str, tag: &str) -> Result<(), String> {
    let parsed = url::Url::parse(value)
        .map_err(|_| "The BlobUI release installer URL is invalid.".to_string())?;
    let expected_path =
        format!("/{OWNER}/{REPOSITORY}/releases/download/{tag}/blobui-installer.exe");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some_and(|port| port != 443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != expected_path
    {
        return Err("The BlobUI release points to an unexpected installer location.".into());
    }
    Ok(())
}

fn parse_asset_digest(value: Option<&Value>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let digest = value
        .as_str()
        .ok_or_else(|| "The BlobUI release installer digest is invalid.".to_string())?;
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| "The BlobUI release installer uses an unsupported digest.".to_string())?;
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("The BlobUI release installer digest is invalid.".into());
    }
    Ok(Some(hex.to_ascii_lowercase()))
}

fn download_release_installer(release: &ReleaseInfo) -> Result<Vec<u8>, String> {
    validate_installer_url(&release.installer_url, &release.tag)?;
    let parsed = url::Url::parse(&release.installer_url)
        .map_err(|_| "The BlobUI release installer URL is invalid.".to_string())?;
    let response = http_get(
        parsed.host_str().expect("validated URL host"),
        parsed.path(),
        "application/octet-stream",
        MAX_INSTALLER_BYTES,
    )?;
    if response.status != 200 {
        return Err(format!(
            "GitHub returned HTTP {} while downloading the BlobUI installer.",
            response.status
        ));
    }
    if response.body.len() as u64 != release.installer_size {
        return Err(
            "The downloaded BlobUI installer size does not match GitHub's release record.".into(),
        );
    }
    if let Some(expected) = release.installer_digest.as_deref() {
        let actual = Sha256::digest(&response.body)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != expected {
            return Err("The downloaded BlobUI installer failed its SHA-256 check.".into());
        }
    }
    Ok(response.body)
}

pub(crate) fn resolve_app_directory(
    install_directory: &Path,
    current_exe: Option<&Path>,
    local_app_data: Option<&Path>,
) -> Result<PathBuf, String> {
    let install = validate_install_directory(install_directory)?;
    let mut found_mismatched_pointer = false;
    if let Some(current_exe) = current_exe {
        reject_reparse_path(current_exe)?;
        for ancestor in current_exe.parent().into_iter().flat_map(Path::ancestors) {
            let pointer_path = ancestor.join(".blobui-install.json");
            match fs::symlink_metadata(&pointer_path) {
                Ok(_) => {
                    if let Some(app) =
                        validate_app_pointer(ancestor, &pointer_path, &install)?
                    {
                        return Ok(app);
                    }
                    found_mismatched_pointer = true;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(
                        "The running Settings app install pointer could not be inspected safely."
                            .into(),
                    )
                }
            }
        }
    }
    if let Some(app) = app_directory_from_install_manifest(&install)? {
        let pointer = app.join(".blobui-install.json");
        if let Some(app) = validate_app_pointer(&app, &pointer, &install)? {
            return Ok(app);
        }
        found_mismatched_pointer = true;
    }
    if let Some(local_app_data) = local_app_data {
        let app = local_app_data.join("Programs/BlobUI");
        let pointer = app.join(".blobui-install.json");
        match fs::symlink_metadata(&pointer) {
            Ok(_) => {
                if let Some(app) = validate_app_pointer(&app, &pointer, &install)? {
                    return Ok(app);
                }
                found_mismatched_pointer = true;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(
                    "The installed BlobUI app pointer could not be inspected safely.".into(),
                )
            }
        }
    }
    if found_mismatched_pointer {
        Err("The installed BlobUI app belongs to a different Anytype folder.".into())
    } else {
        Err(
            "The installed BlobUI app folder could not be found. Run the installer once before using self-updates."
                .into(),
        )
    }
}

fn app_directory_from_install_manifest(install: &Path) -> Result<Option<PathBuf>, String> {
    let path = install.join("blob/.blobui-managed-files.json");
    let bytes = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            reject_reparse_path(&path)?;
            fs::read(&path).map_err(|_| {
                "The selected BlobUI runtime manifest could not be read safely.".to_string()
            })?
        }
        Ok(_) => {
            return Err("The selected BlobUI runtime manifest is not a regular file.".into())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err("The selected BlobUI runtime manifest could not be inspected safely.".into())
        }
    };
    let manifest: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "The selected BlobUI runtime manifest is invalid.".to_string())?;
    match manifest.get("appDirectory") {
        None => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(PathBuf::from(value)))
        }
        Some(_) => Err("The selected BlobUI app directory pointer is invalid.".into()),
    }
}

fn validate_app_pointer(
    app: &Path,
    pointer_path: &Path,
    install: &Path,
) -> Result<Option<PathBuf>, String> {
    reject_reparse_path(app)?;
    reject_reparse_path(pointer_path)?;
    let app = app
        .canonicalize()
        .map_err(|_| "The installed BlobUI app folder could not be resolved.".to_string())?;
    let pointer: Value = serde_json::from_slice(
        &fs::read(pointer_path)
            .map_err(|_| "The installed BlobUI app pointer could not be read.".to_string())?,
    )
    .map_err(|_| "The installed BlobUI app pointer is invalid.".to_string())?;
    let stored = pointer
        .get("installDirectory")
        .and_then(Value::as_str)
        .ok_or_else(|| "The installed BlobUI app pointer is invalid.".to_string())?;
    let stored = Path::new(stored)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(stored));
    if !same_path(&stored, install) {
        return Ok(None);
    }
    for relative in [
        "blobui-settings.exe",
        "settings-app/dist/blobui-settings.exe",
    ] {
        let executable = app.join(relative);
        reject_reparse_path(&executable)?;
        if !fs::symlink_metadata(&executable).is_ok_and(|metadata| metadata.is_file()) {
            return Err("The selected BlobUI app folder is missing a Settings executable.".into());
        }
    }
    Ok(Some(app))
}

pub(crate) fn cleanup_staged_installers(install_directory: &Path) {
    let Ok(install) = validate_install_directory(install_directory) else {
        return;
    };
    let staging = install.join("blob/staging");
    if reject_reparse_path(&staging).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(&staging) else {
        return;
    };

    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".blobui-update-")
        {
            continue;
        }
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            continue;
        }
        let directory = entry.path();
        if reject_reparse_path(&directory).is_err() {
            continue;
        }

        // The staged executable may still be mapped by the installer that
        // reopened Settings. Retry briefly until Windows releases that file.
        for attempt in 0..20 {
            match fs::remove_dir_all(&directory) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(_) if attempt < 19 => {
                    std::thread::sleep(std::time::Duration::from_millis(250))
                }
                Err(_) => break,
            }
        }
    }
}

struct StagedInstaller {
    directory: PathBuf,
    executable: PathBuf,
    retain: bool,
}

impl Drop for StagedInstaller {
    fn drop(&mut self) {
        if !self.retain {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
}

fn stage_installer(install: &Path, bytes: &[u8]) -> Result<StagedInstaller, String> {
    let install = validate_install_directory(install)?;
    let staging = install.join("blob/staging");
    reject_reparse_path(&staging)?;
    fs::create_dir_all(&staging)
        .map_err(|_| "Could not prepare the BlobUI installer staging folder.".to_string())?;
    reject_reparse_path(&staging)?;
    let directory = staging.join(format!(
        ".blobui-update-{}-{:x}",
        std::process::id(),
        unique_nonce()
    ));
    fs::create_dir(&directory)
        .map_err(|_| "Could not create a private BlobUI installer staging folder.".to_string())?;
    let executable = directory.join("blobui-installer.exe");
    let result = (|| {
        reject_reparse_path(&directory)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&executable)
            .map_err(|_| "Could not stage the downloaded BlobUI installer.".to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Could not finish staging the downloaded BlobUI installer.".to_string())?;
        reject_reparse_path(&executable)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&directory);
        return Err(error);
    }
    Ok(StagedInstaller {
        directory,
        executable,
        retain: false,
    })
}

fn launch_update_installer(
    executable: &Path,
    app: &Path,
    install: &Path,
    tag: &str,
) -> Result<(), String> {
    validate_installer_url(
        &format!(
            "https://github.com/{OWNER}/{REPOSITORY}/releases/download/{tag}/blobui-installer.exe"
        ),
        tag,
    )?;
    reject_reparse_path(executable)?;
    if !fs::symlink_metadata(executable).is_ok_and(|metadata| metadata.is_file()) {
        return Err("The staged BlobUI installer is missing.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut child = Command::new(executable)
            .arg("--no-pause")
            .arg("--update-mode")
            .arg("--release-tag")
            .arg(tag)
            .arg("--app-directory")
            .arg(app)
            .arg("--anytype-directory")
            .arg(install)
            .arg("--wait-pid")
            .arg(std::process::id().to_string())
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|_| "Could not start the verified BlobUI installer.".to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(750));
        if let Some(status) = child
            .try_wait()
            .map_err(|_| "Could not verify that the BlobUI installer started.".to_string())?
        {
            return Err(format!(
                "The BlobUI installer exited before starting the update ({status})."
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (executable, app, install, tag);
        Err("The BlobUI installer is supported on Windows.".into())
    }
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
    for item in list {
        let raw = item.as_str().ok_or_else(|| {
            "The saved BlobUI install manifest contains an invalid file path.".to_string()
        })?;
        validate_managed_key(raw)?;
    }
    let release_version = value
        .get("releaseVersion")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(PreviousManifest {
        release_version,
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
        || key == "runtime/settings-app.exe"
        || key == "runtime/blobui-release.json"
        || key == "runtime/Changelogs.md"
        || key == "runtime/Changelogs.yaml")
    {
        return Err("The BlobUI install manifest contains an unsupported file path.".into());
    }
    Ok(Some(key))
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
    let data: Value = crate::parse_settings_json(&bytes)
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

fn unique_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn release_record_accepts_only_the_pinned_official_installer_asset() {
        let record = json!({
            "tag_name": "v1.2.0",
            "assets": [{
                "name": "blobui-installer.exe",
                "browser_download_url": "https://github.com/PainterFlow/Anytype-BlobUI/releases/download/v1.2.0/blobui-installer.exe",
                "size": 125,
                "digest": null
            }]
        });
        let parsed = release_info_from_record(&record).unwrap();
        assert_eq!(parsed.tag, "v1.2.0");
        assert_eq!(parsed.version, semver::Version::parse("1.2.0").unwrap());
        assert_eq!(parsed.installer_size, 125);
        assert_eq!(parsed.installer_digest, None);

        let mut forged = record.clone();
        forged["assets"][0]["browser_download_url"] =
            json!("https://example.invalid/blobui-installer.exe");
        assert!(release_info_from_record(&forged)
            .unwrap_err()
            .contains("unexpected installer location"));
        let mut wrong_tag = record.clone();
        wrong_tag["assets"][0]["browser_download_url"] = json!("https://github.com/PainterFlow/Anytype-BlobUI/releases/download/v1.2.1/blobui-installer.exe");
        assert!(release_info_from_record(&wrong_tag)
            .unwrap_err()
            .contains("unexpected installer location"));
        let mut bad_size = record.clone();
        bad_size["assets"][0]["size"] = json!(MAX_INSTALLER_BYTES as u64 + 1);
        assert!(release_info_from_record(&bad_size)
            .unwrap_err()
            .contains("invalid size"));
        let mut bad_digest = record;
        bad_digest["assets"][0]["digest"] = json!("sha256:bad");
        assert!(release_info_from_record(&bad_digest)
            .unwrap_err()
            .contains("digest is invalid"));
    }

    #[test]
    fn failed_release_blocks_automatic_retry_but_keeps_manual_update_available() {
        let root = fixture_root("failed-auto-retry");
        let (install, _, _, _) = install_fixture(&root);
        let fallback = root.join("runtime");
        let mut preferences = UpdaterPreferences {
            latest_version: Some("1.2.0".into()),
            last_failed_install_version: Some("1.2.0".into()),
            last_check_status: Some("available".into()),
            ..UpdaterPreferences::default()
        };

        let state = state_for(&install, &preferences, Some(&fallback)).unwrap();
        assert!(state.update_available);
        assert!(state.automatic_update_blocked);
        assert!(state.message.unwrap().contains("choose Update now to retry"));

        preferences.latest_version = Some("1.3.0".into());
        let newer = state_for(&install, &preferences, Some(&fallback)).unwrap();
        assert!(newer.update_available);
        assert!(!newer.automatic_update_blocked);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_only_the_app_pointer_for_the_selected_anytype_install() {
        let root = fixture_root("app-pointer");
        let install = root.join("Anytype data");
        let blob = install.join("blob");
        fs::create_dir_all(&blob).unwrap();
        let mut settings = vec![0xEF, 0xBB, 0xBF];
        settings.extend_from_slice(
            &serde_json::to_vec(&json!({"installDirectory": install})).unwrap(),
        );
        fs::write(blob.join("data.json"), settings).unwrap();
        assert_eq!(
            validate_install_directory(&install).unwrap(),
            install.canonicalize().unwrap()
        );
        let local_app_data = root.join("LocalAppData");
        let app = local_app_data.join("Programs/BlobUI");
        fs::create_dir_all(app.join("settings-app/dist")).unwrap();
        fs::write(app.join("blobui-settings.exe"), b"root exe").unwrap();
        fs::write(
            app.join("settings-app/dist/blobui-settings.exe"),
            b"nested exe",
        )
        .unwrap();
        fs::write(
            app.join(".blobui-install.json"),
            serde_json::to_vec(&json!({"installDirectory": install})).unwrap(),
        )
        .unwrap();
        let workspace_exe = Path::new(env!("CARGO_MANIFEST_DIR")).join("dist/blobui-settings.exe");
        let resolved =
            resolve_app_directory(&install, Some(&workspace_exe), Some(&local_app_data)).unwrap();
        assert_eq!(resolved, app.canonicalize().unwrap());

        fs::write(
            app.join(".blobui-install.json"),
            br#"{"installDirectory":"C:\\Elsewhere"}"#,
        )
        .unwrap();
        assert!(
            resolve_app_directory(&install, Some(&workspace_exe), Some(&local_app_data))
                .unwrap_err()
                .contains("different Anytype folder")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_repairs_invalid_preferences_and_preserves_valid_options() {
        let root = fixture_root("profile-recovery");
        let app_data = root.join("APPDATA/anytype");
        let first_install = root.join("First Anytype");
        let second_install = root.join("Second Anytype");
        for install in [&first_install, &second_install] {
            fs::create_dir_all(install.join("blob")).unwrap();
            fs::write(
                install.join("blob/data.json"),
                serde_json::to_vec(&json!({"installDirectory": install})).unwrap(),
            )
            .unwrap();
        }

        let path = preferences_path(&app_data);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"invalid-json").unwrap();
        recover_install_directory(&app_data, &first_install).unwrap();
        let repaired = load_preferences(&path).unwrap();
        assert_eq!(
            repaired.install_directory.as_deref(),
            Some(first_install.to_string_lossy().as_ref())
        );
        assert!(!repaired.automatic_updates_enabled);
        assert!(repaired.automatic_checks_enabled);

        let mut preferences = repaired;
        preferences.automatic_updates_enabled = true;
        preferences.automatic_checks_enabled = false;
        save_preferences(&path, &preferences).unwrap();
        recover_install_directory(&app_data, &second_install).unwrap();
        let preserved = load_preferences(&path).unwrap();
        assert_eq!(
            preserved.install_directory.as_deref(),
            Some(second_install.to_string_lossy().as_ref())
        );
        assert!(preserved.automatic_updates_enabled);
        assert!(!preserved.automatic_checks_enabled);
        fs::remove_dir_all(root).unwrap();
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
    fn release_metadata_validates_schema_and_semantic_version() {
        assert_eq!(
            version_from_metadata(RELEASE_METADATA).unwrap().to_string(),
            "1.2.0"
        );
        for invalid in [
            r#"{"formatVersion":2,"version":"1.2.0"}"#,
            r#"{"formatVersion":1,"version":"invalid"}"#,
            r#"{"version":"1.2.0"}"#,
            "invalid json",
        ] {
            assert!(version_from_metadata(invalid).is_none());
        }
    }

    #[test]
    fn release_notes_are_available_only_for_the_installed_pending_version_and_ack_on_close() {
        let root = fixture_root("release-notes");
        let app_data = root.join("APPDATA/anytype");
        let (install, _, _, _) = install_fixture(&root);
        let runtime = install.join("blob/runtime");
        fs::write(runtime.join("Changelogs.md"), "# BlobUI 1.0.0\n\nUpdated settings.\n").unwrap();
        fs::write(runtime.join("Changelogs.yaml"), "images: []\n").unwrap();
        let path = preferences_path(&app_data);
        let mut preferences = UpdaterPreferences {
            pending_changelog_version: Some("1.0.0".into()),
            ..UpdaterPreferences::default()
        };
        save_preferences(&path, &preferences).unwrap();

        let notes = get_release_notes(&app_data, &install, &runtime).unwrap().unwrap();
        assert_eq!(notes.version, "1.0.0");
        assert!(notes.markdown.contains("Updated settings."));
        assert_eq!(notes.image_manifest, "images: []\n");
        assert_eq!(load_preferences(&path).unwrap().pending_changelog_version.as_deref(), Some("1.0.0"));

        assert!(acknowledge_release_notes(&app_data, &install, &runtime, "1.0.0").unwrap());
        assert!(get_release_notes(&app_data, &install, &runtime).unwrap().is_none());
        assert_eq!(load_preferences(&path).unwrap().pending_changelog_version, None);

        preferences.pending_changelog_version = Some("1.1.0".into());
        save_preferences(&path, &preferences).unwrap();
        assert!(get_release_notes(&app_data, &install, &runtime).unwrap().is_none());
        assert!(!acknowledge_release_notes(&app_data, &install, &runtime, "1.1.0").unwrap());
        assert_eq!(load_preferences(&path).unwrap().pending_changelog_version.as_deref(), Some("1.1.0"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn latest_release_notes_do_not_require_or_consume_a_pending_update() {
        let root = fixture_root("latest-release-notes");
        let (install, _, _, _) = install_fixture(&root);
        let app_data = root.join("APPDATA/anytype");
        let runtime = install.join("blob/runtime");
        fs::write(runtime.join("Changelogs.md"), "# Latest notes\n").unwrap();
        fs::write(runtime.join("Changelogs.yaml"), "images: []\n").unwrap();
        let notes = get_latest_release_notes(&install, &runtime).unwrap().unwrap();
        assert_eq!(notes.version, "1.0.0");
        assert_eq!(notes.markdown, "# Latest notes\n");
        assert_eq!(notes.image_manifest, "images: []\n");
        assert_eq!(load_preferences(&preferences_path(&app_data)).unwrap().pending_changelog_version, None);
        fs::remove_file(runtime.join("Changelogs.yaml")).unwrap();
        assert_eq!(get_latest_release_notes(&install, &runtime).unwrap().unwrap().image_manifest, "images: []\n");
        fs::write(runtime.join("Changelogs.md"), "  \n").unwrap();
        assert!(get_latest_release_notes(&install, &runtime).unwrap().is_none());
        fs::write(runtime.join("Changelogs.md"), "# Latest notes\n").unwrap();
        fs::remove_file(runtime.join("Changelogs.md")).unwrap();
        assert!(get_latest_release_notes(&install, &runtime).unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn release_note_images_are_confined_size_limited_and_signature_checked() {
        let root = fixture_root("release-note-images");
        let runtime = root.join("runtime");
        let image = runtime.join("assets/changelogs/setup.png");
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        let bytes = b"\x89PNG\r\n\x1a\nsmall-png-fixture";
        fs::write(&image, bytes).unwrap();

        let data_url = release_note_image(&runtime, "assets/changelogs/setup.png").unwrap();
        assert!(data_url.starts_with("data:image/png;base64,"));
        assert!(data_url.ends_with(&BASE64.encode(bytes)));
        assert!(release_note_image(&runtime, "assets/changelogs/../setup.png").is_err());
        assert!(release_note_image(&runtime, "assets/changelogs/setup.svg").is_err());

        fs::write(runtime.join("assets/changelogs/fake.png"), b"not a png").unwrap();
        assert!(release_note_image(&runtime, "assets/changelogs/fake.png").unwrap_err().contains("bytes do not match"));
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

}

