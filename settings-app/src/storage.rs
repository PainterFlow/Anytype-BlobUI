use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
thread_local! {
    pub(crate) static FAIL_AFTER_REPLACE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

pub enum CredentialChange<'a> {
    Keep,
    Replace(&'a str),
    Remove,
}

pub(crate) struct Snapshot {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
    pub(crate) permissions: Option<fs::Permissions>,
    private: bool,
}

pub fn commit_settings(
    css_path: &Path,
    data_path: &Path,
    credential_path: &Path,
    css: &[u8],
    data: &[u8],
    credential: CredentialChange<'_>,
) -> Result<(), String> {
    if let Some(parent) = credential_path.parent() {
        reject_reparse_path(parent)?;
        fs::create_dir_all(parent).map_err(|error| format!("Could not prepare the API credential folder: {error}"))?;
    }
    let mut targets = vec![(css_path.to_path_buf(), false), (data_path.to_path_buf(), false)];
    if !matches!(credential, CredentialChange::Keep) { targets.push((credential_path.to_path_buf(), true)); }
    let mut snapshots = Vec::new();
    for (path, private) in &targets {
        reject_reparse_path(path)?;
        snapshots.push(snapshot(path, *private)?);
        preflight(path)?;
    }

    let css_temp = prepare_temp(css_path, css, snapshots.iter().find(|snapshot| snapshot.path == css_path).and_then(|snapshot| snapshot.permissions.as_ref()), false)?;
    let data_temp = match prepare_temp(data_path, data, snapshots.iter().find(|snapshot| snapshot.path == data_path).and_then(|snapshot| snapshot.permissions.as_ref()), false) {
        Ok(path) => path,
        Err(error) => { remove_if_exists(&css_temp); return Err(error); }
    };
    let credential_temp = match credential {
        CredentialChange::Replace(protected_text) => {
            let mut contents = protected_text.as_bytes().to_vec();
            contents.push(b'\n');
            let prepared = prepare_temp(credential_path, &contents, None, true);
            contents.fill(0);
            match prepared {
                Ok(path) => Some(path),
                Err(error) => { remove_if_exists(&css_temp); remove_if_exists(&data_temp); return Err(error); }
            }
        }
        _ => None,
    };

    let commit_result = (|| {
        replace_file(&css_temp, css_path).map_err(|error| format!("Could not replace custom.css: {error}"))?;
        replace_file(&data_temp, data_path).map_err(|error| format!("Could not replace blob/data.json: {error}"))?;
        match credential {
            CredentialChange::Keep => {}
            CredentialChange::Replace(_) => {
                let candidate = credential_temp.as_ref().ok_or_else(|| "The encrypted API key candidate was not prepared.".to_string())?;
                replace_credential(candidate, credential_path)
                    .map_err(|error| format!("Could not replace the encrypted Anytype API key: {error}"))?;
            }
            CredentialChange::Remove => {
                if credential_path.exists() {
                    fs::remove_file(credential_path).map_err(|error| format!("Could not remove the encrypted Anytype API key: {error}"))?;
                }
            }
        }
        Ok(())
    })();

    if let Some(candidate) = &credential_temp { remove_if_exists(candidate); }
    remove_if_exists(&css_temp);
    remove_if_exists(&data_temp);
    if let Err(error) = commit_result {
        let mut rollback_errors = Vec::new();
        // A failed Windows replacement can report failure after it has altered
        // the destination. Reconcile all snapshots instead of trusting flags.
        for snapshot in snapshots.iter().rev() {
            if let Err(rollback) = restore(snapshot) { rollback_errors.push(rollback); }
        }
        if rollback_errors.is_empty() { return Err(error); }
        return Err(format!("{error} Rollback could not restore every saved file: {}", rollback_errors.join("; ")));
    }
    Ok(())
}

pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    reject_reparse_path(path)?;
    let before = snapshot(path, false)?;
    preflight(path)?;
    let candidate = prepare_temp(path, contents, before.permissions.as_ref(), false)?;
    let result = replace_file(&candidate, path)
        .map_err(|error| format!("Could not save {}: {error}", path.display()));
    remove_if_exists(&candidate);
    if let Err(error) = result {
        return match restore(&before) {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!("{error} {rollback}")),
        };
    }
    Ok(())
}

pub(crate) fn snapshot(path: &Path, private: bool) -> Result<Snapshot, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                return Err(format!("A BlobUI save target is a link and cannot be changed safely: {}", path.display()));
            }
            if !metadata.is_file() {
                return Err(format!("A BlobUI save target is not a regular file: {}", path.display()));
            }
            let bytes = fs::read(path).map_err(|error| format!("Could not inspect {} before saving: {error}", path.display()))?;
            Ok(Snapshot { path: path.to_path_buf(), bytes: Some(bytes), permissions: Some(metadata.permissions()), private })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Snapshot { path: path.to_path_buf(), bytes: None, permissions: None, private }),
        Err(error) => Err(format!("Could not inspect {} before saving: {error}", path.display())),
    }
}

pub(crate) fn preflight(path: &Path) -> Result<(), String> {
    reject_reparse_path(path)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.is_file() { return Err(format!("A BlobUI save target is not a regular file: {}", path.display())); }
        if metadata.permissions().readonly() { return Err(format!("A BlobUI save target is read-only: {}", path.display())); }
        OpenOptions::new().write(true).open(path)
            .map_err(|error| format!("Could not prepare to replace {}: {error}", path.display()))?;
    }
    let parent = path.parent().ok_or_else(|| format!("The save target has no parent directory: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("Could not prepare {}: {error}", parent.display()))?;
    let probe = unique_sibling(path, "check");
    let file = OpenOptions::new().write(true).create_new(true).open(&probe)
        .map_err(|error| format!("Could not prepare the save folder for {}: {error}", path.display()))?;
    drop(file);
    fs::remove_file(&probe).map_err(|error| format!("Could not clear the save-folder check for {}: {error}", path.display()))?;
    Ok(())
}

pub(crate) fn prepare_temp(path: &Path, contents: &[u8], permissions: Option<&fs::Permissions>, private: bool) -> Result<PathBuf, String> {
    let parent = path.parent().ok_or_else(|| format!("The save target has no parent directory: {}", path.display()))?;
    reject_reparse_path(parent)?;
    fs::create_dir_all(parent).map_err(|error| format!("Could not prepare {}: {error}", parent.display()))?;
    let temp = unique_sibling(path, "tmp");
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)
            .map_err(|error| format!("Could not create a temporary file for {}: {error}", path.display()))?;
        file.write_all(contents).map_err(|error| format!("Could not write a temporary file for {}: {error}", path.display()))?;
        file.sync_all().map_err(|error| format!("Could not flush a temporary file for {}: {error}", path.display()))?;
        drop(file);
        if private { restrict_credential_acl(&temp)?; }
        else if let Some(permissions) = permissions {
            fs::set_permissions(&temp, permissions.clone())
                .map_err(|error| format!("Could not preserve permissions for {}: {error}", path.display()))?;
        }
        Ok(())
    })();
    if let Err(error) = result { remove_if_exists(&temp); return Err(error); }
    Ok(temp)
}

pub(crate) fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let result = {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, ReplaceFileW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, REPLACEFILE_WRITE_THROUGH};
        let source_wide = source.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let destination_wide = destination.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let replaced = unsafe {
            if destination.exists() {
                ReplaceFileW(destination_wide.as_ptr(), source_wide.as_ptr(), std::ptr::null(), REPLACEFILE_WRITE_THROUGH, std::ptr::null(), std::ptr::null())
            } else {
                MoveFileExW(source_wide.as_ptr(), destination_wide.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)
            }
        };
        if replaced == 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
    };
    #[cfg(not(windows))]
    let result = fs::rename(source, destination);
    maybe_fail_after_replace(destination, result)
}

fn replace_credential(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let result = {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
        let source_wide = source.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let destination_wide = destination.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let result = unsafe { MoveFileExW(source_wide.as_ptr(), destination_wide.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) };
        if result == 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
    };
    #[cfg(not(windows))]
    let result = fs::rename(source, destination);
    maybe_fail_after_replace(destination, result)
}

fn maybe_fail_after_replace(_destination: &Path, result: std::io::Result<()>) -> std::io::Result<()> {
    result?;
    #[cfg(test)]
    {
        let should_fail = FAIL_AFTER_REPLACE.with(|fail| fail.borrow().as_deref() == Some(_destination));
        if should_fail {
            FAIL_AFTER_REPLACE.with(|fail| { fail.borrow_mut().take(); });
            // Simulate a Win32 replacement that changed the destination before it
            // returned an error, so rollback must inspect snapshots rather than flags.
            return Err(std::io::Error::other("injected post-replacement failure"));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn restrict_credential_acl(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SetFileSecurityW};
    let sddl: Vec<u16> = "D:P(A;;FA;;;OW)\0".encode_utf16().collect();
    let path_wide = path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
    let mut descriptor = std::ptr::null_mut();
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut descriptor, std::ptr::null_mut()) == 0 {
            return Err("Windows could not prepare private permissions for the encrypted API key.".into());
        }
        let success = SetFileSecurityW(path_wide.as_ptr(), DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION, descriptor);
        LocalFree(descriptor.cast());
        if success == 0 { return Err("Windows could not restrict the encrypted API key to its owner.".into()); }
    }
    Ok(())
}

#[cfg(not(windows))]
fn restrict_credential_acl(_: &Path) -> Result<(), String> { Ok(()) }

pub(crate) fn restore(snapshot: &Snapshot) -> Result<(), String> {
    match &snapshot.bytes {
        Some(bytes) => {
            if fs::read(&snapshot.path).ok().as_deref() != Some(bytes.as_slice()) {
                let temp = prepare_temp(&snapshot.path, bytes, snapshot.permissions.as_ref(), snapshot.private)?;
                replace_file(&temp, &snapshot.path).map_err(|error| {
                    remove_if_exists(&temp);
                    format!("Could not restore {}: {error}", snapshot.path.display())
                })?;
                remove_if_exists(&temp);
            }
            if let Some(permissions) = &snapshot.permissions {
                if let Ok(metadata) = fs::metadata(&snapshot.path) {
                    if metadata.permissions().readonly() != permissions.readonly() {
                        fs::set_permissions(&snapshot.path, permissions.clone())
                            .map_err(|error| format!("Could not restore permissions for {}: {error}", snapshot.path.display()))?;
                    }
                }
            }
            Ok(())
        }
        None => {
            match fs::remove_file(&snapshot.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("Could not remove a new save file {}: {error}", snapshot.path.display())),
            }
        }
    }
}

pub(crate) fn reject_reparse_path(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() || is_reparse_point(&metadata) => {
                return Err("A BlobUI save path contains a symbolic link or reparse point.".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect a BlobUI save path: {error}")),
        }
    }
    Ok(())
}

fn unique_sibling(path: &Path, suffix: &str) -> PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}-{nonce}.{suffix}", std::process::id()))
}

pub(crate) fn remove_if_exists(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &fs::Metadata) -> bool { false }

#[path = "../../installer/src/transaction.rs"]
pub(crate) mod transaction;

#[cfg(test)]
mod tests {
    use super::*;
    use super::transaction::{commit_update, FileChange};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn restores_css_data_and_private_credential_after_partial_replacement_failure() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = workspace.join(".test-run").join(format!("rollback-{}-{nonce}", std::process::id()));
        let blob = root.join("blob");
        let css_path = root.join("custom.css");
        let data_path = blob.join("data.json");
        let credential_path = blob.join("credentials/anytype-api-key.dpapi");
        fs::create_dir_all(credential_path.parent().unwrap()).unwrap();
        fs::write(&css_path, b"old css").unwrap();
        fs::write(&data_path, b"old data").unwrap();
        fs::write(&credential_path, b"old protected key").unwrap();
        FAIL_AFTER_REPLACE.with(|fail| *fail.borrow_mut() = Some(credential_path.clone()));

        let error = commit_settings(
            &css_path,
            &data_path,
            &credential_path,
            b"new css",
            b"new data",
            CredentialChange::Replace("new protected key"),
        )
        .unwrap_err();
        assert!(error.contains("Could not replace the encrypted Anytype API key"));
        assert_eq!(fs::read(&css_path).unwrap(), b"old css");
        assert_eq!(fs::read(&data_path).unwrap(), b"old data");
        assert_eq!(fs::read(&credential_path).unwrap(), b"old protected key");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_update_restores_css_manifest_managed_files_and_leaves_user_data_untouched() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = workspace.join(".test-run").join(format!("update-rollback-{}-{nonce}", std::process::id()));
        let blob = root.join("blob");
        let css = root.join("custom.css");
        let data = blob.join("data.json");
        let credential = blob.join("credentials/anytype-api-key.dpapi");
        let managed = blob.join("runtime/Dev/theme.css");
        let obsolete = blob.join("runtime/Dev/obsolete.css");
        let manifest = blob.join(".blobui-managed-files.json");
        let app_executable = root.join("BlobUI/settings-app/dist/blobui-settings.exe");
        fs::create_dir_all(credential.parent().unwrap()).unwrap();
        fs::create_dir_all(managed.parent().unwrap()).unwrap();
        fs::create_dir_all(app_executable.parent().unwrap()).unwrap();
        fs::write(&css, b"old generated css").unwrap();
        fs::write(&data, b"unchanged Anytype data").unwrap();
        fs::write(&credential, b"unchanged DPAPI credential").unwrap();
        fs::write(&managed, b"old managed theme").unwrap();
        fs::write(&obsolete, b"old obsolete managed file").unwrap();
        fs::write(&manifest, b"old ownership manifest").unwrap();
        fs::write(&app_executable, b"old app executable").unwrap();

        FAIL_AFTER_REPLACE.with(|fail| *fail.borrow_mut() = Some(manifest.clone()));
        let error = commit_update(vec![
            FileChange { path: css.clone(), contents: Some(b"new generated css".to_vec()) },
            FileChange { path: managed.clone(), contents: Some(b"new managed theme".to_vec()) },
            FileChange { path: obsolete.clone(), contents: None },
            FileChange { path: app_executable.clone(), contents: Some(b"new app executable".to_vec()) },
            FileChange { path: manifest.clone(), contents: Some(b"new ownership manifest".to_vec()) },
        ]).unwrap_err();

        assert!(error.contains("injected post-replacement failure"));
        assert_eq!(fs::read(&css).unwrap(), b"old generated css");
        assert_eq!(fs::read(&manifest).unwrap(), b"old ownership manifest");
        assert_eq!(fs::read(&managed).unwrap(), b"old managed theme");
        assert_eq!(fs::read(&obsolete).unwrap(), b"old obsolete managed file");
        assert_eq!(fs::read(&app_executable).unwrap(), b"old app executable");
        assert_eq!(fs::read(&data).unwrap(), b"unchanged Anytype data");
        assert_eq!(fs::read(&credential).unwrap(), b"unchanged DPAPI credential");
        fs::remove_dir_all(root).unwrap();
    }
}

