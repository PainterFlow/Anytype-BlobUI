//! Saved global space permissions and Anytype space discovery.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(default, rename = "objectMentions")]
    pub object_mentions: crate::anyblob_mentions::Filters,
    pub spaces: BTreeMap<String, Permission>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission { pub access: String, pub files_enabled: bool, pub approval: String }
impl Permission {
    pub fn permits(&self, capability: &str) -> bool {
        match capability {
            "read" => matches!(self.access.as_str(), "read" | "readWrite") || self.access=="createDelete",
            "write" => matches!(self.access.as_str(), "readWrite" | "createDelete"),
            "createDelete" => self.access=="createDelete",
            _ => false,
        }
    }
}
pub fn normalize(value: &Value) -> Configuration {
    let mut spaces = BTreeMap::new();
    if let Some(entries) = value.get("spaces").and_then(Value::as_object) {
        for (id, entry) in entries {
            let access = entry["access"].as_str().filter(|v| ["none", "read", "readWrite", "createDelete"].contains(v)).unwrap_or("none");
            let approval = entry["approval"].as_str().filter(|v| ["alwaysAsk", "approveForMe", "fullAccess"].contains(v)).unwrap_or("approveForMe");
            spaces.insert(id.clone(), Permission { access: access.into(), files_enabled: entry["filesEnabled"].as_bool().unwrap_or(false), approval: approval.into() });
        }
    }
    Configuration { spaces, object_mentions: crate::anyblob_mentions::Filters::normalize(value.get("objectMentions")) }
}
pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Configuration, D::Error> {
    Ok(normalize(&Value::deserialize(deserializer)?))
}
fn valid_file_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

pub(crate) fn image_icon(client: &crate::kanban_api::Client, space_id: &str, file_id: &str) -> Option<Value> {
    if !valid_file_id(space_id) || !valid_file_id(file_id) { return None; }
    let path = format!("/v2/spaces/{}/files/{}/content?width=32", crate::api::encode_segment(space_id), crate::api::encode_segment(file_id));
    let (content_type, bytes) = client.binary(&path).ok()?;
    if bytes.is_empty() || bytes.len() > 256 * 1024 { return None; }
    use base64::Engine;
    Some(serde_json::json!({"src": format!("data:{};base64,{}", content_type.split(';').next().unwrap_or("image/png"), base64::engine::general_purpose::STANDARD.encode(bytes))}))
}

pub(crate) fn icon_value(client: &crate::kanban_api::Client, space_id: &str, icon: &Value) -> Option<Value> {
    if let Some(text) = icon.as_str().filter(|text| !text.trim().is_empty()) {
        return Some(serde_json::json!({"emoji": text}));
    }
    let format = icon["format"].as_str().unwrap_or_default();
    match format {
        "emoji" => icon["emoji"].as_str().or_else(|| icon["value"].as_str()).or_else(|| icon["name"].as_str())
            .filter(|text| !text.trim().is_empty()).map(|text| serde_json::json!({"emoji": text})),
        "file" => icon["file"].as_str().or_else(|| icon["fileId"].as_str()).or_else(|| icon["file_id"].as_str())
            .and_then(|file_id| image_icon(client, space_id, file_id)),
        "icon" => icon["name"].as_str().or_else(|| icon["value"].as_str())
            .filter(|text| !text.trim().is_empty())
            .map(|name| serde_json::json!({"name": name, "color": icon["color"].as_str().unwrap_or_default()})),
        _ => icon["file"].as_str().and_then(|file_id| image_icon(client, space_id, file_id)),
    }
}

fn discover(client: &crate::kanban_api::Client) -> Result<Vec<Value>, String> {
    client.pages("GET", "/v2/spaces?ids=full", None)?.iter().map(|space| {
        let id = space["id"].as_str().filter(|id| !id.is_empty()).ok_or("Space discovery returned a missing ID.")?;
        let mut output = serde_json::json!({"id": id, "name": space["name"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or("Unnamed space")});
        if let Some(file_id) = space["icon_image"].as_str().or_else(|| space["iconImage"].as_str()).filter(|value| !value.is_empty()) {
            if let Some(icon) = image_icon(client, id, file_id) { output["icon"] = icon; }
        }
        Ok(output)
    }).collect()
}
#[tauri::command]
pub async fn discover_codex_spaces(force_refresh: Option<bool>) -> Result<Vec<Value>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::metadata_cache::get("spaces", Value::Null, force_refresh.unwrap_or(false), discover)).await
        .map_err(|_| "Space discovery stopped. Reload to retry.".to_string())?
}

#[tauri::command]
pub async fn discover_anyblob_types(space_ids: Vec<String>, force_refresh: Option<bool>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut types = Vec::new(); let mut errors = Vec::new();
        for sid in space_ids.into_iter().collect::<std::collections::BTreeSet<_>>() {
            blobui_mcp::Operation::ReadObject { space_id: sid.clone(), object_id: sid.clone() }.validate()?;
            let result: Value = crate::metadata_cache::get("types", serde_json::json!([sid]), force_refresh.unwrap_or(false), |client| {
                let spaces = crate::metadata_cache::get("spaces", Value::Null, false, discover)?;
                let Some(space) = spaces.iter().find(|space| space["id"] == sid) else {
                    return Ok(serde_json::json!({"types":[],"errors":[{"spaceId":sid,"error":"A selected space is unavailable."}]}));
                };
                let base = format!("/v2/spaces/{}", crate::api::encode_segment(&sid));
                let rows = source_types(client, &base)?;
                let types: Vec<_> = rows.into_iter().map(|row| {
                    let mut output = serde_json::json!({"spaceId":sid,"space":space["name"],"id":row["id"],"name":row["name"].as_str().unwrap_or("Unnamed type")});
                    if let Some(icon) = icon_value(client, &sid, &row["icon"]) { output["icon"] = icon; }
                    output
                }).collect();
                Ok(serde_json::json!({"types":types,"errors":[]}))
            }).unwrap_or_else(|error| serde_json::json!({"types":[],"errors":[{"spaceId":sid,"error":error}]}));
            types.extend(result["types"].as_array().into_iter().flatten().cloned());
            errors.extend(result["errors"].as_array().into_iter().flatten().cloned());
        }
        Ok(serde_json::json!({"types":types,"errors":errors}))
    }).await.map_err(|_|"Object type discovery stopped. Press Ctrl+R to retry.".to_string())?
}

#[cfg(any(test, feature = "headless-cli"))]
pub(crate) fn discover_sources(client: &crate::kanban_api::Client) -> Result<Vec<Value>, String> {
    let spaces = discover(client)?;
    let mut sources = Vec::new();
    for space in spaces {
        let space_id = space["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("Space discovery returned a missing ID.")?;
        let space_name = space["name"].as_str().unwrap_or("Unnamed space");
        let base = format!("/v2/spaces/{}", crate::api::encode_segment(space_id));
        for source in source_types(client, &base)? {
            let id = source["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or("Type discovery returned a missing full ID.")?;
            sources.push(serde_json::json!({
                "kind": "type",
                "spaceId": space_id,
                "space": space_name,
                "id": id,
                "name": source["name"].as_str().unwrap_or("Unnamed type"),
            }));
        }
    }
    Ok(sources)
}

// Type lists can contain API keys without object IDs, even with ids=full.
// Resolve those entries through the v2 type document before saving permissions.
pub(crate) fn source_types(client: &crate::kanban_api::Client, base: &str) -> Result<Vec<Value>, String> {
    client.pages("GET", &format!("{base}/types?ids=full"), None)?.into_iter().map(|mut row| {
        if row["id"].as_str().filter(|id| !id.is_empty()).is_none() {
            let key = row["key"].as_str().filter(|key| !key.is_empty())
                .ok_or("Type discovery returned a missing API key and full ID.")?;
            let document = client.request("GET", &format!("{base}/types/{}?ids=full", crate::api::encode_segment(key)), None)?;
            let id = document["id"].as_str().filter(|id| !id.is_empty())
                .ok_or("Type discovery returned a missing full ID.")?;
            row["id"] = Value::String(id.into());
            if !document["icon"].is_null() { row["icon"] = document["icon"].clone(); }
        }
        Ok(row)
    }).collect()
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
        assert!(data.get("settingsV2").is_none());
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

    #[test]
    fn headless_source_discovery_includes_space_and_type_names() {
        use std::{io::{Read, Write}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for (expected_path, body) in [
                (
                    "/v2/spaces?ids=full&limit=100&offset=0",
                    r#"{"data":[{"id":"space-1","name":"Work"}],"has_more":false}"#,
                ),
                (
                    "/v2/spaces/space-1/types?ids=full&limit=100&offset=0",
                    r#"{"data":[{"id":"type-1","name":"Task"}],"has_more":false}"#,
                ),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0; 8192];
                let length = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..length]);
                assert!(request.starts_with(&format!("GET {expected_path} HTTP/1.1")));
                let body = body.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let client = crate::kanban_api::Client::with_connection("fake-codex-key", "127.0.0.1", port);
        assert_eq!(
            discover_sources(&client).unwrap(),
            vec![serde_json::json!({
                "kind":"type",
                "spaceId":"space-1",
                "space":"Work",
                "id":"type-1",
                "name":"Task"
            })]
        );
        server.join().unwrap();
    }
}
