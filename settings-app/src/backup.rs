use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const META_FILE: &str = "blob.meta.json";
const DATA_FILE: &str = "data.json";
const MAX_SETTINGS_BYTES: u64 = 16 * 1024 * 1024;

pub fn metadata_bytes() -> Result<Vec<u8>, String> {
    crate::storage::blob_metadata_bytes()
}

pub fn ensure_metadata(blob: &Path) -> Result<(), String> {
    crate::storage::reject_reparse_path(blob)?;
    let path = blob.join(META_FILE);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => crate::storage::reject_reparse_path(&path),
        Ok(_) => Err("The BlobUI folder metadata is not a regular file.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::storage::write_atomic(&path, &metadata_bytes()?)
        }
        Err(error) => Err(format!("Could not inspect BlobUI folder metadata: {error}")),
    }
}

pub fn create_backup(blob: &Path, destination_parent: &Path) -> Result<PathBuf, String> {
    let blob = canonical_directory(blob, "The configured BlobUI folder")?;
    let destination_parent = canonical_directory(destination_parent, "The backup destination")?;
    reject_tree_links(&blob)?;
    if destination_parent.starts_with(&blob) {
        return Err("Choose a backup destination outside the configured BlobUI folder.".into());
    }
    ensure_metadata(&blob)?;
    let destination = destination_parent.join(format!("BlobUI backup {}", timestamp()));
    if destination.exists() {
        return Err("A backup with this name already exists. Choose another destination and try again.".into());
    }
    if let Err(error) = copy_tree(&blob, &destination) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    Ok(destination)
}

pub fn import_backup(install: &Path, backup: &Path) -> Result<(), String> {
    let install = canonical_directory(install, "The configured Anytype folder")?;
    let blob = install.join("blob");
    crate::storage::reject_reparse_path(&blob)?;
    if blob.is_dir() {
        reject_tree_links(&blob)?;
    }
    let backup = canonical_directory(backup, "The selected backup folder")?;
    crate::storage::reject_reparse_path(&backup)?;
    if backup.starts_with(&blob) || blob.starts_with(&backup) {
        return Err("Choose a backup folder outside the configured BlobUI folder.".into());
    }
    reject_tree_links(&backup)?;

    let stage = unique_sibling(&blob, "import");
    if let Err(error) = copy_tree(&backup, &stage) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let prepared = validate_import(&stage, &install);
    if let Err(error) = prepared {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let css = match (|| {
        let data = read_regular_json(&stage.join(DATA_FILE), MAX_SETTINGS_BYTES, "BlobUI settings")?;
        let runtime = if stage.join("runtime/Dev").is_dir() && stage.join("runtime/assets").is_dir() {
            stage.join("runtime")
        } else {
            crate::resolve_runtime_directory_for_install(&install)?
        };
        crate::css::compile(&runtime, &install, &data)
    })() {
        Ok(css) => css,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };
    let result = swap_directory_with(&blob, &stage, || {
        crate::storage::write_atomic(&install.join("custom.css"), css.as_bytes())
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

fn validate_import(blob: &Path, install: &Path) -> Result<(), String> {
    reject_tree_links(blob)?;
    let data_path = blob.join(DATA_FILE);
    let mut data = read_regular_json(&data_path, MAX_SETTINGS_BYTES, "BlobUI settings")?;
    if !data.is_object() {
        return Err("The selected folder is not a BlobUI backup: data.json must contain an object.".into());
    }
    crate::calendar::configuration(&data)?;
    let formula_path = blob.join(".blobui-kanban-formulas.json");
    crate::kanban::initialize_imported_configuration(&formula_path)?;
    let metadata_path = blob.join(META_FILE);
    if metadata_path.exists() {
        let metadata = fs::symlink_metadata(&metadata_path)
            .map_err(|_| "Could not inspect BlobUI folder metadata in the selected backup.".to_string())?;
        if !metadata.is_file() || metadata.len() > 64 * 1024 {
            return Err("The selected backup contains unsupported BlobUI folder metadata.".into());
        }
    }

    // Use the same migration and normalization as an ordinary Settings save.
    let before = data.clone();
    let mut settings = crate::settings_from_data(&data);
    let (mode, preset, color) = crate::accent_for_ui(&settings.accent_color);
    if settings.accent_mode == "preset" && mode == "preset" && preset == settings.accent_preset {
        settings.accent_color = color;
    }
    let settings = crate::validate_settings(settings)?;
    crate::apply_settings(&mut data, install, &settings);
    if data != before {
        let mut bytes = serde_json::to_vec_pretty(&data)
            .map_err(|_| "Could not update the restored Anytype folder path.".to_string())?;
        bytes.push(b'\n');
        crate::storage::write_atomic(&data_path, &bytes)?;
        crate::storage::write_atomic(&blob.join(META_FILE), &metadata_bytes()?)?;
    }
    Ok(())
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

#[cfg(test)]
fn swap_directory(current: &Path, staged: &Path) -> Result<(), String> {
    swap_directory_with(current, staged, || Ok(()))
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
    fn backup_copies_hidden_files_and_keeps_an_existing_blob_version() {
        let root = fixture_root("copy");
        let blob = root.join("Anytype/blob");
        let destination = root.join("outside");
        fs::create_dir_all(blob.join("nested/.hidden")).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(
            blob.join(DATA_FILE),
            br#"{"installDirectory":"fixture","kanbanFormulaResults":{"results":[{"categoryId":"done"}]}}"#,
        )
        .unwrap();
        fs::write(blob.join(".blobui-kanban-formulas.json"), br#"{"version":2,"display":"balanced","definitions":[{"target":{"spaceId":"space","objectId":"object","blockId":"board","viewId":"view"},"tokens":[{"kind":"count"}]}]}"#).unwrap();
        fs::write(blob.join("nested/.hidden/private.txt"), b"hidden user data").unwrap();
        fs::write(
            blob.join(META_FILE),
            br#"{"formatVersion":1,"blobuiVersion":"Dev+oldcommit"}"#,
        )
        .unwrap();

        let backup = create_backup(&blob, &destination).unwrap();
        assert_eq!(
            fs::read(backup.join("nested/.hidden/private.txt")).unwrap(),
            b"hidden user data"
        );
        assert_eq!(
            fs::read(backup.join(".blobui-kanban-formulas.json")).unwrap(),
            br#"{"version":2,"display":"balanced","definitions":[{"target":{"spaceId":"space","objectId":"object","blockId":"board","viewId":"view"},"tokens":[{"kind":"count"}]}]}"#
        );
        let saved_data: Value = serde_json::from_slice(&fs::read(backup.join(DATA_FILE)).unwrap()).unwrap();
        assert_eq!(saved_data["kanbanFormulaResults"]["results"][0]["categoryId"], "done");
        assert_eq!(
            fs::read(backup.join(META_FILE)).unwrap(),
            br#"{"formatVersion":1,"blobuiVersion":"Dev+oldcommit"}"#
        );
        assert_eq!(
            fs::read(blob.join(META_FILE)).unwrap(),
            br#"{"formatVersion":1,"blobuiVersion":"Dev+oldcommit"}"#
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_adds_metadata_only_when_the_folder_has_none() {
        let root = fixture_root("metadata");
        let blob = root.join("Anytype/blob");
        let destination = root.join("outside");
        fs::create_dir_all(&blob).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(blob.join(DATA_FILE), b"{}").unwrap();

        let backup = create_backup(&blob, &destination).unwrap();
        let metadata: Value = serde_json::from_slice(&fs::read(backup.join(META_FILE)).unwrap()).unwrap();
        assert_eq!(metadata["blobuiVersion"], env!("BLOBUI_BUILD_VERSION"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_import_keeps_the_existing_folder_and_failed_swap_rolls_back() {
        let root = fixture_root("rollback");
        let install = root.join("Anytype");
        let blob = install.join("blob");
        let invalid = root.join("invalid backup");
        fs::create_dir_all(blob.join("nested")).unwrap();
        fs::create_dir_all(&invalid).unwrap();
        fs::write(blob.join(DATA_FILE), b"old settings").unwrap();
        fs::write(blob.join("nested/existing.txt"), b"existing file").unwrap();
        fs::write(invalid.join("not-a-backup.txt"), b"invalid").unwrap();
        let error = import_backup(&install, &invalid).unwrap_err();
        assert!(error.contains("BlobUI settings is missing"));
        assert_eq!(fs::read(blob.join(DATA_FILE)).unwrap(), b"old settings");

        let staged = root.join("missing stage");
        let error = swap_directory(&blob, &staged).unwrap_err();
        assert!(error.contains("Could not restore"));
        assert_eq!(fs::read(blob.join("nested/existing.txt")).unwrap(), b"existing file");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_post_swap_css_commit_restores_the_previous_blob_folder() {
        let root = fixture_root("css-rollback");
        let current = root.join("Anytype/blob");
        let staged = root.join("staged");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&staged).unwrap();
        fs::write(current.join(DATA_FILE), b"prior settings").unwrap();
        fs::write(staged.join(DATA_FILE), b"imported settings").unwrap();

        let error = swap_directory_with(&current, &staged, || Err("fixture CSS write failed".into())).unwrap_err();
        assert!(error.contains("fixture CSS write failed"));
        assert_eq!(fs::read(current.join(DATA_FILE)).unwrap(), b"prior settings");
        assert!(!staged.exists());
        assert_eq!(fs::read_dir(root.join("Anytype")).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn import_rebases_the_install_path_and_updates_version_only_when_it_writes_settings() {
        let root = fixture_root("import");
        let install = root.join("New Anytype");
        let backup = root.join("Old Anytype/blob");
        fs::create_dir_all(&install).unwrap();
        fs::create_dir_all(backup.join("runtime/Dev")).unwrap();
        fs::create_dir_all(backup.join("runtime/assets/spacebanner")).unwrap();
        let mut data = json!({});
        crate::apply_settings(
            &mut data,
            &root.join("Old Anytype"),
            &crate::SettingsDraft::default(),
        );
        // Older backups can contain only the local settings representation.
        data.as_object_mut().unwrap().remove("settings");
        fs::write(backup.join(DATA_FILE), serde_json::to_vec(&data).unwrap()).unwrap();
        fs::write(backup.join("runtime/Dev/theme.css"), b"/* fixture */").unwrap();
        fs::write(backup.join("runtime/assets/spacebanner/banner.png"), b"fixture image").unwrap();
        fs::write(backup.join(META_FILE), br#"{"formatVersion":1,"blobuiVersion":"Dev+oldcommit"}"#).unwrap();

        import_backup(&install, &backup).unwrap();
        let data: Value = serde_json::from_slice(&fs::read(install.join("blob/data.json")).unwrap()).unwrap();
        let metadata: Value = serde_json::from_slice(&fs::read(install.join("blob/blob.meta.json")).unwrap()).unwrap();
        assert_eq!(data["installDirectory"], install.canonicalize().unwrap().to_string_lossy().as_ref());
        assert_eq!(metadata["blobuiVersion"], env!("BLOBUI_BUILD_VERSION"));
        assert_eq!(data["settings"]["fontFamily"], "Inter");
        assert_eq!(data["settings"]["includeSelections"], true);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn importing_a_folder_without_calculations_does_not_restore_workspace_legacy_definitions() {
        let root = fixture_root("empty-formulas");
        let install = root.join("Anytype");
        let blob = root.join("staged/blob");
        let legacy = root.join("workspace/.blobui-kanban-formulas.json");
        let prototype = root.join("workspace/.blobui-formula-prototype.json");
        fs::create_dir_all(&blob).unwrap();
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        let data = json!({ "installDirectory": install.to_string_lossy() });
        fs::write(blob.join(DATA_FILE), serde_json::to_vec(&data).unwrap()).unwrap();
        fs::write(&legacy, br#"{"version":2,"display":"spacious","definitions":[]}"#).unwrap();

        validate_import(&blob, &install).unwrap();
        let configuration = crate::kanban::load_at(
            &blob.join(".blobui-kanban-formulas.json"),
            &legacy,
            &prototype,
        )
        .unwrap();
        assert_eq!(configuration.display, "balanced");
        assert!(configuration.definitions.is_empty());
        assert_eq!(
            fs::read(&legacy).unwrap(),
            br#"{"version":2,"display":"spacious","definitions":[]}"#
        );
        fs::remove_dir_all(root).unwrap();
    }
}
