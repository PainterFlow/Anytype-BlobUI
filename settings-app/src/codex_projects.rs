//! Read-only compatibility reader. Project metadata never establishes request identity.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_paths: Vec<String>,
    #[serde(default = "local_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer_name: Option<String>,
}

fn local_kind() -> String { "local".into() }

fn valid_project_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !matches!(value, "." | "..")
}

pub(crate) fn codex_home_path() -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os("CODEX_HOME") {
        if !path.is_empty() {
            return Ok(path.into());
        }
    }
    let profile = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("Codex home directory is unavailable.")?;
    Ok(std::path::PathBuf::from(profile).join(".codex"))
}

fn combine_project_catalogs(
    local: Result<Vec<Project>, String>,
    cloud: Result<Vec<crate::anyblob_cloud_projects::ProjectSummary>, String>,
) -> Result<Vec<Project>, String> {
    let (mut projects, cloud) = match (local, cloud) {
        (Ok(local), Ok(cloud)) => (local, Some(cloud)),
        (Err(local_error), Ok(cloud)) if !cloud.is_empty() => {
            eprintln!("Could not read local Codex projects. Showing synced cloud projects: {local_error}");
            (Vec::new(), Some(cloud))
        }
        (Ok(local), Err(cloud_error)) if !local.is_empty() => {
            eprintln!("Could not read synced cloud projects. Showing local Codex projects: {cloud_error}");
            (local, None)
        }
        (Err(local_error), Ok(_)) => return Err(local_error),
        (Ok(_), Err(cloud_error)) => return Err(cloud_error),
        (Err(local_error), Err(cloud_error)) => {
            eprintln!("Could not read synced cloud projects: {cloud_error}");
            return Err(local_error);
        }
    };

    if let Some(cloud) = cloud {
        let codex_remote_projects = projects
            .iter()
            .filter(|project| project.kind == "remote")
            .filter_map(|project| {
                project
                    .host_id
                    .clone()
                    .map(|host_id| (host_id, project.id.clone()))
            })
            .collect::<std::collections::BTreeSet<_>>();
        projects.extend(cloud.into_iter().filter_map(|project| {
            let already_in_codex_state = codex_remote_projects
                .contains(&(project.host_id.clone(), project.id.clone()));
            (!already_in_codex_state).then_some(Project {
                id: project.id,
                name: project.name,
                root_paths: Vec::new(),
                kind: "cloud".into(),
                host_id: Some(project.host_id),
                computer_name: Some(project.computer_name),
            })
        }));
    }
    projects.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.kind.cmp(&b.kind))
            .then(a.host_id.cmp(&b.host_id))
            .then(a.id.cmp(&b.id))
    });
    Ok(projects)
}

pub fn parse(text: &str) -> Result<Vec<Project>, String> {
    let state: Value = serde_json::from_str(text).map_err(|_| "Codex project metadata is not valid JSON.")?;
    let entries = state.get("local-projects").and_then(Value::as_object)
        .ok_or("Unsupported Codex project metadata. Expected a local-projects object.")?;
    let mut projects = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let mut project: Project = serde_json::from_value(value.clone())
            .map_err(|_| "Unsupported Codex project metadata. Each project needs an ID, name and rootPaths list.")?;
        if !valid_project_id(&project.id) || project.id != *key || project.name.trim().is_empty()
            || project.root_paths.is_empty() || project.root_paths.iter().any(|path| path.trim().is_empty()) {
            return Err("Unsupported Codex project metadata. Project IDs, names and folders must be present and consistent.".into());
        }
        project.kind = local_kind();
        project.host_id = None;
        project.computer_name = None;
        projects.push(project);
    }
    if let Some(remote_entries) = state.get("remote-projects") {
        let remote_entries = remote_entries
            .as_array()
            .ok_or("Unsupported Codex project metadata. Expected a remote-projects list.")?;
        let mut seen = std::collections::BTreeSet::new();
        for entry in remote_entries {
            let host_id = entry["hostId"]
                .as_str()
                .filter(|host_id| valid_project_id(host_id))
                .ok_or("Unsupported Codex project metadata. A remote project needs a valid host ID.")?;
            let id = entry["id"]
                .as_str()
                .filter(|id| valid_project_id(id))
                .ok_or("Unsupported Codex project metadata. A remote project needs a valid ID.")?;
            let name = entry["label"]
                .as_str()
                .map(str::trim)
                .filter(|name| !name.is_empty() && name.len() <= 512 && !name.chars().any(char::is_control))
                .ok_or("Unsupported Codex project metadata. A remote project needs a valid label.")?;
            if !seen.insert((host_id.to_owned(), id.to_owned())) {
                return Err("Unsupported Codex project metadata. Remote project identities must be unique per host.".into());
            }
            projects.push(Project {
                id: id.into(),
                name: name.into(),
                root_paths: Vec::new(),
                kind: "remote".into(),
                host_id: Some(host_id.into()),
                computer_name: None,
            });
        }
    }
    projects.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.kind.cmp(&b.kind)).then(a.host_id.cmp(&b.host_id)).then(a.id.cmp(&b.id)));
    Ok(projects)
}

#[tauri::command]
pub async fn discover_codex_projects() -> Result<Vec<Project>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let local = (|| {
            let path = codex_home_path()?.join(".codex-global-state.json");
            let text = std::fs::read_to_string(path).map_err(|_| {
                "Could not read Codex project metadata. The previous project list has been kept."
            })?;
            parse(&text)
        })();
        combine_project_catalogs(local, crate::anyblob::synced_cloud_projects())
    }).await.map_err(|_| "Project discovery stopped. Reload to retry.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn metadata_ignores_unrelated_state_and_preserves_stable_ids() {
        let mut value = json!({"local-projects":{
            "a":{"id":"a","name":"Same","rootPaths":["C:\\one","C:\\two"],"unknown":42},
            "b":{"id":"b","name":"Same","rootPaths":["C:\\worktree"]}},"selected-project":"ignore"});
        let first = parse(&value.to_string()).unwrap();
        assert_eq!(first.len(),2); assert_eq!(first[0].root_paths.len(),2);
        value["local-projects"]["a"]["name"] = "Renamed".into();
        assert_eq!(parse(&value.to_string()).unwrap()[0].id,first[0].id);
        value["local-projects"].as_object_mut().unwrap().remove("a");
        assert_eq!(parse(&value.to_string()).unwrap().len(),1);
        assert!(parse(r#"{"local-projects":{}}"#).unwrap().is_empty());
    }
    #[test]
    fn rejects_unsupported_and_malformed_metadata() {
        for text in ["{", "null", "{}", r#"{"local-projects":[]}"#,
            r#"{"local-projects":{"a":{"id":"b","name":"X","rootPaths":["C:\\x"]}}}"#,
            r#"{"local-projects":{"a":{"id":"a","name":"X","rootPaths":[]}}}"#,
            r#"{"local-projects":{"a":{"id":"a","name":"X","rootPaths":[2]}}}"#] {
            assert!(parse(text).is_err(),"{text}");
        }
    }

    #[test]
    fn discovers_remote_projects_with_host_scoped_identity() {
        let projects = parse(r#"{"local-projects":{},"remote-projects":[
            {"hostId":"remote-host","id":"same-id","label":"Remote work","remotePath":"/private/path"},
            {"hostId":"other-host","id":"same-id","label":"Other work","remotePath":"/other/path"}
        ]}"#).unwrap();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].kind, "remote");
        assert_eq!(projects[0].id, "same-id");
        assert_eq!(projects[0].host_id.as_deref(), Some("other-host"));
        assert_eq!(projects[0].name, "Other work");
        assert!(projects.iter().all(|project| project.root_paths.is_empty()));
        assert!(!serde_json::to_string(&projects).unwrap().contains("private/path"));

        assert!(parse(r#"{"local-projects":{},"remote-projects":[
            {"hostId":"remote-host","id":"same-id","label":"Remote work"},
            {"hostId":"remote-host","id":"same-id","label":"Duplicate"}
        ]}"#).is_err());
    }

    #[test]
    fn codex_remote_projects_replace_matching_synced_cloud_rows() {
        let local = parse(r#"{"local-projects":{},"remote-projects":[
            {"hostId":"shared-host","id":"project-a","label":"Codex project"}
        ]}"#).unwrap();
        let projects = combine_project_catalogs(
            Ok(local),
            Ok(vec![crate::anyblob_cloud_projects::ProjectSummary {
                id: "project-a".into(),
                name: "Synced project".into(),
                host_id: "shared-host".into(),
                computer_name: "Remote PC".into(),
            }]),
        ).unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, "remote");
        assert_eq!(projects[0].name, "Codex project");
    }

    #[test]
    fn synced_cloud_projects_remain_available_when_local_metadata_is_missing() {
        let projects = combine_project_catalogs(
            Err("Local Codex metadata is unavailable.".into()),
            Ok(vec![crate::anyblob_cloud_projects::ProjectSummary {
                id: "cloud-project".into(),
                name: "Remote work".into(),
                host_id: "remote-host".into(),
                computer_name: "Build server".into(),
            }]),
        )
        .unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, "cloud");
        assert_eq!(projects[0].id, "cloud-project");
        assert_eq!(projects[0].host_id.as_deref(), Some("remote-host"));
        assert!(projects[0].root_paths.is_empty());
    }

    #[test]
    fn local_projects_remain_available_when_cloud_catalog_cannot_be_read() {
        let local = Project {
            id: "local-project".into(),
            name: "Local work".into(),
            root_paths: vec!["C:\\work".into()],
            kind: "local".into(),
            host_id: None,
            computer_name: None,
        };
        let projects = combine_project_catalogs(
            Ok(vec![local.clone()]),
            Err("Cloud project catalog is unavailable.".into()),
        )
        .unwrap();

        assert_eq!(projects, vec![local]);
    }
}
