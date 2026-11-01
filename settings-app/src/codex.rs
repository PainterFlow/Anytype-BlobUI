//! Preferences only. This module never grants access or connects to Codex.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration { pub spaces: BTreeMap<String, Permission> }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission { pub access: String, pub files_enabled: bool, pub approval: String }
pub fn normalize(value: &Value) -> Configuration {
    let mut spaces = BTreeMap::new();
    if let Some(entries) = value.get("spaces").and_then(Value::as_object) {
        for (id, entry) in entries {
            let access = entry["access"].as_str().filter(|v| ["none", "read", "readWrite", "createDelete"].contains(v)).unwrap_or("none");
            let approval = entry["approval"].as_str().filter(|v| ["alwaysAsk", "approveForMe", "fullAccess"].contains(v)).unwrap_or("approveForMe");
            spaces.insert(id.clone(), Permission { access: access.into(), files_enabled: entry["filesEnabled"].as_bool().unwrap_or(false), approval: approval.into() });
        }
    }
    Configuration { spaces }
}
pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Configuration, D::Error> {
    Ok(normalize(&Value::deserialize(deserializer)?))
}
fn discover(client: &crate::kanban_api::Client) -> Result<Vec<Value>, String> {
    client.pages("GET", "/v2/spaces?ids=full", None)?.iter().map(|space| {
        let id = space["id"].as_str().filter(|id| !id.is_empty()).ok_or("Space discovery returned a missing ID.")?;
        Ok(serde_json::json!({"id": id, "name": space["name"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or("Unnamed space")}))
    }).collect()
}
#[tauri::command]
pub async fn discover_codex_spaces() -> Result<Vec<Value>, String> {
    tauri::async_runtime::spawn_blocking(|| discover(&crate::kanban_api::Client::connect()?)).await
        .map_err(|_| "Space discovery stopped. Reload to retry.".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn preferences_round_trip_and_invalid_fallback() {
        let mut old = serde_json::to_value(crate::SettingsDraft::default()).unwrap();
        old.as_object_mut().unwrap().remove("codexIntegration");
        assert!(serde_json::from_value::<crate::SettingsDraft>(old).unwrap().codex_integration.spaces.is_empty());
        let config = normalize(&json!({"spaces":{"full-space-id":{"access":"createDelete","filesEnabled":true,"approval":"alwaysAsk"},"absent":{"access":"invalid","filesEnabled":"true","approval":null}}}));
        assert_eq!(config.spaces["absent"].access,"none");
        assert!(!config.spaces["absent"].files_enabled);
        assert_eq!(config.spaces["absent"].approval,"approveForMe");
        let settings = crate::SettingsDraft { codex_integration: config.clone(), ..crate::SettingsDraft::default() };
        let mut data = json!({"unrelated":{"keep":42},"settings":{"other":true}});
        crate::apply_settings(&mut data, std::path::Path::new("fixture"), &settings);
        assert_eq!(crate::settings_from_data(&data).codex_integration, config);
        assert_eq!(data["unrelated"]["keep"],42);
        assert_eq!(data["settings"]["other"],true);
        assert_eq!(data["settings"]["codexIntegration"],data["settingsV2"]["settings"]["codexIntegration"]);
        assert!(normalize(&Value::Null).spaces.is_empty());
        for access in ["none","read","readWrite","createDelete"] { for approval in ["alwaysAsk","approveForMe","fullAccess"] {
            let config = normalize(&json!({"spaces":{"id":{"access":access,"approval":approval,"filesEnabled":true}}}));
            assert_eq!(serde_json::from_value::<Configuration>(serde_json::to_value(&config).unwrap()).unwrap(),config);
        }}
    }
    #[test]
    fn discovery_paginates_and_rejects_failure() {
        use std::{io::{Read,Write}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for index in 0..3 {
                let (mut stream,_) = listener.accept().unwrap();
                let mut buffer=[0;8192]; let length=stream.read(&mut buffer).unwrap(); let request=String::from_utf8_lossy(&buffer[..length]);
                assert!(request.starts_with("GET /v2/spaces?ids=full&limit=100&offset="));
                assert!(request.contains("Authorization: Bearer fake-codex-key"));
                let (status,body)=match index {
                    0 => { assert!(request.contains("offset=0 ")); ("200 OK",json!({"data":[{"id":"first-full-id","name":"Same"}],"has_more":true})) },
                    1 => { assert!(request.contains("offset=1 ")); ("200 OK",json!({"data":[{"id":"second-full-id","name":"Same"}],"has_more":false})) },
                    _ => ("503 Service Unavailable",json!({"error":"fixture failure"}))
                };
                let body=body.to_string();write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        let client=crate::kanban_api::Client::with_connection("fake-codex-key","127.0.0.1",port);
        let spaces=discover(&client).unwrap();assert_eq!(spaces.len(),2);assert_eq!(spaces[1]["id"],"second-full-id");
        assert!(discover(&client).is_err()); server.join().unwrap();
    }
}
