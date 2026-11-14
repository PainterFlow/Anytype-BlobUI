//! Read the Codex project catalog without exporting folder paths.
use crate::ProjectInfo;
use serde_json::Value;
use std::{path::PathBuf, time::Duration};

const CATALOG_FILE: &str = ".codex-global-state.json";

fn valid_project_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !matches!(value, "." | "..")
}

pub fn codex_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CODEX_HOME") {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    let profile = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("Codex home directory is unavailable.")?;
    Ok(PathBuf::from(profile).join(".codex"))
}

pub fn parse(text: &str) -> Result<Vec<ProjectInfo>, String> {
    let state: Value =
        serde_json::from_str(text).map_err(|_| "Codex project catalog is not valid JSON.")?;
    let entries = state
        .get("local-projects")
        .and_then(Value::as_object)
        .ok_or("Codex project catalog has no local-projects object.")?;
    let mut projects = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let id = value["id"]
            .as_str()
            .filter(|id| valid_project_id(id))
            .ok_or("Codex project catalog contains an invalid project ID.")?;
        let name = value["name"]
            .as_str()
            .map(str::trim)
            .filter(|name| {
                !name.is_empty() && name.len() <= 512 && !name.chars().any(char::is_control)
            })
            .ok_or("Codex project catalog contains an invalid project name.")?;
        if id != key {
            return Err("Codex project catalog contains inconsistent project IDs.".into());
        }
        projects.push(ProjectInfo {
            id: id.into(),
            name: name.into(),
        });
    }
    projects.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(projects)
}

pub fn read_at(home: &std::path::Path) -> Result<Vec<ProjectInfo>, String> {
    let text = std::fs::read_to_string(home.join(CATALOG_FILE))
        .map_err(|_| "Could not read the Codex project catalog.")?;
    parse(&text)
}

pub fn read() -> Result<Vec<ProjectInfo>, String> {
    read_at(&codex_home()?)
}

pub fn computer_name() -> String {
    let value = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Codex host".into());
    let clean = value
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(255)
        .collect::<String>();
    if clean.is_empty() {
        "Codex host".into()
    } else {
        clean
    }
}

pub fn sync_payload_at(
    home: &std::path::Path,
    projects: Vec<ProjectInfo>,
    token: String,
) -> Result<crate::ProjectSync, String> {
    Ok(crate::ProjectSync {
        token,
        host_id: crate::codex_identity::host_id_at(home)?,
        computer_name: computer_name(),
        projects,
    })
}

pub fn start_remote_sync_monitor() {
    if std::env::var_os("ANYBLOB_REMOTE_ADDRESS").is_none() {
        return;
    }
    std::thread::spawn(|| {
        let mut last_synced: Option<(String, Vec<ProjectInfo>)> = None;
        let mut last_attempt: Option<std::time::Instant> = None;
        loop {
            let computer = computer_name();
            match read() {
                Ok(projects) => {
                    let current = (computer.clone(), projects.clone());
                    if last_synced.as_ref() != Some(&current)
                        && last_attempt
                            .is_none_or(|attempt| attempt.elapsed() >= Duration::from_secs(5))
                    {
                        last_attempt = Some(std::time::Instant::now());
                        let result = codex_home()
                            .and_then(|home| crate::credential::read().map(|token| (home, token)))
                            .and_then(|(home, token)| sync_payload_at(&home, projects, token))
                            .and_then(crate::transport::sync_project_catalog);
                        match result {
                            Ok(()) => last_synced = Some(current),
                            Err(error) => eprintln!("Anyblob remote project sync failed: {error}"),
                        }
                    }
                }
                Err(error) => {
                    // Invalid or missing catalog data must never replace a previously synced catalog.
                    if last_synced.is_none()
                        && last_attempt
                            .is_none_or(|attempt| attempt.elapsed() >= Duration::from_secs(30))
                    {
                        eprintln!("Anyblob remote project sync is waiting for a valid Codex catalog: {error}");
                        last_attempt = Some(std::time::Instant::now());
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_sync_uses_stable_ids_and_names_only() {
        let first = parse(
            r#"{"local-projects":{
            "project-a":{"id":"project-a","name":"Same name","rootPaths":["C:\\private\\one"]},
            "project-b":{"id":"project-b","name":"Same name","rootPaths":["C:\\private\\two"]}
        }}"#,
        )
        .unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(
            first[0],
            ProjectInfo {
                id: "project-a".into(),
                name: "Same name".into()
            }
        );
        assert_eq!(first[1].id, "project-b");
        assert!(!serde_json::to_string(&first).unwrap().contains("private"));

        let renamed = parse(r#"{"local-projects":{"project-a":{"id":"project-a","name":"Renamed","rootPaths":["C:\\new"]}}}"#).unwrap();
        assert_eq!(renamed[0].id, first[0].id);
        assert_eq!(renamed[0].name, "Renamed");
    }

    #[test]
    fn empty_catalog_is_valid_and_invalid_sources_are_rejected() {
        assert!(parse(r#"{"local-projects":{}}"#).unwrap().is_empty());
        for source in [
            "{",
            "{}",
            r#"{"local-projects":[]}"#,
            r#"{"local-projects":{"a":{"id":"b","name":"Project"}}}"#,
            r#"{"local-projects":{"a":{"id":"a","name":"  "}}}"#,
            r#"{"local-projects":{"a:b":{"id":"a:b","name":"Project"}}}"#,
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn sync_payload_has_stable_host_identity_and_only_project_ids_and_names() {
        let home = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.test-run/mcp-project-sync")
            .join(std::process::id().to_string());
        std::fs::create_dir_all(&home).unwrap();
        let projects = vec![ProjectInfo {
            id: "project-id".into(),
            name: "Current name".into(),
        }];
        let payload = sync_payload_at(&home, projects.clone(), "a".repeat(64)).unwrap();
        let second = sync_payload_at(&home, projects, "a".repeat(64)).unwrap();
        assert_eq!(payload.host_id, second.host_id);
        assert_eq!(
            payload.host_id,
            crate::codex_identity::host_id_at(&home).unwrap()
        );
        assert_eq!(payload.computer_name, computer_name());
        assert_eq!(payload.token, "a".repeat(64));
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(
            json["hostId"],
            crate::codex_identity::host_id_at(&home).unwrap()
        );
        assert_eq!(json["computerName"], computer_name());
        assert_eq!(json["token"], "a".repeat(64));
        assert_eq!(
            json["projects"],
            serde_json::json!([{"id":"project-id","name":"Current name"}])
        );
        assert!(!json.to_string().contains("rootPaths"));
        assert!(!json.to_string().contains("C:\\"));
        let _ = std::fs::remove_dir_all(&home);
    }
}
