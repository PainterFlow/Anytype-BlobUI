//! Reads local Codex projects without exporting remote folder paths.
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

fn local_kind() -> String {
    "local".into()
}

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

pub fn parse(text: &str) -> Result<Vec<Project>, String> {
    let state: Value = serde_json::from_str(text)
        .map_err(|_| "Codex project metadata is not valid JSON.")?;
    let entries = state
        .get("local-projects")
        .and_then(Value::as_object)
        .ok_or("Unsupported Codex project metadata. Expected a local-projects object.")?;
    let mut projects = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let mut project: Project = serde_json::from_value(value.clone())
            .map_err(|_| "Unsupported Codex project metadata. Each project needs an ID, name and rootPaths list.")?;
        if !valid_project_id(&project.id)
            || project.id != *key
            || project.name.trim().is_empty()
            || project.root_paths.is_empty()
            || project.root_paths.iter().any(|path| path.trim().is_empty())
        {
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
                .filter(|host_id| crate::codex_identity::valid_host_id(host_id))
                .ok_or("Unsupported Codex project metadata. A remote project needs a valid host ID.")?;
            let id = entry["id"]
                .as_str()
                .filter(|id| valid_project_id(id))
                .ok_or("Unsupported Codex project metadata. A remote project needs a valid ID.")?;
            let name = entry["label"]
                .as_str()
                .map(str::trim)
                .filter(|name| {
                    !name.is_empty() && name.len() <= 512 && !name.chars().any(char::is_control)
                })
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

    projects.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(left.kind.cmp(&right.kind))
            .then(left.host_id.cmp(&right.host_id))
            .then(left.id.cmp(&right.id))
    });
    Ok(projects)
}

#[tauri::command]
pub async fn discover_codex_projects() -> Result<Vec<Project>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = codex_home_path()?.join(".codex-global-state.json");
        let text = std::fs::read_to_string(path)
            .map_err(|_| "Could not read Codex project metadata. The previous project list has been kept.")?;
        parse(&text)
    })
    .await
    .map_err(|_| "Project discovery stopped. Reload to retry.".to_string())?
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
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].root_paths.len(), 2);
        value["local-projects"]["a"]["name"] = "Renamed".into();
        assert_eq!(parse(&value.to_string()).unwrap()[0].id, first[0].id);
        value["local-projects"].as_object_mut().unwrap().remove("a");
        assert_eq!(parse(&value.to_string()).unwrap().len(), 1);
        assert!(parse(r#"{"local-projects":{}}"#).unwrap().is_empty());
    }

    #[test]
    fn rejects_unsupported_and_malformed_metadata() {
        for text in [
            "{",
            "null",
            "{}",
            r#"{"local-projects":[]}"#,
            r#"{"local-projects":{"a":{"id":"b","name":"X","rootPaths":["C:\\x"]}}}"#,
            r#"{"local-projects":{"a":{"id":"a","name":"X","rootPaths":[]}}}"#,
            r#"{"local-projects":{"a":{"id":"a","name":"X","rootPaths":[2]}}}"#,
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn discovers_remote_projects_with_colon_bearing_host_ids() {
        let projects = parse(r#"{"local-projects":{},"remote-projects":[
            {"hostId":"remote-host:instance_01","id":"same-id","label":"Remote work","remotePath":"/private/path"},
            {"hostId":"other-host:instance_02","id":"same-id","label":"Other work","remotePath":"/other/path"}
        ]}"#).unwrap();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].kind, "remote");
        assert_eq!(projects[0].id, "same-id");
        assert_eq!(projects[0].host_id.as_deref(), Some("other-host:instance_02"));
        assert_eq!(projects[0].name, "Other work");
        assert!(projects.iter().all(|project| project.root_paths.is_empty()));
        assert!(!serde_json::to_string(&projects).unwrap().contains("private/path"));

        assert!(parse(r#"{"local-projects":{},"remote-projects":[
            {"hostId":"remote-host:instance_01","id":"same-id","label":"Remote work"},
            {"hostId":"remote-host:instance_01","id":"same-id","label":"Duplicate"}
        ]}"#).is_err());
        assert!(parse(r#"{"local-projects":{},"remote-projects":[
            {"hostId":"invalid/host","id":"same-id","label":"Invalid host"}
        ]}"#).is_err());
    }
}
