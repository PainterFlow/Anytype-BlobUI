use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const META_FILE: &str = "blob.meta.json";
const DATA_FILE: &str = "data.json";
const SETTINGS_DIR: &str = "Settings";
const MAX_SETTINGS_BYTES: u64 = 64 * 1024 * 1024;

pub fn metadata_bytes() -> Result<Vec<u8>, String> {
    crate::storage::blob_metadata_bytes()
}

pub fn create_backup(settings_root: &Path, destination_parent: &Path) -> Result<PathBuf, String> {
    let settings_root = canonical_directory(settings_root, "The configured BlobUI app folder")?;
    let settings = canonical_directory(&settings_root.join(SETTINGS_DIR), "The saved Settings folder")?;
    let destination_parent = canonical_directory(destination_parent, "The backup destination")?;
    reject_tree_links(&settings)?;
    // Refuse to export an incomplete store. A backup that cannot be imported
    // would create false confidence about the user's saved settings.
    crate::settings_store::load(&settings_root)?;
    if destination_parent.starts_with(&settings) {
        return Err("Choose a backup destination outside the saved Settings folder.".into());
    }
    let destination = destination_parent.join(format!("BlobUI backup {}", timestamp()));
    if destination.exists() {
        return Err("A backup with this name already exists. Choose another destination and try again.".into());
    }
    if let Err(error) = copy_tree(&settings, &destination) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    let private_credential = destination.join("DO-NOT-SHARE/anytype-api-key.dpapi");
    if private_credential.is_file() {
        if let Err(error) = crate::storage::restrict_credential_acl(&private_credential) {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
    }
    Ok(destination)
}

pub fn import_backup(settings_root: &Path, anytype_data_root: &Path, backup: &Path) -> Result<(), String> {
    let settings_root = canonical_directory(settings_root, "The configured BlobUI app folder")?;
    let anytype_data_root = canonical_directory(anytype_data_root, "The configured Anytype folder")?;
    let backup = canonical_directory(backup, "The selected backup folder")?;
    crate::storage::reject_reparse_path(&backup)?;
    let current_settings = settings_root.join(SETTINGS_DIR);
    if backup.starts_with(&current_settings) || current_settings.starts_with(&backup) {
        return Err("Choose a backup folder outside the saved Settings folder.".into());
    }
    reject_tree_links(&backup)?;

    let stage_root = unique_sibling(&settings_root, "import-root");
    fs::create_dir(&stage_root).map_err(|error| format!("Could not stage imported Settings: {error}"))?;
    let stage_result = (|| {
        let split_backup = has_split_settings_marker(&backup)?;
        if split_backup {
            let staged_settings = stage_root.join(SETTINGS_DIR);
            copy_tree(&backup, &staged_settings)?;
        } else {
            // Older exports contained the Anytype `blob` directory directly.
            // Convert those files into the split layout without changing the
            // selected backup or the current installation.
            let legacy_blob = stage_root.join("legacy-blob");
            copy_tree(&backup, &legacy_blob)?;
            let formula_path = legacy_blob.join(".blobui-kanban-formulas.json");
            if formula_path.exists() {
                crate::kanban::validate_saved_configuration(&formula_path)?;
            }
            crate::settings_store::migrate_legacy_blob(&legacy_blob, &stage_root)?;
        }

        let mut data = crate::settings_store::load(&stage_root)?;
        if !data.is_object() {
            return Err("The selected folder is not a BlobUI backup: settings must contain an object.".into());
        }
        let current_data = if settings_root.join(SETTINGS_DIR).exists() {
            crate::settings_store::load(&settings_root)?
        } else {
            serde_json::json!({})
        };
        preserve_current_credential_if_import_is_unreadable(
            &stage_root,
            &settings_root,
            &mut data,
            &current_data,
        )?;
        if let Some(keybinds) = data.get("keybinds") {
            data["keybinds"] = crate::hotkey::normalize_shortcut_value(keybinds)
                .map_err(|_| "The selected Settings backup contains invalid keyboard shortcuts.".to_string())?;
        }
        validate_updater_preferences(&data)?;
        crate::calendar::configuration(&data)?;
        let mut settings = crate::settings_from_data(&data);
        let (mode, preset, color) = crate::accent_for_ui(&settings.accent_color);
        if settings.accent_mode == "preset" && mode == "preset" && preset == settings.accent_preset {
            settings.accent_color = color;
        }
        let settings = crate::validate_settings(settings)?;
        crate::apply_settings(&mut data, &anytype_data_root, &settings);
        if data.pointer("/updaterPreferences/installDirectory").and_then(Value::as_str).is_some() {
            data["updaterPreferences"]["installDirectory"] = Value::String(anytype_data_root.to_string_lossy().into_owned());
        }
        let formula_configuration = validate_saved_formula_configuration(&stage_root)?;
        // Normalize supported settings in the staged copy. The store preserves
        // unknown top-level, nested, and future fields while it writes.
        crate::settings_store::save(&stage_root, &data)?;

        let runtime = crate::resolve_runtime_directory_for_install(&anytype_data_root)?;
        let css = crate::css::compile(&runtime, &anytype_data_root, &data)?;
        let serialized = crate::serialize_data(&data)?;
        let staged_settings = stage_root.join(SETTINGS_DIR);
        swap_directory_with(&current_settings, &staged_settings, || {
            publish_import_files(&anytype_data_root, css.as_bytes(), &serialized, formula_configuration.as_deref())
        })
    })();
    let _ = fs::remove_dir_all(&stage_root);
    stage_result
}

fn preserve_current_credential_if_import_is_unreadable(
    staged_root: &Path,
    current_root: &Path,
    imported_data: &mut Value,
    current_data: &Value,
) -> Result<(), String> {
    let relative = "Settings/DO-NOT-SHARE/anytype-api-key.dpapi";
    let imported_path = staged_root.join(relative);
    let imported_bytes = match read_optional_credential(&imported_path)? {
        Some(bytes) => bytes,
        None => return Ok(()),
    };
    let imported_text = String::from_utf8(imported_bytes.clone()).ok();
    let imported_is_readable = imported_text
        .as_deref()
        .map(str::trim)
        .and_then(|encrypted| crate::connection::decrypt_saved_key(encrypted).ok())
        .is_some_and(|plaintext| {
            let valid = !plaintext.is_empty()
                && plaintext.len() <= 4096
                && !plaintext.chars().any(char::is_control);
            let secret = crate::SecretString::new(plaintext);
            drop(secret);
            valid
        });
    if imported_is_readable {
        return Ok(());
    }

    let current_path = current_root.join(relative);
    match read_optional_credential(&current_path)? {
        Some(bytes) => {
            if let Some(parent) = imported_path.parent() {
                fs::create_dir_all(parent).map_err(|_| "Could not preserve the current API credential.".to_string())?;
            }
            fs::write(&imported_path, bytes).map_err(|_| "Could not preserve the current API credential.".to_string())?;
            crate::storage::restrict_credential_acl(&imported_path)?;
        }
        None => {
            match fs::symlink_metadata(&imported_path) {
                Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() && !is_reparse_point(&metadata) => {
                    fs::remove_file(&imported_path).map_err(|_| "Could not remove an unreadable imported API credential.".to_string())?;
                }
                Ok(_) => return Err("The imported API credential is not a regular file.".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Could not inspect the imported API credential.".into()),
            }
        }
    }

    if let Some(connection) = current_data.get("apiConnection") {
        imported_data["apiConnection"] = connection.clone();
    } else if let Some(object) = imported_data.as_object_mut() {
        object.remove("apiConnection");
    }
    for field in [
        "calendarObjectColors",
        "calendarViewColors",
        "calendarColorInfo",
        "calendarColorsPublishedAt",
    ] {
        if let Some(value) = current_data.get(field) {
            imported_data[field] = value.clone();
        } else if let Some(object) = imported_data.as_object_mut() {
            object.remove(field);
        }
    }
    Ok(())
}

fn read_optional_credential(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() && !is_reparse_point(&metadata) && metadata.len() <= 64 * 1024 => {
            fs::read(path).map(Some).map_err(|_| "Could not read an API credential file.".to_string())
        }
        Ok(_) => Err("An API credential file is not a supported regular file.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Could not inspect an API credential file.".into()),
    }
}

fn validate_updater_preferences(data: &Value) -> Result<(), String> {
    let Some(preferences) = data.get("updaterPreferences") else {
        return Ok(());
    };
    let preferences = preferences
        .as_object()
        .ok_or("The selected Settings backup contains malformed updater preferences.")?;
    if preferences.get("installDirectory").is_some_and(|value| !value.is_null() && !value.is_string())
        || ["autoplayVideos", "automaticUpdatesEnabled", "automaticChecksEnabled"]
            .iter()
            .any(|key| preferences.get(*key).is_some_and(|value| !value.is_boolean()))
    {
        return Err("The selected Settings backup contains malformed updater preferences.".into());
    }
    Ok(())
}

fn publish_import_files(
    anytype_data_root: &Path,
    css: &[u8],
    data: &[u8],
    formula_configuration: Option<&[u8]>,
) -> Result<(), String> {
    let blob = anytype_data_root.join("blob");
    let css_path = anytype_data_root.join("custom.css");
    let data_path = blob.join(DATA_FILE);
    let metadata_path = blob.join(META_FILE);
    crate::storage::transaction::commit_update(vec![
        crate::storage::transaction::FileChange {
            path: css_path,
            contents: Some(css.to_vec()),
        },
        crate::storage::transaction::FileChange {
            path: data_path,
            contents: Some(data.to_vec()),
        },
        crate::storage::transaction::FileChange {
            path: metadata_path,
            contents: Some(metadata_bytes()?),
        },
        crate::storage::transaction::FileChange {
            path: blob.join(".blobui-kanban-formulas.json"),
            contents: formula_configuration.map(|bytes| bytes.to_vec()),
        },
    ])
}

fn has_split_settings_marker(backup: &Path) -> Result<bool, String> {
    let has_component = [
        "basic-settings.json",
        "advanced-settings.json",
        "calendar-events.json",
        "kanban-calc.json",
        "keybinds.json",
        "anyblob",
        "DO-NOT-SHARE",
    ]
    .iter()
    .any(|name| backup.join(name).exists());
    if has_component {
        return Ok(true);
    }
    let data_path = backup.join(DATA_FILE);
    if !data_path.exists() {
        return Ok(false);
    }
    let data = read_regular_json(&data_path, MAX_SETTINGS_BYTES, "BlobUI settings")?;
    Ok(data.get("formatVersion").and_then(Value::as_u64) == Some(1)
        && data.get("fields").is_some_and(Value::is_object)
        && data.get("shape").is_some_and(Value::is_object))
}

fn validate_saved_formula_configuration(settings_root: &Path) -> Result<Option<Vec<u8>>, String> {
    let path = settings_root.join(SETTINGS_DIR).join("kanban-calc.json");
    let document = read_regular_json(&path, MAX_SETTINGS_BYTES, "Kanban settings")?;
    let Some(value) = document.get("legacyConfiguration") else {
        return Ok(None);
    };
    let version = value.get("version").and_then(Value::as_u64).unwrap_or_default();
    if version == 1 {
        return serde_json::to_vec_pretty(value)
            .map(Some)
            .map_err(|_| "The selected Settings backup contains an invalid Kanban formula configuration.".into());
    }
    let configuration: crate::kanban::Configuration = serde_json::from_value(value.clone())
        .map_err(|_| "The selected Settings backup contains an invalid Kanban formula configuration.".to_string())?;
    configuration.validate()
        .map_err(|error| error.to_string())?;
    serde_json::to_vec_pretty(value)
        .map(Some)
        .map_err(|_| "The selected Settings backup contains an invalid Kanban formula configuration.".into())
}

fn read_regular_json(path: &Path, limit: u64, label: &str) -> Result<Value, String> {
    crate::storage::reject_reparse_path(path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| format!("The selected folder is not a BlobUI backup: {label} is missing."))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > limit {
        return Err(format!("The selected backup contains unsupported {label}."));
    }
    let bytes = fs::read(path).map_err(|_| format!("Could not read {label} from the selected backup."))?;
    serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
        .map_err(|_| format!("The selected backup contains invalid {label}."))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    crate::storage::reject_reparse_path(path)?;
    let resolved = path
        .canonicalize()
        .map_err(|_| format!("{label} could not be found."))?;
    if !resolved.is_dir() {
        return Err(format!("{label} is not a folder."));
    }
    Ok(resolved)
}

fn reject_tree_links(root: &Path) -> Result<(), String> {
    crate::storage::reject_reparse_path(root)?;
    let mut entries = fs::read_dir(root)
        .map_err(|error| format!("Could not read {}: {error}", root.display()))?
        .map(|entry| entry.map(|entry| entry.path()).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err("The selected folder contains a link and cannot be copied safely.".into());
        }
        if metadata.is_dir() {
            reject_tree_links(&path)?;
        } else if !metadata.is_file() {
            return Err("The selected folder contains an unsupported file type.".into());
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("Could not inspect {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err("A backup folder contains a link and cannot be copied safely.".into());
    }
    if !metadata.is_dir() {
        return Err("A backup folder contains an unsupported file type.".into());
    }
    fs::create_dir(destination)
        .map_err(|error| format!("Could not create {}: {error}", destination.display()))?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| format!("Could not read {}: {error}", source.display()))?
        .map(|entry| entry.map(|entry| entry.path()).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for entry in entries {
        let name = entry
            .file_name()
            .ok_or_else(|| "A backup path contains an invalid name.".to_string())?;
        let destination_entry = destination.join(name);
        let metadata = fs::symlink_metadata(&entry)
            .map_err(|error| format!("Could not inspect {}: {error}", entry.display()))?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err("A backup folder contains a link and cannot be copied safely.".into());
        }
        if metadata.is_dir() {
            copy_tree(&entry, &destination_entry)?;
        } else if metadata.is_file() {
            fs::copy(&entry, &destination_entry)
                .map_err(|error| format!("Could not copy {}: {error}", entry.display()))?;
        } else {
            return Err("A backup folder contains an unsupported file type.".into());
        }
    }
    fs::set_permissions(destination, metadata.permissions())
        .map_err(|error| format!("Could not preserve folder permissions: {error}"))?;
    Ok(())
}

fn swap_directory_with(
    current: &Path,
    staged: &Path,
    after_swap: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let rollback = unique_sibling(current, "rollback");
    let had_current = match fs::symlink_metadata(current) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() && !is_reparse_point(&metadata) => true,
        Ok(_) => return Err("The configured BlobUI folder is not a regular directory.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(format!("Could not inspect the configured BlobUI folder: {error}")),
    };
    if had_current {
        fs::rename(current, &rollback)
            .map_err(|error| format!("Could not prepare to restore the backup: {error}"))?;
    }
    if let Err(error) = fs::rename(staged, current) {
        if had_current {
            if let Err(rollback_error) = fs::rename(&rollback, current) {
                return Err(format!("Could not restore the backup ({error}); the previous BlobUI folder remains at {}: {rollback_error}", rollback.display()));
            }
        }
        return Err(format!("Could not restore the selected backup: {error}"));
    }
    if let Err(error) = after_swap() {
        if had_current {
            let failed = unique_sibling(current, "failed-import");
            let mut rollback_errors = Vec::new();
            if let Err(move_error) = fs::rename(current, &failed) {
                rollback_errors.push(format!("Could not move the incomplete restored folder: {move_error}"));
            } else if let Err(restore_error) = fs::rename(&rollback, current) {
                rollback_errors.push(format!("The previous BlobUI folder remains at {}: {restore_error}", rollback.display()));
            }
            let _ = fs::remove_dir_all(failed);
            if !rollback_errors.is_empty() {
                return Err(format!("{error} {}", rollback_errors.join("; ")));
            }
        } else {
            let _ = fs::rename(current, staged);
        }
        return Err(error);
    }
    if had_current {
        // If cleanup is blocked, keep the recoverable previous folder in place.
        let _ = fs::remove_dir_all(rollback);
    }
    Ok(())
}

fn unique_sibling(path: &Path, suffix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}-{nonce}.{suffix}", std::process::id()))
}

fn timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S%.3fZ").to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        root.join(".test-run").join(format!(
            "blob-backup-{label}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }

    #[test]
    fn backup_exports_only_the_complete_settings_folder_and_preserves_unknown_files() {
        let root = fixture_root("export");
        let app = root.join("app");
        let destination = root.join("outside");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&destination).unwrap();
        crate::settings_store::save(&app, &json!({
            "installDirectory": "fixture",
            "futureTopLevel": {"keep": true},
            "settings": {"fontFamily": "Inter"},
            "kanbanFormulaResults": {"results": [{"categoryId": "done"}]}
        })).unwrap();
        fs::create_dir_all(app.join(SETTINGS_DIR).join("nested/.hidden")).unwrap();
        fs::write(app.join(SETTINGS_DIR).join("nested/.hidden/private.bin"), [0x01, 0x02, 0xFE]).unwrap();
        fs::create_dir_all(app.join("blob/runtime/Dev")).unwrap();
        fs::write(app.join("blob/runtime/Dev/theme.css"), b"not part of settings").unwrap();

        let backup = create_backup(&app, &destination).unwrap();
        assert_eq!(
            fs::read(backup.join("nested/.hidden/private.bin")).unwrap(),
            [0x01, 0x02, 0xFE]
        );
        assert!(backup.join("basic-settings.json").is_file());
        assert!(backup.join(META_FILE).is_file());
        assert!(!backup.join("blob").exists());
        assert!(!backup.join("runtime").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incomplete_settings_import_keeps_current_settings_and_css_unchanged() {
        let root = fixture_root("invalid-import");
        let app = root.join("app");
        let anytype = root.join("Anytype");
        let invalid = root.join("invalid backup");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&anytype).unwrap();
        fs::create_dir_all(&invalid).unwrap();
        let original = json!({"settings":{"fontFamily":"Inter"},"unknown":{"keep":true}});
        crate::settings_store::save(&app, &original).unwrap();
        fs::write(anytype.join("custom.css"), b"old stylesheet").unwrap();
        // A component marker makes this a split backup, so it must fail as an
        // incomplete new format instead of being mistaken for a legacy blob.
        fs::write(invalid.join("basic-settings.json"), b"{}").unwrap();

        assert!(import_backup(&app, &anytype, &invalid).is_err());
        assert_eq!(crate::settings_store::load(&app).unwrap(), original);
        assert_eq!(fs::read(anytype.join("custom.css")).unwrap(), b"old stylesheet");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn split_import_rebases_path_and_preserves_future_fields_and_files() {
        let root = fixture_root("split-import");
        let source_app = root.join("SourceApp");
        let current_app = root.join("CurrentApp");
        let old_anytype = root.join("OldAnytype");
        let current_anytype = root.join("CurrentAnytype");
        let destination = root.join("backups");
        fs::create_dir_all(&source_app).unwrap();
        fs::create_dir_all(&current_app).unwrap();
        fs::create_dir_all(&old_anytype).unwrap();
        fs::create_dir_all(&current_anytype).unwrap();
        fs::create_dir_all(&destination).unwrap();
        let mut old_shortcuts = crate::hotkey::default_shortcuts();
        old_shortcuts.retain(|shortcut| shortcut.action != "openComponentCatalog");
        let mut old_shortcuts = serde_json::to_value(old_shortcuts).unwrap();
        old_shortcuts[0]["futureShortcutField"] = json!({"keep": true});
        crate::settings_store::save(&source_app, &json!({
            "installDirectory": old_anytype.to_string_lossy(),
            "keybinds": old_shortcuts,
            "futureTopLevel": {"keep": [1, 2]},
            "settingsV2": {
                "version": 9,
                "futureEnvelope": {"keep": true},
                "settings": {
                    "fontFamily": "Inter",
                    "futureSetting": {"preserve": "yes"},
                    "codexIntegration": {"spaces": {"space-id": {"access": "read", "future": true}}}
                }
            }
        })).unwrap();
        fs::write(source_app.join(SETTINGS_DIR).join("user-state.bin"), [0x10, 0x20]).unwrap();
        crate::settings_store::save(&current_app, &json!({"settings":{"fontFamily":"Serif"}})).unwrap();
        fs::write(current_anytype.join("custom.css"), b"old stylesheet").unwrap();
        let backup = create_backup(&source_app, &destination).unwrap();

        import_backup(&current_app, &current_anytype, &backup).unwrap();

        let loaded = crate::settings_store::load(&current_app).unwrap();
        assert_eq!(loaded["installDirectory"], current_anytype.canonicalize().unwrap().to_string_lossy().as_ref());
        assert_eq!(loaded["futureTopLevel"]["keep"], json!([1, 2]));
        assert_eq!(loaded["settingsV2"]["version"], 9);
        assert_eq!(loaded["settingsV2"]["futureEnvelope"]["keep"], true);
        assert_eq!(loaded["settingsV2"]["settings"]["futureSetting"]["preserve"], "yes");
        assert_eq!(loaded["settingsV2"]["settings"]["codexIntegration"]["spaces"]["space-id"]["future"], true);
        assert_eq!(loaded["keybinds"].as_array().unwrap().len(), 6);
        assert_eq!(loaded["keybinds"][0]["futureShortcutField"]["keep"], true);
        assert_eq!(fs::read(current_app.join(SETTINGS_DIR).join("user-state.bin")).unwrap(), [0x10, 0x20]);
        assert_ne!(fs::read(current_anytype.join("custom.css")).unwrap(), b"old stylesheet");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn unreadable_imported_credential_keeps_the_current_key_and_connection() {
        let root = fixture_root("credential-import");
        let source_app = root.join("SourceApp");
        let current_app = root.join("CurrentApp");
        let source_anytype = root.join("SourceAnytype");
        let current_anytype = root.join("CurrentAnytype");
        let destination = root.join("backups");
        for directory in [&source_app, &current_app, &source_anytype, &current_anytype, &destination] {
            fs::create_dir_all(directory).unwrap();
        }

        crate::settings_store::save(&source_app, &json!({
            "installDirectory": source_anytype.to_string_lossy(),
            "apiConnection": {"enabled": true, "ip": "192.0.2.10", "port": 31009},
            "settings": {"fontFamily": "Inter"}
        })).unwrap();
        fs::create_dir_all(source_app.join(SETTINGS_DIR).join("DO-NOT-SHARE")).unwrap();
        fs::write(
            source_app.join(SETTINGS_DIR).join("DO-NOT-SHARE/anytype-api-key.dpapi"),
            b"not a DPAPI credential",
        ).unwrap();

        let current_connection = json!({"enabled": true, "ip": "127.0.0.1", "port": 31010});
        crate::settings_store::save(&current_app, &json!({
            "installDirectory": current_anytype.to_string_lossy(),
            "apiConnection": current_connection,
            "settings": {"fontFamily": "Serif"}
        })).unwrap();
        let current_key = crate::connection::encrypt_api_key("current-account-key").unwrap();
        let current_credential = current_app.join(SETTINGS_DIR).join("DO-NOT-SHARE/anytype-api-key.dpapi");
        fs::create_dir_all(current_credential.parent().unwrap()).unwrap();
        fs::write(&current_credential, current_key.as_bytes()).unwrap();
        crate::storage::restrict_credential_acl(&current_credential).unwrap();

        let backup = create_backup(&source_app, &destination).unwrap();
        import_backup(&current_app, &current_anytype, &backup).unwrap();

        let imported = crate::settings_store::load(&current_app).unwrap();
        assert_eq!(imported["apiConnection"], current_connection);
        assert_eq!(fs::read(current_credential).unwrap(), current_key.as_bytes());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_blob_import_migrates_settings_without_changing_the_backup() {
        let root = fixture_root("legacy-import");
        let app = root.join("App");
        let anytype = root.join("Anytype");
        let backup = root.join("Legacy backup");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&anytype).unwrap();
        fs::create_dir_all(backup.join("assets")).unwrap();
        let source = json!({
            "installDirectory": root.join("OldAnytype").to_string_lossy(),
            "futureLegacyField": {"preserve": true},
            "settings": {"fontFamily": "Inter", "includeSelections": true}
        });
        fs::write(backup.join(DATA_FILE), serde_json::to_vec(&source).unwrap()).unwrap();
        fs::write(backup.join(META_FILE), br#"{"formatVersion":1,"blobuiVersion":"older","futureMeta":true}"#).unwrap();
        fs::write(backup.join("assets/banner.png"), [0x89, b'P', b'N', b'G']).unwrap();
        let original_data = fs::read(backup.join(DATA_FILE)).unwrap();

        import_backup(&app, &anytype, &backup).unwrap();

        let loaded = crate::settings_store::load(&app).unwrap();
        assert_eq!(loaded["futureLegacyField"]["preserve"], true);
        assert_eq!(loaded["installDirectory"], anytype.canonicalize().unwrap().to_string_lossy().as_ref());
        assert_eq!(fs::read(app.join(SETTINGS_DIR).join("assets/banner.png")).unwrap(), [0x89, b'P', b'N', b'G']);
        let metadata: Value = serde_json::from_slice(&fs::read(app.join(SETTINGS_DIR).join(META_FILE)).unwrap()).unwrap();
        assert_eq!(metadata["futureMeta"], true);
        assert_eq!(fs::read(backup.join(DATA_FILE)).unwrap(), original_data);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_post_swap_css_commit_restores_the_previous_settings_folder() {
        let root = fixture_root("css-rollback");
        let current = root.join(SETTINGS_DIR);
        let staged = root.join("staged-settings");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&staged).unwrap();
        fs::write(current.join("sentinel.txt"), b"prior settings").unwrap();
        fs::write(staged.join("sentinel.txt"), b"imported settings").unwrap();

        let error = swap_directory_with(&current, &staged, || Err("fixture CSS write failed".into())).unwrap_err();
        assert!(error.contains("fixture CSS write failed"));
        assert_eq!(fs::read(current.join("sentinel.txt")).unwrap(), b"prior settings");
        assert!(!staged.exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
