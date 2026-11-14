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
        projects.extend(cloud.into_iter().map(|project| Project {
            id: project.id,
            name: project.name,
            root_paths: Vec::new(),
            kind: "cloud".into(),
            host_id: Some(project.host_id),
            computer_name: Some(project.computer_name),
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
    let mut projects = Vec::new();
    for (key, value) in entries {
        let mut project: Project = serde_json::from_value(value.clone())
            .map_err(|_| "Unsupported Codex project metadata. Each project needs an ID, name and rootPaths list.")?;
        if project.id.is_empty() || project.id != *key || project.name.trim().is_empty()
            || project.root_paths.is_empty() || project.root_paths.iter().any(|path| path.trim().is_empty()) {
            return Err("Unsupported Codex project metadata. Project IDs, names and folders must be present and consistent.".into());
        }
        project.kind = local_kind();
        project.host_id = None;
        project.computer_name = None;
        projects.push(project);
    }
    projects.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    Ok(projects)
}

#[tauri::command]
pub async fn discover_codex_projects() -> Result<Vec<Project>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let local = (|| {
            let profile = std::env::var_os("USERPROFILE")
                .ok_or("Windows user profile is unavailable.")?;
            let path = std::path::PathBuf::from(profile).join(".codex/.codex-global-state.json");
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
