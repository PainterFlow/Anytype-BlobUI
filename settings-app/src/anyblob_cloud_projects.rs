//! Persisted project catalogs received from authenticated remote Anyblob hosts.
//! Project roots never cross the tunnel.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEntry {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostCatalog {
    pub computer_name: String,
    pub projects: Vec<ProjectEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub format_version: u32,
    pub hosts: BTreeMap<String, HostCatalog>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub host_id: String,
    pub computer_name: String,
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
        })
        && !matches!(value, "." | "..")
}

fn validate_host(host_id: &str, host: &HostCatalog) -> Result<(), String> {
    if !valid_identifier(host_id, 128) {
        return Err("Cloud project sync has an invalid host ID.".into());
    }
    if host.computer_name.trim().is_empty()
        || host.computer_name.len() > 256
        || host.computer_name.chars().any(char::is_control)
    {
        return Err("Cloud project sync has an invalid computer name.".into());
    }
    if host.projects.len() > 10_000 {
        return Err("Cloud project sync contains too many projects.".into());
    }
    let mut ids = BTreeSet::new();
    for project in &host.projects {
        if !valid_identifier(&project.id, 512)
            || project.name.trim().is_empty()
            || project.name.len() > 1024
            || project.name.chars().any(char::is_control)
            || !ids.insert(project.id.as_str())
        {
            return Err("Cloud project sync contains an invalid or duplicate project.".into());
        }
    }
    Ok(())
}

fn validate_catalog(catalog: &Catalog) -> Result<(), String> {
    if catalog.format_version != FORMAT_VERSION {
        return Err("The saved cloud project catalog has an unsupported format version.".into());
    }
    for (host_id, host) in &catalog.hosts {
        validate_host(host_id, host)?;
    }
    Ok(())
}

pub fn project_key(host_id: &str, project_id: &str) -> String {
    format!("cloud:{host_id}:{project_id}")
}

pub fn read_at(path: &std::path::Path) -> Result<Catalog, String> {
    crate::reject_reparse_component(path)?;
    if !path.exists() {
        return Ok(Catalog {
            format_version: FORMAT_VERSION,
            hosts: BTreeMap::new(),
        });
    }
    let bytes = std::fs::read(path).map_err(|_| "Could not read the saved cloud project catalog.")?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("The saved cloud project catalog is too large.".into());
    }
    let catalog: Catalog = serde_json::from_slice(&bytes)
        .map_err(|_| "The saved cloud project catalog is invalid JSON.")?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

pub fn update_at(
    path: &std::path::Path,
    host_id: &str,
    computer_name: &str,
    projects: Vec<ProjectEntry>,
) -> Result<(), String> {
    let host = HostCatalog {
        computer_name: computer_name.into(),
        projects,
    };
    validate_host(host_id, &host)?;
    let mut catalog = read_at(path)?;
    catalog.hosts.insert(host_id.into(), host);
    let mut bytes = serde_json::to_vec_pretty(&catalog)
        .map_err(|_| "Could not encode the cloud project catalog.")?;
    bytes.push(b'\n');
    crate::storage::write_atomic(path, &bytes)?;
    Ok(())
}

pub fn projects(catalog: &Catalog) -> Vec<ProjectSummary> {
    let mut projects = catalog
        .hosts
        .iter()
        .flat_map(|(host_id, host)| {
            host.projects.iter().map(|project| ProjectSummary {
                id: project.id.clone(),
                name: project.name.clone(),
                host_id: host_id.clone(),
                computer_name: host.computer_name.clone(),
            })
        })
        .collect::<Vec<_>>();
    projects.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(left.computer_name.cmp(&right.computer_name))
            .then(left.host_id.cmp(&right.host_id))
            .then(left.id.cmp(&right.id))
    });
    projects
}

/// Verify a remote identity against the latest saved catalog before it can select a scope.
/// `Ok(None)` is an explicitly verified projectless chat.
pub fn resolve_at(
    path: &std::path::Path,
    host_id: &str,
    project_id: Option<&str>,
) -> Result<Option<String>, String> {
    if !valid_identifier(host_id, 128) {
        return Err("Remote Codex supplied an invalid host identity.".into());
    }
    let catalog = read_at(path)?;
    let host = catalog
        .hosts
        .get(host_id)
        .ok_or("Remote Codex host is not in the synced project catalog.")?;
    let Some(project_id) = project_id else {
        return Ok(None);
    };
    if !valid_identifier(project_id, 512)
        || !host.projects.iter().any(|project| project.id == project_id)
    {
        return Err("Remote Codex project is not in the synced project catalog.".into());
    }
    Ok(Some(project_key(host_id, project_id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.test-run/cloud-projects")
                .join(format!("{}-{nonce}", std::process::id()));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn sync_is_host_scoped_keeps_roots_private_and_updates_names() {
        let fixture = Fixture::new();
        update_at(
            &fixture.0,
            "host_a",
            "CLOUD-ONE",
            vec![ProjectEntry { id: "same-id".into(), name: "Work".into() }],
        ).unwrap();
        update_at(
            &fixture.0,
            "host_b",
            "CLOUD-TWO",
            vec![ProjectEntry { id: "same-id".into(), name: "Work".into() }],
        ).unwrap();
        assert_eq!(resolve_at(&fixture.0, "host_a", Some("same-id")).unwrap(), Some("cloud:host_a:same-id".into()));
        assert_eq!(resolve_at(&fixture.0, "host_b", Some("same-id")).unwrap(), Some("cloud:host_b:same-id".into()));
        update_at(
            &fixture.0,
            "host_a",
            "RENAMED-PC",
            vec![ProjectEntry { id: "same-id".into(), name: "Renamed work".into() }],
        ).unwrap();
        let catalog = read_at(&fixture.0).unwrap();
        assert_eq!(catalog.hosts["host_a"].computer_name, "RENAMED-PC");
        assert_eq!(projects(&catalog)[0].computer_name, "RENAMED-PC");
        let saved = std::fs::read_to_string(&fixture.0).unwrap();
        assert!(!saved.contains("rootPaths") && !saved.contains("CLOUD-ONE"));
    }

    #[test]
    fn empty_catalog_replaces_host_catalog_and_invalid_updates_do_not_mutate_it() {
        let fixture = Fixture::new();
        update_at(
            &fixture.0,
            "host",
            "CLOUD",
            vec![ProjectEntry { id: "p".into(), name: "Project".into() }],
        ).unwrap();
        update_at(&fixture.0, "host", "CLOUD", vec![]).unwrap();
        assert!(read_at(&fixture.0).unwrap().hosts["host"].projects.is_empty());
        assert!(update_at(
            &fixture.0,
            "host",
            "CLOUD",
            vec![ProjectEntry { id: "bad/id".into(), name: "Invalid".into() }],
        ).is_err());
        assert!(read_at(&fixture.0).unwrap().hosts["host"].projects.is_empty());
        assert!(resolve_at(&fixture.0, "host", None).unwrap().is_none());
        assert!(resolve_at(&fixture.0, "host", Some("gone")).is_err());
        assert!(resolve_at(&fixture.0, "missing-host", None).is_err());
    }

    #[test]
    fn invalid_catalog_file_does_not_become_an_empty_catalog() {
        let fixture = Fixture::new();
        std::fs::write(&fixture.0, json!({"formatVersion":99,"hosts":{}}).to_string()).unwrap();
        assert!(read_at(&fixture.0).is_err());
    }
}
