//! Fixed-destination personal marketplace registration. No shell execution.
use crate::storage;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
static INSTALL: Mutex<()> = Mutex::new(());
const PAYLOAD: &[u8] = include_bytes!("../../codex-plugin/dist/marketplace-payload.json");
const FILES: [&str; 24] = [
    ".codex-plugin/plugin.json",
    ".mcp.json",
    "README.md",
    "assets/icon.png",
    "bin/blobui-mcp.exe",
    "references/workflow-rules.md",
    "skills/search-and-answer/SKILL.md",
    "skills/search-and-answer/agents/openai.yaml",
    "skills/capture-and-organize/SKILL.md",
    "skills/capture-and-organize/agents/openai.yaml",
    "skills/maintain-pages/SKILL.md",
    "skills/maintain-pages/agents/openai.yaml",
    "skills/plan-projects/SKILL.md",
    "skills/plan-projects/agents/openai.yaml",
    "skills/review-project-progress/SKILL.md",
    "skills/review-project-progress/agents/openai.yaml",
    "skills/process-meeting-notes/SKILL.md",
    "skills/process-meeting-notes/agents/openai.yaml",
    "skills/manage-types-and-templates/SKILL.md",
    "skills/manage-types-and-templates/agents/openai.yaml",
    "skills/build-queries-and-views/SKILL.md",
    "skills/build-queries-and-views/agents/openai.yaml",
    "skills/audit-organization/SKILL.md",
    "skills/audit-organization/agents/openai.yaml",
];
fn profile() -> Result<PathBuf, String> {
    if cfg!(test) {
        return Err("Tests must supply a workspace profile directly.".into());
    }
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or("The Windows profile is unavailable.".into())
}
fn tree(path: &Path) -> Result<(), String> {
    storage::reject_reparse_path(path)?;
    if path.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| "Could not inspect the Anyblob package.")? {
            tree(
                &entry
                    .map_err(|_| "Could not inspect the Anyblob package.")?
                    .path(),
            )?;
        }
    }
    Ok(())
}
fn payload(bytes: &[u8]) -> Result<(Value, Vec<(String, Vec<u8>)>), String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "The bundled Anyblob package is missing or invalid.")?;
    let template = value["marketplace"].clone();
    if template["plugins"].as_array().is_none_or(|v| v.len() != 1)
        || template["plugins"][0]["name"] != "anyblob"
        || template["plugins"][0]["source"]["path"] != "./.codex/plugins/anyblob"
    {
        return Err("Invalid bundled marketplace.".into());
    }
    let entries = value["files"]
        .as_object()
        .ok_or("Invalid bundled Anyblob files.")?;
    if entries.len() != FILES.len() {
        return Err("Incomplete bundled Anyblob package.".into());
    }
    let files = FILES
        .iter()
        .map(|path| {
            let bytes = STANDARD
                .decode(
                    entries[*path]
                        .as_str()
                        .ok_or("Incomplete bundled Anyblob package.")?,
                )
                .map_err(|_| "Invalid bundled Anyblob file.")?;
            if bytes.is_empty() {
                return Err("Empty bundled Anyblob file.".into());
            }
            Ok((path.to_string(), bytes))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let manifest: Value =
        serde_json::from_slice(&files[0].1).map_err(|_| "Invalid Anyblob manifest.")?;
    if manifest["name"] != "anyblob" || !files[4].1.starts_with(b"MZ") {
        return Err("Invalid Anyblob executable or manifest.".into());
    }
    Ok((template, files))
}
fn catalog(path: &Path, template: &Value) -> Result<Value, String> {
    storage::reject_reparse_path(path)?;
    let mut value = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes).map_err(|_| {
            "The personal marketplace contains invalid JSON. Repair it before installing Anyblob."
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => template.clone(),
        Err(_) => return Err("Could not read the personal marketplace.".into()),
    };
    if !value["name"].as_str().is_some_and(|s| !s.is_empty())
        || !value["plugins"].is_array()
        || value["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| !p.is_object() || !p["name"].is_string())
    {
        return Err("The personal marketplace has an invalid catalog structure.".into());
    }
    if value["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"] == "anyblob")
        .count()
        > 1
    {
        return Err("The personal marketplace contains duplicate Anyblob entries.".into());
    }
    let entry = &template["plugins"][0];
    let plugins = value["plugins"].as_array_mut().unwrap();
    if let Some(old) = plugins.iter_mut().find(|p| p["name"] == "anyblob") {
        // Preserve entry metadata while updating only fields owned by this installer.
        let object = old.as_object_mut().unwrap();
        for key in ["source", "category"] {
            object.insert(key.into(), entry[key].clone());
        }
        let policy = object.entry("policy").or_insert(json!({}));
        if !policy.is_object() {
            *policy = json!({});
        }
        for key in ["installation", "authentication"] {
            policy[key] = entry["policy"][key].clone();
        }
    } else {
        plugins.push(entry.clone());
    }
    Ok(value)
}
fn installed(root: &Path, bytes: &[u8]) -> Result<bool, String> {
    let (template, files) = payload(bytes)?;
    let path = root.join(".agents/plugins/marketplace.json");
    let merged = catalog(&path, &template)?;
    let original = match fs::read(&path) {
        Ok(b) => serde_json::from_slice::<Value>(&b).map_err(|_| "Invalid marketplace.")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("Could not read the personal marketplace.".into()),
    };
    if original != merged {
        return Ok(false);
    }
    let package = root.join(".codex/plugins/anyblob");
    tree(&package)?;
    for (path, data) in files {
        if fs::read(package.join(path)).ok().as_deref() != Some(data.as_slice()) {
            return Ok(false);
        }
    }
    Ok(true)
}
fn package_version(package: &Path) -> Result<Option<String>, String> {
    let path = package.join(".codex-plugin/plugin.json");
    storage::reject_reparse_path(&path)?;
    match fs::read(&path) {
        Ok(bytes) => {
            let manifest: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "The installed Anyblob manifest is invalid.")?;
            let version = manifest["version"].as_str().ok_or("Anyblob has no release version.")?;
            semver::Version::parse(version).map_err(|_| "Invalid Anyblob release version.")?;
            Ok(Some(version.to_string()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Could not read the installed Anyblob version.".into()),
    }
}
fn installation_status(root: &Path, bytes: &[u8]) -> Result<Value, String> {
    let (_, files) = payload(bytes)?;
    let manifest: Value = serde_json::from_slice(&files[0].1).map_err(|_| "Invalid Anyblob manifest.")?;
    let bundled = manifest["version"].as_str().ok_or("Anyblob has no release version.")?;
    semver::Version::parse(bundled).map_err(|_| "Invalid bundled Anyblob release version.")?;
    let version = package_version(&root.join(".codex/plugins/anyblob"))?;
    let current = installed(root, bytes)?;
    // Codex owns its cache. Inspect the documented local copy, never overwrite it.
    let catalog_path = root.join(".agents/plugins/marketplace.json");
    storage::reject_reparse_path(&catalog_path)?;
    let catalog: Value = match fs::read(catalog_path) {
        Ok(data) => serde_json::from_slice(&data).map_err(|_| "Invalid marketplace.")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(_) => return Err("Could not read the personal marketplace.".into()),
    };
    let mut refresh = false;
    if let Some(name) = catalog["name"].as_str() {
        // A catalog name must be a single directory component before resolving cache paths.
        if !name.is_empty() && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c)) {
            let cache_root = root.join(".codex/plugins/cache").join(name).join("anyblob");
            storage::reject_reparse_path(&cache_root)?;
            let latest = if cache_root.is_dir() {
                fs::read_dir(&cache_root).map_err(|_| "Could not inspect the Anyblob cache.")?
                    .map(|entry| entry.map_err(|_| "Could not inspect the Anyblob cache."))
                    .collect::<Result<Vec<_>, _>>()?.into_iter()
                    .filter_map(|entry| entry.file_name().to_str().and_then(|name|
                        semver::Version::parse(name).ok().map(|version| (version, name.to_owned()))))
                    .max_by(|a,b| a.0.cmp(&b.0)).map(|(_,name)| name)
            } else { None };
            // Codex uses both local and versioned cache directories. Inspect only
            // relevant releases, since older historical caches may remain on disk.
            for release in [Some("local"), latest.as_deref()].into_iter().flatten() {
                let cache = cache_root.join(release);
                storage::reject_reparse_path(&cache)?;
                if package_version(&cache)?.is_some() {
                    tree(&cache)?;
                    refresh |= files.iter().any(|(path, data)| fs::read(cache.join(path)).ok().as_deref() != Some(data.as_slice()));
                }
            }
        }
    }
    Ok(json!({"installed":current,"updateAvailable":version.is_some() && (!current || refresh),
        "bundledVersion":bundled,"installedVersion":version,"codexRefreshRequired":refresh}))
}
fn install_at(root: &Path, bytes: &[u8]) -> Result<(), String> {
    let (template, files) = payload(bytes)?;
    let target = root.join(".codex/plugins/anyblob");
    let catalog_path = root.join(".agents/plugins/marketplace.json");
    tree(&target)?;
    if target.exists() && !target.is_dir() {
        return Err("The Anyblob destination is not a directory.".into());
    }
    let value = catalog(&catalog_path, &template)?;
    let before = storage::snapshot(&catalog_path, false)?;
    let original = fs::read(&catalog_path).ok();
    storage::preflight(&catalog_path)?;
    storage::reject_reparse_path(&target)?;
    fs::create_dir_all(target.parent().unwrap())
        .map_err(|_| "Could not prepare the plugin directory.")?;
    let staging = storage::unique_sibling(&target, "stage");
    let backup = storage::unique_sibling(&target, "backup");
    fs::create_dir(&staging).map_err(|_| "Could not stage the Anyblob package.")?;
    let mut backed = false;
    let mut replaced = false;
    let mut catalog_attempted = false;
    let result = (|| {
        for (relative, data) in files {
            let file = staging.join(relative);
            fs::create_dir_all(file.parent().unwrap()).map_err(|_| "Could not stage Anyblob.")?;
            fs::write(file, data).map_err(|_| "Could not stage Anyblob.")?;
        }
        tree(&target)?;
        if target.exists() {
            fs::rename(&target,&backup).map_err(|_|"Could not preserve the previous Anyblob package. Close active Anyblob tools and retry.")?;
            backed = true;
        }
        fs::rename(&staging, &target).map_err(|_| "Could not install the Anyblob package.")?;
        replaced = true;
        if fs::read(&catalog_path).ok() != original {
            return Err("The personal marketplace changed during installation. Retry to keep those changes.".into());
        }
        let catalog_bytes = serde_json::to_vec_pretty(&value).unwrap();
        if original.as_deref() != Some(catalog_bytes.as_slice()) {
            catalog_attempted = true;
            storage::write_atomic(&catalog_path, &catalog_bytes)?;
        }
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        if replaced {
            if fs::remove_dir_all(&target).is_err() {
                failures.push("package removal");
            }
        }
        if backed {
            if fs::rename(&backup, &target).is_err() {
                failures.push("package restore");
            }
        }
        if catalog_attempted && storage::restore(&before).is_err() {
            failures.push("catalog restore");
        }
        let _ = fs::remove_dir_all(&staging);
        if !failures.is_empty() {
            return Err(format!(
                "{error} Rollback failed for {}.",
                failures.join(", ")
            ));
        }
        return Err(error);
    }
    // A loaded executable can retain the backup until Windows releases it. Registration succeeded.
    if backed {
        let _ = fs::remove_dir_all(backup);
    }
    Ok(())
}
#[tauri::command]
pub fn anyblob_installation_status() -> Result<Value, String> {
    let _guard = INSTALL
        .lock()
        .map_err(|_| "Marketplace installation is unavailable.")?;
    installation_status(&profile()?, PAYLOAD)
}
#[tauri::command]
pub async fn install_anyblob_marketplace() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = INSTALL
            .lock()
            .map_err(|_| "Marketplace installation is unavailable.")?;
        install_at(&profile()?, PAYLOAD)
    })
    .await
    .map_err(|_| "Marketplace installation stopped.")?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_status_detects_old_versions_and_stale_codex_copies() {
        let root = root();
        let absent = installation_status(&root, PAYLOAD).unwrap();
        assert_eq!(absent["installed"], false);
        assert_eq!(absent["updateAvailable"], false);
        assert_eq!(absent["installedVersion"], Value::Null);
        install_at(&root, PAYLOAD).unwrap();
        let current = installation_status(&root, PAYLOAD).unwrap();
        assert_eq!(current["updateAvailable"], false);
        assert_eq!(current["installedVersion"], current["bundledVersion"]);
        let manifest_path = root.join(".codex/plugins/anyblob/.codex-plugin/plugin.json");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["version"] = json!("0.0.1");
        fs::write(&manifest_path, manifest.to_string()).unwrap();
        assert_eq!(installation_status(&root, PAYLOAD).unwrap()["updateAvailable"], true);
        install_at(&root, PAYLOAD).unwrap();
        let cache = root.join(".codex/plugins/cache/personal/anyblob/local");
        let (_, files) = payload(PAYLOAD).unwrap();
        for (relative, bytes) in files {
            let path = cache.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        assert_eq!(installation_status(&root, PAYLOAD).unwrap()["updateAvailable"], false);
        fs::write(cache.join("README.md"), b"old release content").unwrap();
        let stale = installation_status(&root, PAYLOAD).unwrap();
        assert_eq!(stale["installed"], true);
        assert_eq!(stale["updateAvailable"], true);
        assert_eq!(stale["codexRefreshRequired"], true);
        install_at(&root, PAYLOAD).unwrap();
        assert_eq!(fs::read(cache.join("README.md")).unwrap(), b"old release content");
        assert_eq!(installation_status(&root, PAYLOAD).unwrap()["codexRefreshRequired"], true);
        let old_cache = cache.with_file_name("0.0.1");
        fs::rename(&cache, &old_cache).unwrap();
        assert_eq!(installation_status(&root, PAYLOAD).unwrap()["codexRefreshRequired"], true);
        let latest_cache = cache.with_file_name(current["bundledVersion"].as_str().unwrap());
        let (_, files) = payload(PAYLOAD).unwrap();
        for (relative, bytes) in files {
            let path = latest_cache.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        assert_eq!(installation_status(&root, PAYLOAD).unwrap()["codexRefreshRequired"], false);
        fs::write(latest_cache.join("README.md"), b"stale versioned copy").unwrap();
        assert_eq!(installation_status(&root, PAYLOAD).unwrap()["codexRefreshRequired"], true);
        fs::remove_dir_all(root).unwrap();
    }
    fn root() -> PathBuf {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.test-run")
            .join(format!(
                "marketplace-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn install_repeat_update_and_preserve() {
        let root = root();
        assert!(!installed(&root, PAYLOAD).unwrap());
        install_at(&root, PAYLOAD).unwrap();
        assert!(installed(&root, PAYLOAD).unwrap());
        let path = root.join(".agents/plugins/marketplace.json");
        let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["extra"] = json!({"keep":42});
        v["plugins"][0]["custom"] = json!("retain");
        v["plugins"][0]["policy"]["custom"] = json!(true);
        v["plugins"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"other","custom":true}));
        fs::write(&path, v.to_string()).unwrap();
        fs::write(root.join(".codex/plugins/anyblob/README.md"), "outdated").unwrap();
        assert!(!installed(&root, PAYLOAD).unwrap());
        install_at(&root, PAYLOAD).unwrap();
        install_at(&root, PAYLOAD).unwrap();
        let after: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(after, v);
        assert!(installed(&root, PAYLOAD).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn skills_are_installed_and_missing_files_require_update() {
        let root = root();
        install_at(&root, PAYLOAD).unwrap();
        let package = root.join(".codex/plugins/anyblob");
        for relative in FILES.iter().filter(|p| p.starts_with("skills/") || p.starts_with("references/")) {
            assert!(!fs::read(package.join(relative)).unwrap().is_empty());
        }
        let skill = package.join("skills/maintain-pages/SKILL.md");
        fs::remove_file(&skill).unwrap();
        assert!(!installed(&root, PAYLOAD).unwrap());
        install_at(&root, PAYLOAD).unwrap();
        assert!(installed(&root, PAYLOAD).unwrap());
        let mut incomplete: Value = serde_json::from_slice(PAYLOAD).unwrap();
        incomplete["files"].as_object_mut().unwrap().remove("skills/maintain-pages/SKILL.md");
        assert!(payload(&serde_json::to_vec(&incomplete).unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_missing_payload_and_rollback() {
        let root = root();
        assert!(install_at(&root, b"").is_err());
        install_at(&root, PAYLOAD).unwrap();
        let path = root.join(".agents/plugins/marketplace.json");
        for malformed in [
            "null",
            "[]",
            "{}",
            r#"{"name":"personal","plugins":{}}"#,
            r#"{"name":"personal","plugins":[null]}"#,
        ] {
            fs::write(&path, malformed).unwrap();
            assert!(install_at(&root, PAYLOAD).is_err());
            assert_eq!(fs::read(&path).unwrap(), malformed.as_bytes());
        }
        fs::write(&path, b"broken").unwrap();
        assert!(install_at(&root, PAYLOAD).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
        fs::write(&path, b"{\"name\":\"personal\",\"plugins\":[]}").unwrap();
        let before = fs::read(&path).unwrap();
        let readme = root.join(".codex/plugins/anyblob/README.md");
        fs::write(&readme, b"old").unwrap();
        storage::FAIL_AFTER_REPLACE.with(|f| *f.borrow_mut() = Some(path.clone()));
        assert!(install_at(&root, PAYLOAD).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
        assert_eq!(fs::read(readme).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn rejects_linked_destinations() {
        use std::os::windows::fs::symlink_dir;
        let root = root();
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::create_dir(root.join(".codex")).unwrap();
        let link = root.join(".codex/plugins");
        if symlink_dir(&outside, &link).is_err() {
            assert!(std::process::Command::new("cmd.exe")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(&link)
                .arg(&outside)
                .output()
                .unwrap()
                .status
                .success());
        }
        assert!(install_at(&root, PAYLOAD).is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_dir(&link).unwrap();
        fs::create_dir_all(root.join(".agents")).unwrap();
        assert!(std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(root.join(".agents/plugins"))
            .arg(&outside)
            .output()
            .unwrap()
            .status
            .success());
        assert!(install_at(&root, PAYLOAD).is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_dir(root.join(".agents/plugins")).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
