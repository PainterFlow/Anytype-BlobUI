//! Discovers Anyblob projects under the configured local root.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
};

const PROJECT_FILE: &str = "anyblob.config.toml";
const MAX_PROJECT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_paths: Vec<String>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computer_name: Option<String>,
}

pub(crate) struct ProjectConfig {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) remote: bool,
}

#[derive(Deserialize)]
struct ProjectFile {
    name: String,
    key: String,
    remote: bool,
}

pub(crate) fn parse_config(text: &str) -> Result<ProjectConfig, String> {
    if text.len() > MAX_PROJECT_BYTES {
        return Err("Anyblob project config exceeds 16 KiB.".into());
    }
    let file: ProjectFile = toml::from_str(text)
        .map_err(|_| "Anyblob project config must specify a name, key, and remote flag.")?;
    identify_fields(&file.name, &file.key, file.remote)
}

pub(crate) fn identify_fields(name: &str, key: &str, remote: bool) -> Result<ProjectConfig, String> {
    let name = name.trim();
    if name.is_empty()
        || name.len() > 512
        || name.chars().any(char::is_control)
        || key.trim().is_empty()
        || key.len() > MAX_PROJECT_BYTES
        || key.chars().any(char::is_control)
    {
        return Err("Anyblob project identity contains an invalid name or key.".into());
    }
    Ok(ProjectConfig {
        id: format!("{:x}", Sha256::digest(key.as_bytes())),
        name: name.into(),
        remote,
    })
}

fn scan_root(root: &Path) -> Result<Vec<Project>, String> {
    if !root.is_absolute() {
        return Err("An Anyblob projects folder must be an absolute path.".into());
    }
    crate::reject_reparse_component(root)?;
    let mut projects = Vec::new();
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Could not read Anyblob projects folder {}: {error}", root.display()))?;
    for entry in entries {
        let directory = entry.map_err(|error| error.to_string())?.path();
        let metadata = fs::symlink_metadata(&directory).map_err(|error| error.to_string())?;
        if !metadata.is_dir() || crate::is_windows_reparse_point(&metadata) {
            continue;
        }
        let path = directory.join(PROJECT_FILE);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("Could not inspect {}: {error}", path.display())),
        };
        if !metadata.is_file() || crate::is_windows_reparse_point(&metadata) {
            return Err(format!("{} must be a regular file.", path.display()));
        }
        let mut text = String::new();
        fs::File::open(&path)
            .and_then(|file| file.take(MAX_PROJECT_BYTES as u64 + 1).read_to_string(&mut text))
            .map_err(|_| format!("Could not read {} as UTF-8.", path.display()))?;
        let config = parse_config(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        projects.push(Project {
            id: config.id,
            name: config.name,
            root_paths: vec![directory.to_string_lossy().into_owned()],
            // Project files are discovered only under the explicitly selected local folder.
            // The legacy remote flag remains in the authentication file contract, but it no
            // longer routes discovery through a remote Codex folder.
            kind: "local".into(),
            host_id: None,
            computer_name: None,
        });
    }
    Ok(projects)
}

fn discover_in(local: Option<&Path>) -> Result<Vec<Project>, String> {
    let mut projects = Vec::new();
    let mut ids = BTreeSet::new();
    if let Some(root) = local {
        for project in scan_root(root)? {
            if !ids.insert(project.id.clone()) {
                return Err("Anyblob projects must have unique keys.".into());
            }
            projects.push(project);
        }
    }
    projects.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(left.kind.cmp(&right.kind))
            .then(left.id.cmp(&right.id))
    });
    Ok(projects)
}

pub(crate) fn discover() -> Result<Vec<Project>, String> {
    let install = crate::resolve_install_directory(&crate::app_data_directory())?;
    let _ = crate::settings_data_path(&install)?;
    let settings_path =
        crate::settings_root_for_install(&install)?.join("Settings/anyblob/settings.json");
    crate::reject_reparse_component(&settings_path)?;
    let local = if settings_path.exists() {
        crate::read_json_file(&settings_path)?["localProjectsFolder"]
            .as_str()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    } else {
        None
    };
    discover_in(local.as_deref())
}

#[tauri::command]
pub async fn discover_codex_projects() -> Result<Vec<Project>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let projects = discover()?;
        crate::anyblob::synchronize_project_registry(&projects)?;
        Ok(projects)
    })
        .await
        .map_err(|_| "Project discovery stopped. Reload to retry.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../.test-run/anyblob-project-discovery")
                .join(format!("{}-{name}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Clean up only the files created by the fixture.
            let mut directories = Vec::new();
            let mut pending = vec![self.0.clone()];
            while let Some(directory) = pending.pop() {
                if let Ok(entries) = fs::read_dir(&directory) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                            pending.push(path);
                        } else {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
                directories.push(directory);
            }
            for directory in directories.into_iter().rev() {
                let _ = fs::remove_dir(directory);
            }
        }
    }

    #[test]
    fn discovers_projects_only_in_the_configured_local_folder() {
        let fixture = Fixture::new("roots");
        fixture.write(
            "local/first/anyblob.config.toml",
            "name = 'Local'\nkey = 'local secret'\nremote = false",
        );
        fixture.write(
            "unconfigured/second/anyblob.config.toml",
            "name = 'Unconfigured'\nkey = 'unconfigured secret'\nremote = true",
        );
        fixture.write(
            "unconfigured/other/nested/anyblob.config.toml",
            "name = 'Nested'\nkey = 'nested secret'\nremote = true",
        );
        let local = fixture.0.join("local");
        let projects = discover_in(Some(&local)).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, "local");
        assert_eq!(Path::new(&projects[0].root_paths[0]), local.join("first"));
        let exported = serde_json::to_string(&projects).unwrap();
        assert!(!exported.contains("secret"));
    }

    #[test]
    fn rejects_invalid_configs_without_echoing_credentials() {
        for invalid in [
            "name = 'Missing key'\nremote = true",
            "name = 'Invalid flag'\nkey = 'secret'\nremote = 'yes'",
            "name = ''\nkey = 'secret'\nremote = true",
        ] {
            assert!(!parse_config(invalid).err().unwrap().contains("secret"));
        }
    }

    #[test]
    fn project_identity_survives_a_rename_and_rejects_shared_keys() {
        let first = "name = 'First'\nkey = 'same key'\nremote = false";
        let second = "name = 'Second'\nkey = 'same key'\nremote = true";
        let id = parse_config(first).unwrap().id;
        assert_eq!(id.len(), 64);
        assert_eq!(parse_config(second).unwrap().id, id);
        let fixture = Fixture::new("duplicate");
        fixture.write("first/anyblob.config.toml", first);
        fixture.write("second/anyblob.config.toml", second);
        assert!(discover_in(Some(&fixture.0)).is_err());
    }

    #[test]
    fn separate_authentication_fields_match_the_project_file_identity() {
        let from_file = parse_config("name = ' Work '\nkey = 'same key'\nremote = true").unwrap();
        let from_fields = identify_fields(" Work ", "same key", true).unwrap();
        assert_eq!(from_fields.id, from_file.id);
        assert_eq!(from_fields.name, from_file.name);
        assert_eq!(from_fields.remote, from_file.remote);
        assert_eq!(from_fields.name, "Work");
        assert!(identify_fields("Work", "secret\nkey", true).is_err());
    }
}
