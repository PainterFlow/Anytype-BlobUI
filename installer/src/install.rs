use crate::transaction::{commit_update, FileChange};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Remove only manifest-owned files. Settings, backups, and unrelated files stay.
pub fn uninstall(app: &Path, anytype: &Path) -> Result<(), String> {
    let app = normalized(app)?;
    let anytype = normalized(anytype)?;
    validate_update_target(&app, &anytype)?;
    let app_manifest = read_app_manifest(&app.join(".blobui-app-managed-files.json"))?;
    if app_manifest.managed_files.is_empty() {
        return Err("The current BlobUI ownership manifest was not found. Run the latest installer once before uninstalling. Nothing was removed.".into());
    }
    let runtime = read_uninstall_manifest(&anytype.join("blob/.blobui-managed-files.json"))?;
    let mut changes = Vec::new();
    for key in app_manifest.managed_files {
        if key == "blobui-installer.exe" {
            continue;
        }
        changes.push(FileChange {
            path: app.join(key),
            contents: None,
        });
    }
    for key in runtime.managed_files {
        // Earlier installers listed the shared default banner as managed even
        // though Settings treats it as a user-owned fallback after install.
        if key == "assets/spacebanner/banner.png" {
            continue;
        }
        changes.push(FileChange {
            path: anytype.join("blob").join(key),
            contents: None,
        });
    }
    let css_path = anytype.join("custom.css");
    reject_links(&css_path)?;
    if css_path.exists() {
        let current = fs::read(&css_path).map_err(|e| e.to_string())?;
        let data_bytes = fs::read(anytype.join("blob/data.json")).map_err(|e| e.to_string())?;
        let mut data: Value = serde_json::from_slice(
            data_bytes
                .strip_prefix(&[0xEF, 0xBB, 0xBF])
                .unwrap_or(&data_bytes),
        )
        .map_err(|e| e.to_string())?;
        fill_compile_defaults(&mut data, &anytype);
        let installed = compile_package_css(&app, &anytype, &data)?.into_bytes();
        if current != installed {
            return Err("custom.css has changed since installation. Nothing was removed. Save a copy and restore the installed stylesheet before uninstalling.".into());
        }
        changes.push(FileChange {
            path: anytype
                .join("blob/backups")
                .join(format!("uninstalled-{}.css.bak", unique_suffix())),
            contents: Some(current),
        });
        changes.push(FileChange {
            path: css_path,
            contents: None,
        });
    }
    if let Some(shortcut) = owned_start_menu_shortcut(&app)? {
        changes.push(FileChange {
            path: shortcut,
            contents: None,
        });
    }
    for path in [
        app.join(".blobui-install.json"),
        app.join(".blobui-app-managed-files.json"),
        anytype.join("blob/.blobui-managed-files.json"),
    ] {
        changes.push(FileChange {
            path,
            contents: None,
        });
    }
    // Preflight and rollback cover the whole removal. Never recursively delete a folder.
    commit_update(changes)
}

#[derive(Default)]
struct PreviousManifest {
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
    retained_installer: &Path,
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
    let mut previous_shortcut = None;
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
        copy_file(retained_installer, &app.join("blobui-installer.exe"))
            .map_err(|error| format!("Could not retain the installer: {error}"))?;
        fs::write(
            app.join(".blobui-app-managed-files.json"),
            app_manifest_bytes(&package, version, true)?,
        )
        .map_err(|e| format!("Could not write app ownership manifest: {e}"))?;
        let pointer = serde_json::to_vec_pretty(&json!({"installDirectory": anytype}))
            .map_err(|e| e.to_string())?;
        fs::write(app.join(".blobui-install.json"), pointer)
            .map_err(|e| format!("Could not write app install pointer: {e}"))?;

        let changes =
            make_install_changes(
                &package,
                &anytype,
                &app,
                version,
                &css,
                &data_bytes,
                !data_exists,
            )?;
        if shortcut {
            previous_shortcut =
                create_shortcut(&app_dist.join("blobui-settings.exe"), &app, &anytype)?;
            shortcut_created = true;
        }
        commit_update(changes)?;
        Ok(())
    })();
    if let Err(error) = install_result {
        if shortcut_created {
            let shortcut_path = start_menu_shortcut_path();
            if let Some(previous) = previous_shortcut {
                let _ = fs::write(shortcut_path, previous);
            } else {
                let _ = fs::remove_file(shortcut_path);
            }
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
    retained_installer: Option<&Path>,
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
    reject_links(&app.join("blobui-installer.exe"))?;
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
    let mut changes = make_update_app_changes(&package, &app, version, retained_installer)?;
    changes.extend(make_install_changes(
        &package,
        &anytype,
        &app,
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
    let remove_stage_parent = match fs::symlink_metadata(&stage_parent) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => {
            return Err(format!(
                "Could not inspect BlobUI compiler staging: {error}"
            ))
        }
    };
    fs::create_dir_all(&stage_parent)
        .map_err(|e| format!("Could not prepare BlobUI update staging: {e}"))?;
    let _stage_parent_guard =
        remove_stage_parent.then(|| RemoveEmptyDirectory(stage_parent.clone()));
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
    retained_installer: Option<&Path>,
) -> Result<Vec<FileChange>, String> {
    let manifest_path = app.join(".blobui-app-managed-files.json");
    let previous = read_app_manifest(&manifest_path)?;
    let files = packaged_app_files(package)?;
    for key in files.keys() {
        validate_app_key(key)?;
    }

    let mut desired = files.keys().cloned().collect::<BTreeSet<_>>();
    let manage_installer = retained_installer.is_some()
        || previous
            .managed_files
            .iter()
            .any(|key| key == "blobui-installer.exe");
    if manage_installer {
        desired.insert("blobui-installer.exe".into());
    }
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
    if let Some(source) = retained_installer {
        changes.push(FileChange {
            path: app.join("blobui-installer.exe"),
            contents: Some(
                fs::read(source)
                    .map_err(|e| format!("Could not read the installer for retention: {e}"))?,
            ),
        });
    }
    changes.push(FileChange {
        path: manifest_path,
        contents: Some(app_manifest_bytes(package, version, manage_installer)?),
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
    files.insert("Changelogs.md".into(), package.join("Changelogs.md"));
    files.insert("Changelogs.yaml".into(), package.join("Changelogs.yaml"));
    for key in files.keys() {
        validate_app_key(key)?;
    }
    Ok(files)
}

fn app_manifest_bytes(
    package: &Path,
    version: &str,
    manage_installer: bool,
) -> Result<Vec<u8>, String> {
    let files = packaged_app_files(package)?;
    let mut managed_files = files.keys().cloned().collect::<BTreeSet<_>>();
    if manage_installer {
        managed_files.insert("blobui-installer.exe".into());
    }
    let manifest = json!({
        "manifestVersion": 1,
        "releaseVersion": version,
        "managedFiles": managed_files,
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
            || key == "blobui-installer.exe"
            || key == "blobui-release.json"
            || key == "Changelogs.md"
            || key == "Changelogs.yaml"
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
        managed_files: managed_files.into_iter().collect(),
    })
}

fn read_uninstall_manifest(path: &Path) -> Result<PreviousManifest, String> {
    reject_links(path)?;
    let bytes = fs::read(path).map_err(|_| {
        "The BlobUI runtime ownership manifest could not be read. Nothing was removed.".to_string()
    })?;
    let value: Value = serde_json::from_slice(
        bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes),
    )
    .map_err(|_| {
        "The BlobUI runtime ownership manifest is invalid. Nothing was removed.".to_string()
    })?;
    if value.get("manifestVersion").and_then(Value::as_u64) != Some(1) {
        return Err(
            "The BlobUI runtime ownership manifest has an unsupported format. Nothing was removed."
                .into(),
        );
    }
    let items = value
        .get("managedFiles")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "The BlobUI runtime ownership manifest has no managed file list. Nothing was removed."
                .to_string()
        })?;
    let mut managed_files = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let key = item.as_str()
            .ok_or_else(|| "The BlobUI runtime ownership manifest contains an invalid path. Nothing was removed.".to_string())?;
        validate_managed_key(key)?;
        if !seen.insert(key.to_ascii_lowercase()) {
            return Err("The BlobUI runtime ownership manifest contains duplicate paths. Nothing was removed.".into());
        }
        managed_files.insert(key.to_string());
    }
    if managed_files.is_empty() {
        return Err("The BlobUI runtime ownership manifest is empty. Nothing was removed.".into());
    }
    Ok(PreviousManifest {
        managed_files: managed_files.into_iter().collect(),
    })
}

struct RemoveTree(PathBuf);
impl Drop for RemoveTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct RemoveEmptyDirectory(PathBuf);
impl Drop for RemoveEmptyDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

fn validate_package(package: &Path, version: &str) -> Result<(), String> {
    reject_links(package)?;
    for path in [
        "Dev",
        "assets",
        "settings-app/dist/blobui-settings.exe",
        "blobui-release.json",
        "Changelogs.md",
        "Changelogs.yaml",
    ] {
        let payload = package.join(path);
        reject_links(&payload)?;
        if !payload.exists() {
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
    validate_changelog_payload(package)?;
    Ok(())
}

pub fn validate_release_package(package: &Path) -> Result<(), String> {
    let path = package.join("blobui-release.json");
    reject_links(&path)?;
    let metadata: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|_| "The package release metadata is missing.".to_string())?,
    )
    .map_err(|_| "The package release metadata is invalid JSON.".to_string())?;
    if metadata.get("formatVersion").and_then(Value::as_u64) != Some(1) {
        return Err("The package release metadata has an unsupported format.".into());
    }
    let version = metadata.get("version").and_then(Value::as_str)
        .ok_or("The package release version is missing.")?;
    if version.len() > 64
        || !version.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
        || !version.as_bytes().first().is_some_and(u8::is_ascii_digit)
    {
        return Err("The package release version is invalid.".into());
    }
    validate_package(package, version)
}

const MAX_CHANGELOG_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CHANGELOG_TEXT_BYTES: u64 = 512 * 1024;

#[derive(Default)]
struct ChangelogImage {
    id: String,
    src: Option<String>,
    alt_seen: bool,
}

fn validate_changelog_payload(package: &Path) -> Result<(), String> {
    let markdown_path = package.join("Changelogs.md");
    reject_links(&markdown_path)?;
    let markdown_metadata = fs::metadata(&markdown_path)
        .map_err(|_| "The release notes Markdown is missing or unreadable.".to_string())?;
    if !markdown_metadata.is_file() || markdown_metadata.len() == 0 || markdown_metadata.len() > MAX_CHANGELOG_TEXT_BYTES {
        return Err("The release notes Markdown is empty or exceeds the supported size.".into());
    }
    let markdown = fs::read_to_string(&markdown_path)
        .map_err(|_| "The release notes Markdown is not valid UTF-8.".to_string())?;
    if markdown.trim().is_empty() {
        return Err("The release notes Markdown is empty.".into());
    }

    let yaml_path = package.join("Changelogs.yaml");
    reject_links(&yaml_path)?;
    let yaml_metadata = fs::metadata(&yaml_path)
        .map_err(|_| "The release notes image manifest is missing or unreadable.".to_string())?;
    if !yaml_metadata.is_file() || yaml_metadata.len() == 0 || yaml_metadata.len() > MAX_CHANGELOG_TEXT_BYTES {
        return Err("The release notes image manifest is empty or exceeds the supported size.".into());
    }
    let yaml = fs::read_to_string(&yaml_path)
        .map_err(|_| "The release notes image manifest is not valid UTF-8.".to_string())?;
    let entries = parse_changelog_image_manifest(&yaml)?;
    let ids = entries.iter().map(|entry| entry.id.as_str()).collect::<BTreeSet<_>>();
    validate_changelog_image_references(&markdown, &ids)?;
    for entry in entries {
        let source = entry.src.ok_or("A release note image is missing its src field.")?;
        validate_changelog_image_file(package, &source)?;
    }
    Ok(())
}

fn parse_changelog_image_manifest(yaml: &str) -> Result<Vec<ChangelogImage>, String> {
    let mut entries = Vec::new();
    let mut current = None;
    let mut section_seen = false;
    let mut empty_list = false;
    let mut ids = BTreeSet::new();

    fn save(
        current: &mut Option<ChangelogImage>,
        entries: &mut Vec<ChangelogImage>,
        ids: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        let Some(image) = current.take() else { return Ok(()); };
        let source = image.src.as_deref().ok_or("Each Changelogs.yaml image needs an id and src.")?;
        validate_changelog_source(source)?;
        if !ids.insert(image.id.clone()) {
            return Err("Changelogs.yaml contains a duplicate image id.".into());
        }
        entries.push(image);
        Ok(())
    }

    for raw_line in yaml.trim_start_matches('\u{feff}').lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') { continue; }
        if line.contains('\t') { return Err("Changelogs.yaml must use spaces for indentation.".into()); }
        let section = line.split_once(" #").map_or(line, |(value, _)| value).trim();
        if section == "images:" {
            if section_seen { return Err("Changelogs.yaml contains more than one images section.".into()); }
            section_seen = true;
            continue;
        }
        if Regex::new(r"^images:\s*\[\s*\]$").unwrap().is_match(section) {
            if section_seen { return Err("Changelogs.yaml contains more than one images section.".into()); }
            section_seen = true;
            empty_list = true;
            continue;
        }
        if !section_seen { return Err("Changelogs.yaml must begin with an images section.".into()); }
        if empty_list { return Err("Changelogs.yaml cannot add entries after images: [].".into()); }
        if let Some(item) = line.strip_prefix("  - ").and_then(|item| item.strip_prefix("id:")) {
            save(&mut current, &mut entries, &mut ids)?;
            let id = parse_changelog_scalar(item)?;
            if !valid_changelog_id(&id) {
                return Err("Changelogs.yaml image ids may use letters, numbers, hyphens, and underscores.".into());
            }
            current = Some(ChangelogImage { id, ..ChangelogImage::default() });
            continue;
        }
        if let Some(field) = line.strip_prefix("    ") {
            let (name, value) = field.split_once(':').ok_or("Changelogs.yaml supports id, src, and optional alt fields.")?;
            let image = current.as_mut().ok_or("Changelogs.yaml image fields must follow a list item.")?;
            match name {
                "src" => {
                    if image.src.is_some() { return Err("Changelogs.yaml repeats src for an image.".into()); }
                    image.src = Some(parse_changelog_scalar(value)?);
                }
                "alt" => {
                    if image.alt_seen { return Err("Changelogs.yaml repeats alt for an image.".into()); }
                    let _ = parse_changelog_scalar(value)?;
                    image.alt_seen = true;
                }
                _ => return Err("Changelogs.yaml supports id, src, and optional alt fields.".into()),
            }
            continue;
        }
        return Err("Changelogs.yaml supports an images list with id, src, and optional alt fields.".into());
    }
    if !section_seen { return Err("Changelogs.yaml must contain an images section.".into()); }
    save(&mut current, &mut entries, &mut ids)?;
    Ok(entries)
}

fn parse_changelog_scalar(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() { return Err("Changelogs.yaml contains an empty value.".into()); }
    if value.starts_with('"') {
        return serde_json::from_str(value).map_err(|_| "Changelogs.yaml contains an invalid quoted value.".into());
    }
    if value.starts_with('\'') {
        if !value.ends_with('\'') || value.len() < 2 {
            return Err("Changelogs.yaml contains an invalid quoted value.".into());
        }
        return Ok(value[1..value.len() - 1].replace("''", "'"));
    }
    Ok(value.split_once(" #").map_or(value, |(scalar, _)| scalar).trim().to_string())
}

fn valid_changelog_id(id: &str) -> bool {
    let mut chars = id.bytes();
    let Some(first) = chars.next() else { return false; };
    id.len() <= 64
        && first.is_ascii_alphanumeric()
        && chars.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn validate_changelog_source(source: &str) -> Result<(), String> {
    let relative = source.strip_prefix("assets/changelogs/")
        .ok_or("Changelogs.yaml image paths must stay inside assets/changelogs/.")?;
    let segments = relative.split('/').collect::<Vec<_>>();
    if segments.is_empty() || segments.iter().any(|segment| {
        segment.is_empty() || *segment == "." || *segment == ".."
            || !segment.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    }) {
        return Err("Changelogs.yaml contains an unsafe image path.".into());
    }
    if !matches!(Path::new(source).extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("png" | "jpg" | "jpeg" | "gif" | "webp")) {
        return Err("Changelogs.yaml supports PNG, JPEG, GIF, and WebP images.".into());
    }
    Ok(())
}

fn validate_changelog_image_file(package: &Path, source: &str) -> Result<(), String> {
    validate_changelog_source(source)?;
    let path = package.join(source);
    reject_links(&path)?;
    let metadata = fs::metadata(&path).map_err(|_| format!("The release note image is missing: {source}"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_CHANGELOG_IMAGE_BYTES {
        return Err(format!("The release note image has an unsupported size or type: {source}"));
    }
    let bytes = fs::read(&path).map_err(|_| format!("The release note image could not be read: {source}"))?;
    let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_ascii_lowercase();
    let valid = match extension.as_str() {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        _ => false,
    };
    if !valid { return Err(format!("The release note image bytes do not match its file type: {source}")); }
    Ok(())
}

fn validate_changelog_image_references(markdown: &str, aliases: &BTreeSet<&str>) -> Result<(), String> {
    let mut text = String::new();
    let mut fence: Option<(char, usize)> = None;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed.chars().take_while(|character| *character == '`' || *character == '~').collect::<String>();
        if marker.len() >= 3 {
            let character = marker.chars().next().unwrap();
            if let Some((active, length)) = fence {
                if active == character && marker.len() >= length { fence = None; }
            } else {
                fence = Some((character, marker.len()));
            }
            text.push('\n');
        } else if fence.is_none() {
            text.push_str(line);
            text.push('\n');
        }
    }
    let inline_code = Regex::new(r"`+[^`\n]*`+").unwrap();
    let text = inline_code.replace_all(&text, " ");
    let image_reference = Regex::new(r"!\[[^\]\n]*\]\(\s*@(?<id>[A-Za-z0-9][A-Za-z0-9_-]{0,63})(?:\s+[^)\n]*)?\s*\)").unwrap();
    let html_image_reference = Regex::new(r#"(?is)<img\b[^>]*\bsrc\s*=\s*['"]@(?<id>[A-Za-z0-9][A-Za-z0-9_-]{0,63})['"][^>]*>"#).unwrap();
    for captures in image_reference.captures_iter(&text).chain(html_image_reference.captures_iter(&text)) {
        let id = captures.name("id").unwrap().as_str();
        if !aliases.contains(id) {
            return Err(format!("Release notes reference image id '{id}' which is not declared in Changelogs.yaml."));
        }
    }
    Ok(())
}

fn make_install_changes(
    package: &Path,
    anytype: &Path,
    app: &Path,
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
    files.insert(
        "runtime/Changelogs.md".into(),
        package.join("Changelogs.md"),
    );
    files.insert(
        "runtime/Changelogs.yaml".into(),
        package.join("Changelogs.yaml"),
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
    let manifest = json!({
        "manifestVersion": 1,
        "releaseVersion": version,
        "appDirectory": app.to_string_lossy().into_owned(),
        "managedFiles": managed
    });
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
            #[cfg(windows)]
            {
                comparable_windows_path(&a).eq_ignore_ascii_case(&comparable_windows_path(&b))
            }
            #[cfg(not(windows))]
            {
                a.to_string_lossy()
                    .eq_ignore_ascii_case(&b.to_string_lossy())
            }
        })
}

#[cfg(windows)]
fn comparable_windows_path(path: &Path) -> String {
    // Windows canonicalize can add a verbatim prefix that saved install paths
    // do not have, even though both strings refer to the same directory.
    const VERBATIM_PREFIX: &str = "\\\\?\\";
    const VERBATIM_UNC_PREFIX: &str = "\\\\?\\UNC\\";

    let path = path.to_string_lossy().replace('/', "\\");
    if path
        .get(..VERBATIM_UNC_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(VERBATIM_UNC_PREFIX))
    {
        return format!(r"\\{}", &path[VERBATIM_UNC_PREFIX.len()..]);
    }
    if path
        .get(..VERBATIM_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(VERBATIM_PREFIX))
    {
        let without_prefix = &path[VERBATIM_PREFIX.len()..];
        if without_prefix
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && without_prefix.as_bytes().get(1) == Some(&b':')
        {
            return without_prefix.to_owned();
        }
    }
    path
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
fn create_shortcut(
    target: &Path,
    working_directory: &Path,
    install_directory: &Path,
) -> Result<Option<Vec<u8>>, String> {
    create_shortcut_at(
        target,
        working_directory,
        &start_menu_shortcut_path(),
        install_directory,
    )
}

#[cfg(windows)]
fn create_shortcut_at(
    target: &Path,
    working_directory: &Path,
    link_path: &Path,
    install_directory: &Path,
) -> Result<Option<Vec<u8>>, String> {
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
    let previous_shortcut = match fs::symlink_metadata(link_path) {
        Ok(metadata) => {
            reject_links(link_path)?;
            if !metadata.is_file() {
                return Err(
                    "A BlobUI Start menu shortcut path already exists and is not a file.".into(),
                );
            }
            let Some(existing_target) = read_start_menu_shortcut_target(link_path)? else {
                return Err(
                    "A BlobUI Start menu shortcut already exists and cannot be verified.".into(),
                );
            };
            if !shortcut_targets_install(&existing_target, install_directory) {
                return Err(
                    "A BlobUI Start menu shortcut already exists and is not owned by this installation."
                        .into(),
                );
            }
            Some(fs::read(link_path).map_err(|error| {
                format!("Could not preserve the existing BlobUI Start menu shortcut: {error}")
            })?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!("Could not inspect the BlobUI Start menu shortcut: {error}"))
        }
    };
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if let Err(e) = init.ok() {
        return Err(format!(
            "Could not initialize Windows shortcut support: {e}"
        ));
    }
    let _apartment = ComApartment;
    let save_result = (|| unsafe {
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
    })();
    if let Err(error) = save_result {
        if let Some(previous) = &previous_shortcut {
            let _ = fs::write(link_path, previous);
        } else {
            let _ = fs::remove_file(link_path);
        }
        return Err(error);
    }
    Ok(previous_shortcut)
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

#[cfg(windows)]
fn read_start_menu_shortcut_target(path: &Path) -> Result<Option<PathBuf>, String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Could not inspect the BlobUI Start menu shortcut: {error}"
            ))
        }
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    reject_links(path)?;
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    init.ok()
        .map_err(|error| format!("Could not inspect the BlobUI Start menu shortcut: {error}"))?;
    let _apartment = ComApartment;
    let target = (|| unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|error| {
                format!("Could not inspect the BlobUI Start menu shortcut: {error}")
            })?;
        let persist: IPersistFile = link.cast().map_err(|error| error.to_string())?;
        let path_wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        persist
            .Load(PCWSTR(path_wide.as_ptr()), STGM_READ)
            .map_err(|error| format!("Could not read the BlobUI Start menu shortcut: {error}"))?;
        let mut target = [0u16; 32768];
        let mut file_data = WIN32_FIND_DATAW::default();
        link.GetPath(&mut target, &mut file_data, SLGP_RAWPATH.0 as u32)
            .map_err(|error| {
                format!("Could not read the BlobUI Start menu shortcut target: {error}")
            })?;
        Ok::<_, String>(OsString::from_wide(
            &target[..target
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(target.len())],
        ))
    })()?;
    Ok(Some(PathBuf::from(target)))
}

#[cfg(windows)]
fn shortcut_targets_install(target: &Path, install_directory: &Path) -> bool {
    let Some(app) = target
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return false;
    };
    if !same_path(target, &app.join("settings-app/dist/blobui-settings.exe")) {
        return false;
    }
    validate_existing_app(app, install_directory).is_ok()
}

#[cfg(windows)]
fn owned_start_menu_shortcut(app: &Path) -> Result<Option<PathBuf>, String> {
    let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from) else {
        return Ok(None);
    };
    let path = app_data.join("Microsoft/Windows/Start Menu/Programs/BlobUI.lnk");
    let Some(target) = read_start_menu_shortcut_target(&path)? else {
        return Ok(None);
    };
    let expected = app.join("settings-app/dist/blobui-settings.exe");
    Ok(same_path(&target, &expected).then_some(path))
}

#[cfg(not(windows))]
fn owned_start_menu_shortcut(_: &Path) -> Result<Option<PathBuf>, String> {
    Ok(None)
}

#[cfg(not(windows))]
fn create_shortcut(_: &Path, _: &Path, _: &Path) -> Result<Option<Vec<u8>>, String> {
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
        fs::write(package.join("Changelogs.md"), "# Example release notes\n").unwrap();
        fs::write(package.join("Changelogs.yaml"), "images: []\n").unwrap();
        package
    }

    #[test]
    fn release_notes_manifest_requires_declared_local_images_and_matching_references() {
        let root = fixture_root("release-notes");
        let package = write_package(&root, "1.2.0", b"settings executable");
        validate_package(&package, "1.2.0").unwrap();

        let image = package.join("assets/changelogs/setup.png");
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        fs::write(&image, b"\x89PNG\r\n\x1a\nfixture").unwrap();
        fs::write(
            package.join("Changelogs.yaml"),
            "images:\n  - id: setup\n    src: assets/changelogs/setup.png\n    alt: Setup screen\n",
        )
        .unwrap();
        fs::write(
            package.join("Changelogs.md"),
            "![Setup](@setup)\n\n`![code](@not-an-image)`\n",
        )
        .unwrap();
        validate_package(&package, "1.2.0").unwrap();

        fs::write(package.join("Changelogs.md"), "![Missing](@missing)\n").unwrap();
        assert!(validate_package(&package, "1.2.0").unwrap_err().contains("not declared"));

        fs::write(package.join("Changelogs.md"), "![Setup](@setup)\n").unwrap();
        fs::write(
            package.join("Changelogs.yaml"),
            "images:\n  - id: setup\n    src: assets/changelogs/../setup.png\n",
        )
        .unwrap();
        assert!(validate_package(&package, "1.2.0").is_err());

        fs::write(
            package.join("Changelogs.yaml"),
            "images:\n  - id: setup\n    src: assets/changelogs/missing.png\n",
        )
        .unwrap();
        assert!(validate_package(&package, "1.2.0").unwrap_err().contains("missing"));

        fs::write(
            package.join("Changelogs.yaml"),
            "images:\n  - id: setup\n    src: assets/changelogs/setup.svg\n",
        )
        .unwrap();
        assert!(validate_package(&package, "1.2.0").unwrap_err().contains("PNG, JPEG, GIF, and WebP"));

        fs::remove_dir_all(root).unwrap();
    }

    fn installed_uninstall_fixture(label: &str) -> (PathBuf, PathBuf, PathBuf, Vec<u8>) {
        let root = fixture_root(label);
        let anytype = root.join("Anytype data");
        let app = root.join("BlobUI app");
        let blob = anytype.join("blob");
        fs::create_dir_all(&anytype).unwrap();
        fs::write(anytype.join("custom.css"), b"pre-existing user css").unwrap();
        let mut data = default_data(&anytype);
        data["settings"]["fontFamily"] = json!("Inter");
        data["settings"]["accentColor"] = json!("#123456");
        data["futureField"] = json!({"preserve": true});
        fs::create_dir_all(&blob).unwrap();
        let data_bytes = serde_json::to_vec_pretty(&data).unwrap();
        fs::write(blob.join("data.json"), &data_bytes).unwrap();
        fs::create_dir_all(blob.join("credentials")).unwrap();
        fs::write(
            blob.join("credentials/anytype-api-key.dpapi"),
            b"credential",
        )
        .unwrap();
        fs::create_dir_all(blob.join("assets/spacebanner")).unwrap();
        fs::write(blob.join("assets/spacebanner/banner.png"), b"custom banner").unwrap();
        fs::write(anytype.join("keep.txt"), b"unrelated Anytype data").unwrap();
        let package = write_package(&root, "1.2.0", b"settings executable");
        let installer = root.join("installer.exe");
        fs::write(&installer, b"retained installer").unwrap();
        install(&package, &anytype, &app, "1.2.0", false, &installer).unwrap();
        fs::write(app.join("notes.txt"), b"unrelated app file").unwrap();
        let installed_css = fs::read(anytype.join("custom.css")).unwrap();
        (root, anytype, app, installed_css)
    }

    #[test]
    fn failed_installer_retention_rolls_back_a_new_install() {
        let root = fixture_root("initial-installer-retention-failure");
        let anytype = root.join("Anytype data");
        let app = root.join("BlobUI app");
        let blob = anytype.join("blob");
        fs::create_dir_all(&blob).unwrap();
        let data = serde_json::to_vec(&default_data(&anytype)).unwrap();
        fs::write(blob.join("data.json"), &data).unwrap();
        fs::write(anytype.join("custom.css"), b"prior stylesheet").unwrap();
        let package = write_package(&root, "1.2.0", b"settings executable");
        let missing_installer = root.join("missing-installer.exe");

        let error = install(
            &package,
            &anytype,
            &app,
            "1.2.0",
            false,
            &missing_installer,
        )
        .unwrap_err();

        assert!(error.contains("Could not retain the installer"));
        assert!(!app.exists());
        assert_eq!(fs::read(blob.join("data.json")).unwrap(), data);
        assert_eq!(fs::read(anytype.join("custom.css")).unwrap(), b"prior stylesheet");
        assert!(!blob.join("runtime/settings-app.exe").exists());
        fs::remove_dir_all(root).unwrap();
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
        let retained_installer = root.join("new-installer.exe");
        fs::write(&retained_installer, b"updated retained installer").unwrap();

        install_update(
            &package,
            &anytype,
            &app,
            "1.2.0",
            Some(&retained_installer),
        )
        .unwrap();

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
            fs::read(app.join("blobui-installer.exe")).unwrap(),
            b"updated retained installer"
        );
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
        assert!(app_manifest["managedFiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "blobui-installer.exe"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_retained_installer_update_keeps_the_existing_install() {
        let (root, anytype, app, original_css) =
            installed_uninstall_fixture("retained-installer-failure");
        let package = write_package(&root, "1.3.0", b"new-settings-executable");
        let retained_installer = root.join("new-installer.exe");
        fs::write(&retained_installer, b"new installer").unwrap();
        let installer_target = app.join("blobui-installer.exe");
        fs::remove_file(&installer_target).unwrap();
        fs::create_dir(&installer_target).unwrap();
        let original_data = fs::read(anytype.join("blob/data.json")).unwrap();
        let original_executable = fs::read(app.join("blobui-settings.exe")).unwrap();

        let error = install_update(
            &package,
            &anytype,
            &app,
            "1.3.0",
            Some(&retained_installer),
        )
        .unwrap_err();

        assert!(error.contains("not a regular file"));
        assert_eq!(
            fs::read(app.join("blobui-settings.exe")).unwrap(),
            original_executable
        );
        assert_eq!(fs::read(anytype.join("blob/data.json")).unwrap(), original_data);
        assert_eq!(fs::read(anytype.join("custom.css")).unwrap(), original_css);
        assert!(installer_target.is_dir());
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
        assert!(install_update(&package, &anytype, &app, "1.2.0", None).is_err());
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

    #[test]
    fn uninstall_removes_owned_files_and_preserves_user_data_backups_and_installer() {
        let (root, anytype, app, installed_css) = installed_uninstall_fixture("uninstall-preserve");
        let blob = anytype.join("blob");
        let data_before = fs::read(blob.join("data.json")).unwrap();
        let backup_dir = blob.join("backups");
        let old_css_backup = fs::read_dir(&backup_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("custom-")
            })
            .expect("the original user CSS backup should exist");
        assert_eq!(fs::read(&old_css_backup).unwrap(), b"pre-existing user css");

        uninstall(&app, &anytype).unwrap();

        assert!(!anytype.join("custom.css").exists());
        assert!(!app.join(".blobui-install.json").exists());
        assert!(!app.join(".blobui-app-managed-files.json").exists());
        assert!(!app.join("blobui-settings.exe").exists());
        assert!(!app.join("settings-app/dist/blobui-settings.exe").exists());
        assert!(!app.join("Dev/theme.css").exists());
        assert!(!blob.join("runtime/settings-app.exe").exists());
        assert!(!blob.join("runtime/Dev/theme.css").exists());
        assert!(!blob.join(".blobui-managed-files.json").exists());
        assert_eq!(
            fs::read(app.join("blobui-installer.exe")).unwrap(),
            b"retained installer"
        );
        assert_eq!(
            fs::read(app.join("notes.txt")).unwrap(),
            b"unrelated app file"
        );
        assert_eq!(fs::read(blob.join("data.json")).unwrap(), data_before);
        assert_eq!(
            fs::read(blob.join("credentials/anytype-api-key.dpapi")).unwrap(),
            b"credential"
        );
        assert_eq!(
            fs::read(blob.join("assets/spacebanner/banner.png")).unwrap(),
            b"custom banner"
        );
        assert_eq!(
            fs::read(anytype.join("keep.txt")).unwrap(),
            b"unrelated Anytype data"
        );
        assert_eq!(fs::read(old_css_backup).unwrap(), b"pre-existing user css");
        let uninstall_backup = fs::read_dir(backup_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("uninstalled-")
            })
            .expect("the removed generated CSS should have a backup");
        assert_eq!(fs::read(uninstall_backup).unwrap(), installed_css);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uninstall_refuses_changed_css_without_removing_anything() {
        let (root, anytype, app, _) = installed_uninstall_fixture("uninstall-changed-css");
        let blob = anytype.join("blob");
        let changed_css = [
            fs::read(anytype.join("custom.css")).unwrap(),
            b"\n/* user edit */".to_vec(),
        ]
        .concat();
        fs::write(anytype.join("custom.css"), &changed_css).unwrap();
        let app_exe = fs::read(app.join("blobui-settings.exe")).unwrap();
        let data = fs::read(blob.join("data.json")).unwrap();

        let error = uninstall(&app, &anytype).unwrap_err();

        assert!(error.contains("custom.css has changed"));
        assert_eq!(fs::read(anytype.join("custom.css")).unwrap(), changed_css);
        assert_eq!(fs::read(app.join("blobui-settings.exe")).unwrap(), app_exe);
        assert_eq!(fs::read(blob.join("data.json")).unwrap(), data);
        assert!(app.join(".blobui-install.json").exists());
        assert!(blob.join(".blobui-managed-files.json").exists());
        assert!(
            !blob.join("staging").exists(),
            "CSS comparison must not leave compiler staging behind"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uninstall_rejects_traversal_in_either_manifest_before_deleting_files() {
        for (label, manifest_path, manifest_contents) in [
            (
                "uninstall-runtime-traversal",
                "runtime",
                br#"{"manifestVersion":1,"managedFiles":["runtime/../data.json"]}"#.as_slice(),
            ),
            (
                "uninstall-app-traversal",
                "app",
                br#"{"manifestVersion":1,"managedFiles":["Dev/../outside.txt"]}"#.as_slice(),
            ),
        ] {
            let (root, anytype, app, _) = installed_uninstall_fixture(label);
            let blob = anytype.join("blob");
            let manifest = if manifest_path == "runtime" {
                blob.join(".blobui-managed-files.json")
            } else {
                app.join(".blobui-app-managed-files.json")
            };
            fs::write(&manifest, manifest_contents).unwrap();
            let css = fs::read(anytype.join("custom.css")).unwrap();
            let app_exe = fs::read(app.join("blobui-settings.exe")).unwrap();
            let data = fs::read(blob.join("data.json")).unwrap();

            assert!(uninstall(&app, &anytype).is_err(), "{label} was accepted");

            assert_eq!(fs::read(anytype.join("custom.css")).unwrap(), css);
            assert_eq!(fs::read(app.join("blobui-settings.exe")).unwrap(), app_exe);
            assert_eq!(fs::read(blob.join("data.json")).unwrap(), data);
            assert!(app.join(".blobui-install.json").exists());
            assert_eq!(fs::read(&manifest).unwrap(), manifest_contents);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[cfg(windows)]
    #[test]
    fn uninstall_rejects_reparse_managed_path_before_deleting_other_files() {
        use std::os::windows::fs::symlink_file;
        let (root, anytype, app, _) = installed_uninstall_fixture("uninstall-reparse");
        let link = app.join("Dev/theme.css");
        fs::remove_file(&link).unwrap();
        if let Err(error) = symlink_file(app.join("notes.txt"), &link) {
            assert_eq!(
                error.raw_os_error(),
                Some(1314),
                "unexpected failure creating a reparse fixture: {error}"
            );
            eprintln!("Skipping reparse-point integration assertion: this Windows account cannot create symbolic links (privilege 1314).");
            fs::remove_dir_all(root).unwrap();
            return;
        }
        let css = fs::read(anytype.join("custom.css")).unwrap();
        let app_exe = fs::read(app.join("blobui-settings.exe")).unwrap();

        assert!(uninstall(&app, &anytype).is_err());

        assert_eq!(fs::read(anytype.join("custom.css")).unwrap(), css);
        assert_eq!(fs::read(app.join("blobui-settings.exe")).unwrap(), app_exe);
        assert!(app.join(".blobui-install.json").exists());
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        fs::remove_dir_all(root).unwrap();
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
        create_shortcut_at(&target, &root, &link, &root).unwrap();
        assert!(link.is_file());

        let original = fs::read(&link).unwrap();
        assert!(create_shortcut_at(&target, &root, &link, &root).is_err());
        assert_eq!(fs::read(&link).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn native_shortcut_replaces_a_link_owned_by_the_same_install() {
        let root = fixture_root("shortcut-reinstall");
        let install = root.join("Anytype data");
        let old_app = root.join("old-app");
        let new_app = root.join("new-app");
        let link = root.join("BlobUI.lnk");

        for (app, executable) in [
            (&old_app, &b"old executable"[..]),
            (&new_app, &b"new executable"[..]),
        ] {
            fs::create_dir_all(app.join("settings-app/dist")).unwrap();
            fs::write(
                app.join(".blobui-install.json"),
                serde_json::to_vec(&json!({"installDirectory": install.clone()})).unwrap(),
            )
            .unwrap();
            fs::write(app.join("blobui-settings.exe"), executable).unwrap();
            fs::write(
                app.join("settings-app/dist/blobui-settings.exe"),
                executable,
            )
            .unwrap();
        }

        let old_target = old_app.join("settings-app/dist/blobui-settings.exe");
        let new_target = new_app.join("settings-app/dist/blobui-settings.exe");
        assert_eq!(
            create_shortcut_at(&old_target, &old_app, &link, &install).unwrap(),
            None
        );
        let original = fs::read(&link).unwrap();

        let replaced = create_shortcut_at(&new_target, &new_app, &link, &install).unwrap();

        assert_eq!(replaced, Some(original));
        let installed_target = read_start_menu_shortcut_target(&link)
            .unwrap()
            .unwrap();
        assert!(same_path(&installed_target, &new_target));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn uninstall_removes_only_a_start_menu_shortcut_that_targets_this_install() {
        use std::env;

        let (root, anytype, app, _) = installed_uninstall_fixture("uninstall-shortcut-owned");
        let prior_appdata = env::var_os("APPDATA");
        env::set_var("APPDATA", root.join("roaming"));
        let owned_link = start_menu_shortcut_path();
        let target = app.join("settings-app/dist/blobui-settings.exe");
        let create_result = create_shortcut_at(&target, &app, &owned_link, &anytype);
        if let Err(error) = create_result {
            if let Some(value) = prior_appdata {
                env::set_var("APPDATA", value);
            } else {
                env::remove_var("APPDATA");
            }
            panic!("could not create owned shortcut fixture: {error}");
        }
        let uninstall_result = uninstall(&app, &anytype);
        if let Some(value) = prior_appdata {
            env::set_var("APPDATA", value);
        } else {
            env::remove_var("APPDATA");
        }
        uninstall_result.unwrap();
        assert!(
            !owned_link.exists(),
            "the install-owned shortcut should be removed"
        );
        fs::remove_dir_all(root).unwrap();

        let (root, anytype, app, _) = installed_uninstall_fixture("uninstall-shortcut-foreign");
        let prior_appdata = env::var_os("APPDATA");
        env::set_var("APPDATA", root.join("roaming"));
        let foreign_target = root.join("other-app.exe");
        fs::write(&foreign_target, b"another app").unwrap();
        let foreign_link = start_menu_shortcut_path();
        let create_result = create_shortcut_at(&foreign_target, &root, &foreign_link, &anytype);
        if let Err(error) = create_result {
            if let Some(value) = prior_appdata {
                env::set_var("APPDATA", value);
            } else {
                env::remove_var("APPDATA");
            }
            panic!("could not create foreign shortcut fixture: {error}");
        }
        let uninstall_result = uninstall(&app, &anytype);
        if let Some(value) = prior_appdata {
            env::set_var("APPDATA", value);
        } else {
            env::remove_var("APPDATA");
        }
        uninstall_result.unwrap();
        assert!(
            foreign_link.is_file(),
            "a shortcut to another app must be preserved"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

