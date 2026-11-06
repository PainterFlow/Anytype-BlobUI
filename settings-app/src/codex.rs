//! Saved permission preferences and read-only Anytype source discovery.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(default, rename = "objectMentions")]
    pub object_mentions: crate::anyblob_mentions::Filters,
    pub spaces: BTreeMap<String, Permission>,
    #[serde(default)]
    pub projects: BTreeMap<String, ProjectConfiguration>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfiguration {
    pub name: String,
    pub root_paths: Vec<String>,
    pub sources: Vec<SourcePermission>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePermission {
    pub space_id: String, pub kind: String, pub id: String,
    pub name: String, pub space: String,
    pub permission: Permission,
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
    let mut projects = BTreeMap::new();
    if let Some(entries) = value.get("projects").and_then(Value::as_object) {
        for (id, entry) in entries {
            let mut sources = Vec::new();
            if let Some(rows) = entry["sources"].as_array() {
                for row in rows {
                    let normalized = normalize(&serde_json::json!({"spaces":{"source":row["permission"]}}));
                    let text = |key: &str| row[key].as_str().unwrap_or("").to_string();
                    sources.push(SourcePermission { space_id:text("spaceId"), kind:text("kind"), id:text("id"), name:text("name"), space:text("space"), permission:normalized.spaces["source"].clone() });
                }
            } else if !entry["sources"].is_null() {
                // Corrupt saved selections must never silently restore global access.
                sources.push(SourcePermission { space_id:String::new(),kind:String::new(),id:String::new(),name:"Invalid saved source".into(),space:String::new(),permission: Permission {access:"none".into(),files_enabled:false,approval:"alwaysAsk".into()} });
            }
            projects.insert(id.clone(), ProjectConfiguration {
                name:entry["name"].as_str().unwrap_or(id).into(),
                root_paths:entry["rootPaths"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect(),
                sources,
            });
        }
    }
    Configuration { spaces, projects, object_mentions: crate::anyblob_mentions::Filters::normalize(value.get("objectMentions")) }
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
                let types: Vec<_> = rows.into_iter().map(|row| serde_json::json!({"spaceId":sid,"space":space["name"],"id":row["id"],"name":row["name"].as_str().unwrap_or("Unnamed type")})).collect();
                Ok(serde_json::json!({"types":types,"errors":[]}))
            }).unwrap_or_else(|error| serde_json::json!({"types":[],"errors":[{"spaceId":sid,"error":error}]}));
            types.extend(result["types"].as_array().into_iter().flatten().cloned());
            errors.extend(result["errors"].as_array().into_iter().flatten().cloned());
        }
        Ok(serde_json::json!({"types":types,"errors":errors}))
    }).await.map_err(|_|"Object type discovery stopped. Press Ctrl+R to retry.".to_string())?
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
        }
        Ok(row)
    }).collect()
}

fn discover_sources(client: &crate::kanban_api::Client) -> Result<Vec<Value>, String> {
    let mut sources = Vec::new();
    let mut identities = std::collections::BTreeSet::new();
    for space in discover(client)? {
        let sid = space["id"].as_str().ok_or("Missing space ID.")?;
        let path = format!("/v2/spaces/{}", crate::api::encode_segment(sid));
        let mut add = |kind: &str, id: &str, name: &str| -> Result<(), String> {
            if id.is_empty() || !identities.insert((sid.to_string(),kind.to_string(),id.to_string())) {
                return Err("Source discovery returned a missing or duplicate identity.".into());
            }
            sources.push(serde_json::json!({"spaceId":sid,"space":space["name"],"kind":kind,"id":id,"name":name})); Ok(())
        };
        add("space",sid,space["name"].as_str().unwrap_or("Unnamed space"))?;
        for row in source_types(client, &path)? {
            let id = row["id"].as_str().filter(|s| !s.is_empty()).ok_or("Type discovery returned a missing full ID.")?;
            add("type",id,row["name"].as_str().unwrap_or("Unnamed type"))?;
        }
        for row in client.pages("POST", &format!("{path}/search?ids=full"), Some(&serde_json::json!({})))? {
            let kind = match row["type"].as_str() { Some("collection") => "collection", Some("query" | "set") => "query", _ => continue };
            let id = row["id"].as_str().filter(|s| !s.is_empty()).ok_or("Source discovery returned a missing ID.")?;
            add(kind,id,row["name"].as_str().unwrap_or("Untitled"))?;
        }
    }
    Ok(sources)
}
#[tauri::command]
pub async fn discover_codex_sources(force_refresh: Option<bool>) -> Result<Vec<Value>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::metadata_cache::get("sources", Value::Null, force_refresh.unwrap_or(false), discover_sources)).await
        .map_err(|_| "Source discovery stopped. Reload to retry.".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn project_overrides_round_trip_and_keep_unavailable_selections() {
        let input = json!({"spaces":{"space":{"access":"none"}},"projects":{"stable-id":{"name":"Old name","rootPaths":["C:\\one","C:\\two"],"sources":[{"spaceId":"space","kind":"query","id":"query-id","name":"Tasks","space":"Work","permission":{"access":"readWrite","filesEnabled":true,"approval":"alwaysAsk"}}]}}});
        let config=normalize(&input);
        assert_eq!(config.projects["stable-id"].sources[0].permission.access,"readWrite");
        assert_eq!(config.spaces["space"].access,"none");
        assert_eq!(config,normalize(&serde_json::to_value(&config).unwrap()));
        let malformed=normalize(&json!({"projects":{"saved":{"sources":"invalid"}}}));
        assert!(!malformed.projects["saved"].sources.is_empty());
        assert!(normalize(&json!({"spaces":{}})).projects.is_empty());
    }
    #[test]
    fn source_discovery_is_independent_of_permissions_and_paginates() {
        use std::{io::{Read,Write},net::TcpListener};
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
        let server=std::thread::spawn(move|| {
            for index in 0..6 {
                let(mut stream,_)=listener.accept().unwrap();
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut bytes=Vec::new();
                // Drain the body before closing, otherwise Windows can reset the socket.
                loop {
                    let mut chunk=[0;4096];let n=stream.read(&mut chunk).unwrap();assert!(n>0);bytes.extend_from_slice(&chunk[..n]);
                    assert!(bytes.len()<65536);
                    if let Some(end)=bytes.windows(4).position(|part|part==b"\r\n\r\n") {
                        let header=String::from_utf8_lossy(&bytes[..end]);
                        let length=header.lines().filter_map(|line|line.split_once(':'))
                            .find(|(key,_)|key.eq_ignore_ascii_case("content-length"))
                            .map(|(_,value)|value.trim().parse::<usize>().unwrap()).unwrap_or(0);
                        if bytes.len()>=end+4+length {break;}
                    }
                }
                let request=String::from_utf8_lossy(&bytes);
                let body=match index {
                    0=>{assert!(request.starts_with("GET /v2/spaces?ids=full"));json!({"data":[{"id":"space","name":"Work"}],"has_more":false})},
                    1=>{assert!(request.starts_with("GET /v2/spaces/space/types?ids=full"));json!({"data":[{"key":"task / key","name":"Task"}],"has_more":false})},
                    2=>{assert!(request.starts_with("GET /v2/spaces/space/types/task%20%2F%20key?ids=full "));json!({"id":"type-full"})},
                    3=>{assert!(request.starts_with("POST /v2/spaces/space/search?ids=full"));json!({"data":[{"id":"collection","type":"collection","name":"Tasks"}],"has_more":true})},
                    4=>{assert!(request.contains("offset=1 "));json!({"data":[{"id":"query","type":"query","name":"Tasks"},{"id":"page","type":"page","name":"Skip"}],"has_more":false})},
                    _=>json!({"error":"fixture"})
                };
                let body=body.to_string();let status=if index==5 {"503 Service Unavailable"}else{"200 OK"};
                write!(stream,"HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        let client=crate::kanban_api::Client::with_connection("fixture-key","127.0.0.1",port);
        let sources=discover_sources(&client).unwrap();assert_eq!(sources.len(),4); assert_eq!(sources[1]["id"],"type-full");
        assert_eq!(sources.iter().map(|row|row["kind"].as_str().unwrap()).collect::<Vec<_>>(),vec!["space","type","collection","query"]);
        assert!(discover_sources(&client).is_err());server.join().unwrap();
    }
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
