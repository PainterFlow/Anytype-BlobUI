use crate::transaction::{commit_update, FileChange};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct PreviousManifest {
    release_version: Option<String>,
    managed_files: Vec<String>,
}

/// Installs an already extracted BlobUI package into a fresh app directory and
/// the selected Anytype data directory. Anytype settings are read verbatim and
/// are never rewritten when they already exist.
pub fn install(
    package: &Path,
    anytype: &Path,
    app: &Path,
    version: &str,
    shortcut: bool,
) -> Result<(), String> {
    let package = normalized(package)?;
    let anytype = normalized(anytype)?;
    let app = normalized(app)?;
    validate_target(&anytype, "Anytype data")?;
    validate_target(&app, "BlobUI app")?;
    if contains_releases(&anytype) || contains_releases(&app) {
        return Err("Installation inside a Releases folder is not supported.".into());
    }
    if overlaps(&app, &anytype) || overlaps(&app, &package) || overlaps(&anytype, &package) {
        return Err("Choose an app folder separate from Anytype and the extracted package.".into());
    }
    if !anytype.is_dir() {
        return Err(format!(
            "Anytype data folder does not exist: {}",
            anytype.display()
        ));
    }
    if app.exists() {
        if !app.is_dir()
            || fs::read_dir(&app)
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
        {
            return Err("The BlobUI app folder must be empty. Choose a new folder.".into());
        }
    }
    reject_links(&package)?;
    validate_package(&package, version)?;

    let data_path = anytype.join("blob/data.json");
    reject_links(&data_path)?;
    let data_exists = data_path.exists();
    let data_bytes = if data_exists {
        fs::read(&data_path).map_err(|e| format!("Could not read existing BlobUI settings: {e}"))?
    } else {
        serde_json::to_vec_pretty(&default_data(&anytype)).map_err(|e| e.to_string())?
    };
    let json_bytes = data_bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(&data_bytes);
    let mut data: Value = serde_json::from_slice(json_bytes)
        .map_err(|e| format!("Existing blob/data.json is not valid JSON: {e}"))?;
    if data_exists {
        let prior = data
            .get("installDirectory")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "Existing BlobUI settings do not identify their Anytype folder.".to_string()
            })?;
        if !same_path(Path::new(prior), &anytype) {
            return Err("Existing BlobUI settings belong to another Anytype folder.".into());
        }
    }
    if let Some(object) = data.as_object_mut() {
        object.insert("installDirectory".into(), json!(anytype));
    }
    // Compilation may need current defaults, but do not serialize this value
    // back to disk: this preserves unknown and newer settings byte-for-byte.
    fill_compile_defaults(&mut data, &anytype);
    let css = compile_package_css(&package, &anytype, &data)?;

    let app_preexisted = app.exists();
    let mut app_created = false;
    let mut shortcut_created = false;
    let install_result = (|| {
        fs::create_dir_all(&app).map_err(|e| format!("Could not create app folder: {e}"))?;
        app_created = true;
        for name in ["Dev", "assets"] {
            let source = package.join(name);
            if source.exists() {
                copy_tree(&source, &app.join(name))?;
            }
        }
        copy_file(
            &package.join("blobui-release.json"),
            &app.join("blobui-release.json"),
        )?;
        let executable = package.join("settings-app/dist/blobui-settings.exe");
        let app_dist = app.join("settings-app/dist");
        fs::create_dir_all(&app_dist)
            .map_err(|e| format!("Could not prepare app executable folder: {e}"))?;
        copy_file(&executable, &app_dist.join("blobui-settings.exe"))?;
        copy_file(&executable, &app.join("blobui-settings.exe"))?;
        fs::write(
            app.join(".blobui-app-managed-files.json"),
            app_manifest_bytes(&package, version)?,
        )
        .map_err(|e| format!("Could not write app ownership manifest: {e}"))?;
        let pointer = serde_json::to_vec_pretty(&json!({"installDirectory": anytype}))
            .map_err(|e| e.to_string())?;
        fs::write(app.join(".blobui-install.json"), pointer)
            .map_err(|e| format!("Could not write app install pointer: {e}"))?;

        let changes =
            make_install_changes(&package, &anytype, version, &css, &data_bytes, !data_exists)?;
        if shortcut {
            create_shortcut(&app_dist.join("blobui-settings.exe"), &app)?;
            shortcut_created = true;
        }
        commit_update(changes)?;
        Ok(())
    })();
    if let Err(error) = install_result {
        if shortcut_created {
            let _ = fs::remove_file(start_menu_shortcut_path());
        }
        if app_created {
            if reject_links(&app).is_ok() {
                if app_preexisted {
                    clear_directory_contents(&app);
                } else {
                    let _ = fs::remove_dir_all(&app);
                }
            }
        }
        return Err(error);
    }
    Ok(())
}

/// Replaces the files owned by an existing BlobUI installation. All app files,
/// runtime files, the generated stylesheet, and the ownership manifest are
/// committed together so a failed replacement restores the prior installation.
pub fn install_update(
    package: &Path,
    anytype: &Path,
    app: &Path,
    version: &str,
) -> Result<(), String> {
    let package = normalized(package)?;
    let anytype = normalized(anytype)?;
    let app = normalized(app)?;
    validate_target(&anytype, "Anytype data")?;
    validate_target(&app, "BlobUI app")?;
    if contains_releases(&anytype) || contains_releases(&app) {
        return Err("Installation inside a Releases folder is not supported.".into());
    }
    if overlaps(&app, &anytype) || overlaps(&app, &package) || overlaps(&anytype, &package) {
        return Err("Choose an app folder separate from Anytype and the extracted package.".into());
    }
    validate_update_target(&app, &anytype)?;
    reject_links(&package)?;
    validate_package(&package, version)?;

    let data_path = anytype.join("blob/data.json");
    reject_links(&data_path)?;
    let data_bytes = fs::read(&data_path)
        .map_err(|e| format!("Could not read existing BlobUI settings: {e}"))?;
    let json_bytes = data_bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(&data_bytes);
    let mut data: Value = serde_json::from_slice(json_bytes)
        .map_err(|e| format!("Existing blob/data.json is not valid JSON: {e}"))?;
    let prior = data
        .get("installDirectory")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "Existing BlobUI settings do not identify their Anytype folder.".to_string()
        })?;
    if !same_path(Path::new(prior), &anytype) {
        return Err("Existing BlobUI settings belong to another Anytype folder.".into());
    }
    fill_compile_defaults(&mut data, &anytype);

    let css = compile_package_css(&package, &anytype, &data)?;
    let mut changes = make_update_app_changes(&package, &app, version)?;
    changes.extend(make_install_changes(
        &package,
        &anytype,
        version,
        &css,
        &data_bytes,
        false,
    )?);
    commit_update(changes)
}

pub fn validate_update_target(app: &Path, anytype: &Path) -> Result<(), String> {
    let app = normalized(app)?;
    let anytype = normalized(anytype)?;
    validate_target(&anytype, "Anytype data")?;
    validate_target(&app, "BlobUI app")?;
    if contains_releases(&anytype) || contains_releases(&app) {
        return Err("Installation inside a Releases folder is not supported.".into());
    }
    if overlaps(&app, &anytype) {
        return Err("The BlobUI app folder must be separate from Anytype data.".into());
    }
    if !anytype.is_dir() || !app.is_dir() {
        return Err(
            "The existing BlobUI and Anytype installation folders could not be found.".into(),
        );
    }
    validate_existing_app(&app, &anytype)?;
    let data_path = anytype.join("blob/data.json");
    reject_links(&data_path)?;
    let bytes = fs::read(&data_path).map_err(|_| {
        "BlobUI settings were not found in the selected Anytype folder.".to_string()
    })?;
    let data: Value = serde_json::from_slice(
        bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes),
    )
    .map_err(|_| "BlobUI data.json in the selected Anytype folder is invalid.".to_string())?;
    let stored = data
        .get("installDirectory")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "BlobUI settings do not identify the selected Anytype folder.".to_string()
        })?;
    if !same_path(Path::new(stored), &anytype) {
        return Err("BlobUI settings do not match the selected Anytype folder.".into());
    }
    Ok(())
}

fn validate_existing_app(app: &Path, anytype: &Path) -> Result<(), String> {
    let pointer_path = app.join(".blobui-install.json");
    reject_links(&pointer_path)?;
    let pointer: Value = serde_json::from_slice(
        &fs::read(&pointer_path)
            .map_err(|_| "The selected app folder is not a BlobUI installation.".to_string())?,
    )
    .map_err(|_| "The selected app folder has an invalid install pointer.".to_string())?;
    let stored = pointer
        .get("installDirectory")
        .and_then(Value::as_str)
        .ok_or_else(|| "The selected app folder has an invalid install pointer.".to_string())?;
    if !same_path(Path::new(stored), anytype) {
        return Err("The selected app folder belongs to a different Anytype installation.".into());
    }
    for relative in [
        "blobui-settings.exe",
        "settings-app/dist/blobui-settings.exe",
    ] {
        let path = app.join(relative);
        reject_links(&path)?;
        if !path.is_file() {
            return Err(format!("The existing BlobUI app is missing {relative}."));
        }
    }
    Ok(())
}

fn compile_package_css(package: &Path, anytype: &Path, data: &Value) -> Result<String, String> {
    let manifest = read_previous_manifest(&anytype.join("blob/.blobui-managed-files.json"))?;
    let stage_parent = anytype.join("blob/staging");
    reject_links(&stage_parent)?;
    fs::create_dir_all(&stage_parent)
        .map_err(|e| format!("Could not prepare BlobUI update staging: {e}"))?;
    let stage = stage_parent.join(format!(".bi-compile-{}", unique_suffix()));
    fs::create_dir(&stage).map_err(|e| format!("Could not create BlobUI compiler staging: {e}"))?;
    let guard = RemoveTree(stage.clone());
    let compile_root = stage.join("source");
    fs::create_dir(&compile_root)
        .map_err(|e| format!("Could not prepare BlobUI compiler input: {e}"))?;
    copy_tree(&package.join("Dev"), &compile_root.join("Dev"))?;
    copy_tree(&package.join("assets"), &compile_root.join("assets"))?;
    preserve_unowned_runtime_css(
        &package.join("Dev"),
        &anytype.join("blob/runtime/Dev"),
        &compile_root.join("Dev"),
        &manifest.managed_files,
    )?;
    let css = crate::css::compile(&compile_root, anytype, data)?;
    drop(guard);
    Ok(css)
}

fn preserve_unowned_runtime_css(
    package_dev: &Path,
    installed_dev: &Path,
    compile_dev: &Path,
    previously_managed: &[String],
) -> Result<(), String> {
    let managed = previously_managed
        .iter()
        .map(|path| path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut release_files = BTreeMap::new();
    collect_managed_tree(package_dev, "", &mut release_files)?;
    let collisions = release_files
        .keys()
        .filter(|path| path.ends_with(".css"))
        .map(|path| path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    if !installed_dev.exists() {
        return Ok(());
    }
    reject_links(installed_dev)?;
    let mut local_files = BTreeMap::new();
    collect_managed_tree(installed_dev, "", &mut local_files)?;
    for (relative, source) in local_files {
        if !relative.to_ascii_lowercase().ends_with(".css") {
            continue;
        }
        let managed_path = format!("runtime/Dev/{relative}").to_ascii_lowercase();
        let key = relative.to_ascii_lowercase();
        if managed.contains(&managed_path) || collisions.contains(&key) {
            continue;
        }
        let destination = compile_dev.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not preserve a local CSS folder: {e}"))?;
        }
        copy_file(&source, &destination)?;
    }
    Ok(())
}

#[derive(Default)]
struct AppManifest {
    managed_files: Vec<String>,
}

fn make_update_app_changes(
    package: &Path,
    app: &Path,
    version: &str,
) -> Result<Vec<FileChange>, String> {
    let manifest_path = app.join(".blobui-app-managed-files.json");
    let previous = read_app_manifest(&manifest_path)?;
    let files = packaged_app_files(package)?;
    for key in files.keys() {
        validate_app_key(key)?;
    }

    let desired = files.keys().cloned().collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for key in &previous.managed_files {
        if !desired.contains(key) {
            changes.push(FileChange {
                path: app.join(key),
                contents: None,
            });
        }
    }
    for (key, source) in &files {
        changes.push(FileChange {
            path: app.join(key),
            contents: Some(
                fs::read(source).map_err(|e| format!("Could not read packaged app file: {e}"))?,
            ),
        });
    }
    changes.push(FileChange {
        path: manifest_path,
        contents: Some(app_manifest_bytes(package, version)?),
    });
    Ok(changes)
}

fn packaged_app_files(package: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut files = BTreeMap::<String, PathBuf>::new();
    collect_managed_tree(&package.join("Dev"), "Dev", &mut files)?;
    collect_managed_tree(&package.join("assets"), "assets", &mut files)?;
    files.insert(
        "settings-app/dist/blobui-settings.exe".into(),
        package.join("settings-app/dist/blobui-settings.exe"),
    );
    files.insert(
        "blobui-settings.exe".into(),
        package.join("settings-app/dist/blobui-settings.exe"),
    );
    files.insert(
        "blobui-release.json".into(),
        package.join("blobui-release.json"),
    );
    for key in files.keys() {
        validate_app_key(key)?;
    }
    Ok(files)
}

fn app_manifest_bytes(package: &Path, version: &str) -> Result<Vec<u8>, String> {
    let files = packaged_app_files(package)?;
    let manifest = json!({
        "manifestVersion": 1,
        "releaseVersion": version,
        "managedFiles": files.keys().cloned().collect::<BTreeSet<_>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| "Could not prepare the app ownership manifest.".to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn read_app_manifest(path: &Path) -> Result<AppManifest, String> {
    reject_links(path)?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppManifest::default())
        }
        Err(error) => {
            return Err(format!(
                "Could not read the app ownership manifest: {error}"
            ))
        }
    };
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "The app ownership manifest is invalid.".to_string())?;
    if value.get("manifestVersion").and_then(Value::as_u64) != Some(1) {
        return Err("The app ownership manifest has an unsupported format.".into());
    }
    let list = value
        .get("managedFiles")
        .and_then(Value::as_array)
        .ok_or_else(|| "The app ownership manifest has no managed file list.".to_string())?;
    let mut managed_files = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for item in list {
        let raw = item
            .as_str()
            .ok_or_else(|| "The app ownership manifest contains an invalid path.".to_string())?;
        validate_app_key(raw)?;
        if !seen.insert(raw.to_ascii_lowercase()) {
            return Err("The app ownership manifest contains duplicate paths.".into());
        }
        managed_files.insert(raw.to_string());
    }
    Ok(AppManifest {
        managed_files: managed_files.into_iter().collect(),
    })
}

fn validate_app_key(key: &str) -> Result<(), String> {
    if key.contains('\\')
        || key.starts_with('/')
        || key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !(key == "blobui-settings.exe"
            || key == "blobui-release.json"
            || key.starts_with("Dev/")
            || key.starts_with("assets/")
            || key.starts_with("settings-app/dist/"))
    {
        return Err(format!(
            "The app ownership manifest contains an unsafe path: {key}"
        ));
    }
    for segment in key.split('/') {
        if segment.ends_with('.')
            || segment.ends_with(' ')
            || segment
                .bytes()
                .any(|byte| byte < 0x20 || b":*?\"<>|".contains(&byte))
        {
            return Err(format!(
                "The app ownership manifest contains an unsafe path: {key}"
            ));
        }
        let stem = segment
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) {
            return Err(format!(
                "The app ownership manifest contains an unsafe path: {key}"
            ));
        }
    }
    Ok(())
}

fn read_previous_manifest(path: &Path) -> Result<PreviousManifest, String> {
    reject_links(path)?;
    let value: Value = match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
                .map_err(|e| format!("Existing managed-files manifest is invalid: {e}"))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PreviousManifest::default())
        }
        Err(error) => return Err(format!("Could not read managed-files manifest: {error}")),
    };
    let mut managed_files = BTreeSet::new();
    if let Some(items) = value.get("managedFiles").and_then(Value::as_array) {
        for item in items {
            let key = item.as_str().ok_or_else(|| {
                "The managed-files manifest contains an invalid path.".to_string()
            })?;
            validate_managed_key(key)?;
            managed_files.insert(key.to_string());
        }
    }
    Ok(PreviousManifest {
        release_version: value
            .get("releaseVersion")
            .and_then(Value::as_str)
            .map(str::to_owned),
        managed_files: managed_files.into_iter().collect(),
    })
}

struct RemoveTree(PathBuf);
impl Drop for RemoveTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn validate_package(package: &Path, version: &str) -> Result<(), String> {
    for path in [
        "Dev",
        "assets",
        "settings-app/dist/blobui-settings.exe",
        "blobui-release.json",
    ] {
        if !package.join(path).exists() {
            return Err(format!("The package is missing required payload: {path}"));
        }
    }
    let meta: Value = serde_json::from_slice(
        &fs::read(package.join("blobui-release.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("The package release metadata is invalid: {e}"))?;
    if meta.get("formatVersion").and_then(Value::as_u64) != Some(1)
        || meta.get("version").and_then(Value::as_str) != Some(version)
    {
        return Err("The package release metadata does not match the selected version.".into());
    }
    Ok(())
}

fn make_install_changes(
    package: &Path,
    anytype: &Path,
    version: &str,
    css: &str,
    data: &[u8],
    write_data: bool,
) -> Result<Vec<FileChange>, String> {
    let blob = anytype.join("blob");
    let manifest_path = blob.join(".blobui-managed-files.json");
    reject_links(&manifest_path)?;
    let old: Value = match fs::read(&manifest_path) {
        Ok(bytes) => {
            serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
                .map_err(|e| format!("Existing managed-files manifest is invalid: {e}"))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({"managedFiles": []}),
        Err(e) => return Err(format!("Could not read managed-files manifest: {e}")),
    };
    let mut previous = BTreeSet::new();
    if let Some(items) = old.get("managedFiles").and_then(Value::as_array) {
        for item in items {
            let key = item.as_str().ok_or_else(|| {
                "The managed-files manifest contains an invalid path.".to_string()
            })?;
            validate_managed_key(key)?;
            previous.insert(key.to_string());
        }
    }

    let mut files = BTreeMap::<String, PathBuf>::new();
    collect_managed_tree(&package.join("Dev"), "runtime/Dev", &mut files)?;
    collect_managed_tree(&package.join("assets"), "runtime/assets", &mut files)?;
    files.insert(
        "runtime/settings-app.exe".into(),
        package.join("settings-app/dist/blobui-settings.exe"),
    );
    files.insert(
        "runtime/blobui-release.json".into(),
        package.join("blobui-release.json"),
    );
    let managed = files.keys().cloned().collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for (key, source) in &files {
        if key == "runtime/assets/spacebanner/banner.png" {
            let destination = blob.join("assets/spacebanner/banner.png");
            reject_links(&destination)?;
            if !destination.exists() {
                changes.push(FileChange {
                    path: destination,
                    contents: Some(fs::read(source).map_err(|e| e.to_string())?),
                });
            }
        }
        let relative = key
            .strip_prefix("runtime/")
            .ok_or_else(|| "Invalid runtime asset key.".to_string())?;
        changes.push(FileChange {
            path: blob.join("runtime").join(relative),
            contents: Some(
                fs::read(source).map_err(|e| {
                    format!("Could not read packaged file {}: {e}", source.display())
                })?,
            ),
        });
    }
    for stale in previous.difference(&managed) {
        if stale == "assets/spacebanner/banner.png" {
            continue;
        }
        let relative = if let Some(p) = stale.strip_prefix("runtime/") {
            PathBuf::from("runtime").join(p)
        } else if let Some(p) = stale.strip_prefix("assets/") {
            PathBuf::from("assets").join(p)
        } else {
            return Err(format!("Unsupported managed file path: {stale}"));
        };
        changes.push(FileChange {
            path: blob.join(relative),
            contents: None,
        });
    }
    // If an earlier manifest managed the old root-level asset convention, keep
    // its legacy ownership marker; new files use runtime/ and assets/ paths.
    let manifest =
        json!({"manifestVersion": 1, "releaseVersion": version, "managedFiles": managed});
    changes.push(FileChange {
        path: manifest_path,
        contents: Some(serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?),
    });
    changes.push(FileChange {
        path: anytype.join("custom.css"),
        contents: Some(css.as_bytes().to_vec()),
    });
    if write_data {
        changes.push(FileChange {
            path: blob.join("data.json"),
            contents: Some(data.to_vec()),
        });
    }
    let existing_css_path = anytype.join("custom.css");
    reject_links(&existing_css_path)?;
    if let Ok(existing_css) = fs::read(&existing_css_path) {
        let backup = blob
            .join("backups")
            .join(format!("custom-{}.css.bak", unique_suffix()));
        changes.push(FileChange {
            path: backup,
            contents: Some(existing_css),
        });
    }
    Ok(changes)
}

fn collect_managed_tree(
    root: &Path,
    prefix: &str,
    out: &mut BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(root)
        .map_err(|e| format!("Could not read {}: {e}", root.display()))?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if is_reparse(&metadata) {
            return Err(format!(
                "Package content contains a link: {}",
                path.display()
            ));
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let relative = relative.to_string_lossy().replace('\\', "/");
        let key = if prefix.is_empty() {
            relative
        } else {
            format!("{prefix}/{relative}")
        };
        if metadata.is_dir() {
            collect_managed_tree(&path, &key, out)?;
        } else if metadata.is_file() {
            out.insert(key, path);
        } else {
            return Err(format!("Unsupported package file type: {}", path.display()));
        }
    }
    Ok(())
}

fn validate_managed_key(key: &str) -> Result<(), String> {
    if key.contains('\\')
        || key.starts_with('/')
        || key
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || !(key.starts_with("runtime/") || key.starts_with("assets/"))
    {
        return Err(format!(
            "The managed-files manifest contains an unsafe path: {key}"
        ));
    }
    for segment in key.split('/') {
        if segment.ends_with('.')
            || segment.ends_with(' ')
            || segment
                .bytes()
                .any(|byte| byte < 0x20 || b":*?\"<>|".contains(&byte))
        {
            return Err(format!(
                "The managed-files manifest contains an unsafe path: {key}"
            ));
        }
        let stem = segment
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) {
            return Err(format!(
                "The managed-files manifest contains an unsafe path: {key}"
            ));
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if is_reparse(&metadata) {
        return Err(format!(
            "Package content contains a link: {}",
            source.display()
        ));
    }
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            copy_tree(&path, &destination.join(path.file_name().unwrap()))?;
        }
        Ok(())
    } else if metadata.is_file() {
        copy_file(source, destination)
    } else {
        Err(format!(
            "Unsupported package file type: {}",
            source.display()
        ))
    }
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(format!("Package file is unsafe: {}", source.display()));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(source, destination)
        .map_err(|e| format!("Could not copy {}: {e}", source.display()))?;
    Ok(())
}

fn clear_directory_contents(path: &Path) {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let child = entry.path();
            if let Ok(metadata) = fs::symlink_metadata(&child) {
                if metadata.is_dir() && !is_reparse(&metadata) {
                    let _ = fs::remove_dir_all(child);
                } else {
                    let _ = fs::remove_file(child);
                }
            }
        }
    }
}

fn reject_links(path: &Path) -> Result<(), String> {
    let mut cursor = Some(path);
    while let Some(current) = cursor {
        if let Ok(meta) = fs::symlink_metadata(current) {
            if is_reparse(&meta) {
                return Err(format!(
                    "Installer paths cannot pass through a link or junction: {}",
                    current.display()
                ));
            }
        }
        cursor = current.parent();
    }
    Ok(())
}

fn validate_target(path: &Path, label: &str) -> Result<(), String> {
    if path.file_name().is_none() {
        return Err(format!("Choose a {label} folder rather than a drive root."));
    }
    reject_links(path)
}

fn normalized(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                result.push(component.as_os_str())
            }
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
        }
    }
    Ok(result)
}

fn contains_releases(path: &Path) -> bool {
    path.components().any(|part| {
        part.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("Releases")
    })
}
fn overlaps(a: &Path, b: &Path) -> bool {
    path_within_ci(a, b) || path_within_ci(b, a)
}
fn path_within_ci(path: &Path, root: &Path) -> bool {
    let path = path.to_string_lossy();
    let root = root
        .to_string_lossy()
        .trim_end_matches(|c| c == '\\' || c == '/')
        .to_string();
    path.eq_ignore_ascii_case(&root)
        || path.get(..root.len()).is_some_and(|head| {
            head.eq_ignore_ascii_case(&root)
                && path
                    .as_bytes()
                    .get(root.len())
                    .is_some_and(|byte| *byte == b'\\' || *byte == b'/')
        })
}
fn same_path(a: &Path, b: &Path) -> bool {
    normalized(a)
        .ok()
        .zip(normalized(b).ok())
        .is_some_and(|(a, b)| {
            a.to_string_lossy()
                .eq_ignore_ascii_case(&b.to_string_lossy())
        })
}
fn is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos:x}", std::process::id())
}

fn default_data(anytype: &Path) -> Value {
    json!({ "schemaVersion": 1, "apiVersion": "v2", "installDirectory": anytype,
        "setupMode": "defaults", "selectedSpace": {"id":"", "name":""}, "settingsObjectIds": [],
        "settings": {"fontFamily":"Libron", "codeFontFamily":"Consolas", "accentColor":"#DE50DE"}, "calendarObjectColors": {} })
}

fn fill_compile_defaults(data: &mut Value, anytype: &Path) {
    if !data.is_object() {
        *data = default_data(anytype);
    }
    let defaults = default_data(anytype);
    if let (Some(target), Some(source)) = (data.as_object_mut(), defaults.as_object()) {
        for (key, value) in source {
            target.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    if let Some(settings) = data.get_mut("settings") {
        if !settings.is_object() {
            *settings = json!({});
        }
        if let (Some(target), Some(source)) =
            (settings.as_object_mut(), defaults["settings"].as_object())
        {
            for (key, value) in source {
                target.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
    }
}

#[cfg(windows)]
fn create_shortcut(target: &Path, working_directory: &Path) -> Result<(), String> {
    create_shortcut_at(target, working_directory, &start_menu_shortcut_path())
}

#[cfg(windows)]
fn create_shortcut_at(
    target: &Path,
    working_directory: &Path,
    link_path: &Path,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    let programs = link_path
        .parent()
        .ok_or_else(|| "The shortcut path has no parent folder.".to_string())?;
    reject_links(&programs)?;
    fs::create_dir_all(&programs)
        .map_err(|e| format!("Could not prepare Start menu folder: {e}"))?;
    if link_path.exists() {
        return Err(
            "A BlobUI Start menu shortcut already exists. Remove it or disable shortcut creation."
                .into(),
        );
    }
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if let Err(e) = init.ok() {
        return Err(format!(
            "Could not initialize Windows shortcut support: {e}"
        ));
    }
    let _apartment = ComApartment;
    (|| unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("Could not create Start menu shortcut: {e}"))?;
        let target_w = target
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let work_w = working_directory
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        link.SetPath(PCWSTR(target_w.as_ptr()))
            .map_err(|e| e.to_string())?;
        link.SetWorkingDirectory(PCWSTR(work_w.as_ptr()))
            .map_err(|e| e.to_string())?;
        let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        let path_w = link_path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        persist
            .Save(PCWSTR(path_w.as_ptr()), true)
            .map_err(|e| format!("Could not save Start menu shortcut: {e}"))
    })()
}

#[cfg(windows)]
struct ComApartment;

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::Com::CoUninitialize() }
    }
}

#[cfg(windows)]
fn start_menu_shortcut_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Microsoft/Windows/Start Menu/Programs/BlobUI.lnk")
}

#[cfg(not(windows))]
fn create_shortcut(_: &Path, _: &Path) -> Result<(), String> {
    Err("Start menu shortcuts require Windows.".into())
}
#[cfg(not(windows))]
fn start_menu_shortcut_path() -> PathBuf {
    PathBuf::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(label: &str) -> PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run")
            .join(format!("installer-update-{label}-{}", unique_suffix()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_package(root: &Path, version: &str, executable: &[u8]) -> PathBuf {
        let package = root.join("package");
        fs::create_dir_all(package.join("Dev")).unwrap();
        fs::create_dir_all(package.join("assets/spacebanner")).unwrap();
        fs::create_dir_all(package.join("settings-app/dist")).unwrap();
        fs::write(
            package.join("Dev/theme.css"),
            b".release-theme { color: purple; }",
        )
        .unwrap();
        fs::write(
            package.join("assets/spacebanner/banner.png"),
            b"release-banner",
        )
        .unwrap();
        fs::write(
            package.join("settings-app/dist/blobui-settings.exe"),
            executable,
        )
        .unwrap();
        fs::write(
            package.join("blobui-release.json"),
            serde_json::to_vec(&json!({"formatVersion": 1, "version": version})).unwrap(),
        )
        .unwrap();
        package
    }

    #[test]
    fn update_replaces_app_and_runtime_while_preserving_user_state_and_local_css() {
        let root = fixture_root("preserve");
        let anytype = root.join("Anytype data");
        let blob = anytype.join("blob");
        let app = root.join("BlobUI app");
        let package = write_package(&root, "1.2.0", b"new-settings-executable");
        let data = json!({
            "installDirectory": anytype,
            "settings": {
                "fontFamily": "Inter", "codeFontFamily": "Consolas", "accentColor": "#2678B2",
                "bannerImage": "", "reducedMotion": false, "tabHoverEffect": false,
                "linkHoverEffect": false, "objectMentionHoverEffect": false
            },
            "apiConnection": {"enabled": false},
            "calendarObjectColors": {},
            "futureField": {"keep": true}
        });
        let data_bytes = serde_json::to_vec_pretty(&data).unwrap();
        let credential = b"encrypted-credential";
        let custom_banner = b"custom-banner";
        fs::create_dir_all(blob.join("runtime/Dev")).unwrap();
        fs::create_dir_all(blob.join("runtime/assets/spacebanner")).unwrap();
        fs::create_dir_all(blob.join("assets/spacebanner")).unwrap();
        fs::create_dir_all(blob.join("credentials")).unwrap();
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(app.join("settings-app/dist")).unwrap();
        fs::create_dir_all(app.join("Dev")).unwrap();
        fs::write(blob.join("data.json"), &data_bytes).unwrap();
        fs::write(blob.join("credentials/anytype-api-key.dpapi"), credential).unwrap();
        fs::write(anytype.join("custom.css"), b"old css").unwrap();
        fs::write(blob.join("assets/spacebanner/banner.png"), custom_banner).unwrap();
        fs::write(
            blob.join("runtime/Dev/local.css"),
            b".local-theme { color: teal; }",
        )
        .unwrap();
        fs::write(blob.join("runtime/Dev/obsolete.css"), b"old managed css").unwrap();
        fs::write(blob.join("runtime/settings-app.exe"), b"old runtime app").unwrap();
        fs::write(app.join("blobui-settings.exe"), b"old root app").unwrap();
        fs::write(
            app.join("settings-app/dist/blobui-settings.exe"),
            b"old nested app",
        )
        .unwrap();
        fs::write(app.join("Dev/theme.css"), b"old theme").unwrap();
        fs::write(app.join("notes.txt"), b"user app file").unwrap();
        fs::write(
            app.join(".blobui-install.json"),
            serde_json::to_vec(&json!({"installDirectory": anytype})).unwrap(),
        )
        .unwrap();
        fs::write(
            blob.join(".blobui-managed-files.json"),
            serde_json::to_vec(&json!({
                "manifestVersion": 1,
                "releaseVersion": "1.1.0",
                "managedFiles": ["runtime/Dev/obsolete.css", "runtime/settings-app.exe"]
            }))
            .unwrap(),
        )
        .unwrap();
        let preferences = root.join("updater.json");
        fs::write(&preferences, b"{\"automaticUpdatesEnabled\":true}").unwrap();
        let original_data = fs::read(blob.join("data.json")).unwrap();

        install_update(&package, &anytype, &app, "1.2.0").unwrap();

        assert_eq!(
            fs::read(app.join("blobui-settings.exe")).unwrap(),
            b"new-settings-executable"
        );
        assert_eq!(
            fs::read(app.join("settings-app/dist/blobui-settings.exe")).unwrap(),
            b"new-settings-executable"
        );
        assert_eq!(fs::read(app.join("notes.txt")).unwrap(), b"user app file");
        assert_eq!(
            fs::read(blob.join("runtime/settings-app.exe")).unwrap(),
            b"new-settings-executable"
        );
        assert_eq!(fs::read(blob.join("data.json")).unwrap(), original_data);
        assert_eq!(
            fs::read(blob.join("credentials/anytype-api-key.dpapi")).unwrap(),
            credential
        );
        assert_eq!(
            fs::read(blob.join("assets/spacebanner/banner.png")).unwrap(),
            custom_banner
        );
        assert_eq!(
            fs::read(blob.join("runtime/Dev/local.css")).unwrap(),
            b".local-theme { color: teal; }"
        );
        assert!(!blob.join("runtime/Dev/obsolete.css").exists());
        let css = fs::read_to_string(anytype.join("custom.css")).unwrap();
        assert!(css.contains(".release-theme"));
        assert!(css.contains(".local-theme"));
        assert_eq!(
            fs::read(&preferences).unwrap(),
            b"{\"automaticUpdatesEnabled\":true}"
        );
        let app_manifest: Value =
            serde_json::from_slice(&fs::read(app.join(".blobui-app-managed-files.json")).unwrap())
                .unwrap();
        assert_eq!(app_manifest["releaseVersion"], "1.2.0");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_rejects_app_pointer_for_another_anytype_install() {
        let root = fixture_root("wrong-pointer");
        let anytype = root.join("Anytype data");
        let app = root.join("BlobUI app");
        let package = write_package(&root, "1.2.0", b"new-settings-executable");
        fs::create_dir_all(anytype.join("blob")).unwrap();
        fs::create_dir_all(app.join("settings-app/dist")).unwrap();
        fs::write(app.join("blobui-settings.exe"), b"old").unwrap();
        fs::write(app.join("settings-app/dist/blobui-settings.exe"), b"old").unwrap();
        fs::write(
            app.join(".blobui-install.json"),
            b"{\"installDirectory\":\"C:/Elsewhere\"}",
        )
        .unwrap();
        assert!(install_update(&package, &anytype, &app, "1.2.0").is_err());
        assert_eq!(fs::read(app.join("blobui-settings.exe")).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compile_defaults_do_not_discard_newer_settings_or_unknown_fields() {
        let mut data = json!({
            "settingsV2": {"futureOption": {"value": 17}},
            "vendorField": ["keep", 3],
            "settings": {"accentColor": "#123456", "futureSetting": true}
        });
        let newer = data["settingsV2"].clone();
        let vendor = data["vendorField"].clone();
        fill_compile_defaults(&mut data, Path::new("C:/Anytype"));
        assert_eq!(data["settingsV2"], newer);
        assert_eq!(data["vendorField"], vendor);
        assert_eq!(data["settings"]["accentColor"], "#123456");
        assert_eq!(data["settings"]["futureSetting"], true);
        assert_eq!(data["settings"]["fontFamily"], "Libron");
    }

    #[test]
    fn managed_manifest_paths_cannot_escape_blob() {
        for unsafe_path in [
            "runtime/../data.json",
            "runtime\\settings-app.exe",
            "/outside",
            "custom.css",
        ] {
            assert!(
                validate_managed_key(unsafe_path).is_err(),
                "accepted {unsafe_path}"
            );
        }
        assert!(validate_managed_key("runtime/Dev/theme.css").is_ok());
        assert!(validate_managed_key("assets/spacebanner/banner.png").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn native_shortcut_is_created_at_requested_path_and_existing_link_is_preserved() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("test-shortcut-{}", unique_suffix()));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("blobui-settings.exe");
        let link = root.join("BlobUI.lnk");
        fs::write(&target, b"test executable placeholder").unwrap();
        create_shortcut_at(&target, &root, &link).unwrap();
        assert!(link.is_file());

        let original = fs::read(&link).unwrap();
        assert!(create_shortcut_at(&target, &root, &link).is_err());
        assert_eq!(fs::read(&link).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }
}
