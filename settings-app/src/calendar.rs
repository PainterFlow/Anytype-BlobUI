//! Calendar property rules. All Anytype access is read-only and uses API v2.
use crate::{kanban_api::Client, kanban_snapshot::Target};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum Rule {
    #[default]
    Inherit,
    None,
    Property { key: String },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Override { pub target: Target, pub rule: Rule }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeRule {
    pub space_id: String,
    pub type_key: String,
    pub default: Rule,
    pub overrides: Vec<Override>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration { pub version: u32, pub types: Vec<TypeRule> }
impl Default for Configuration {
    fn default() -> Self { Self { version: 1, types: Vec::new() } }
}
impl Configuration {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.types.len() > 1000 { return Err("Unsupported calendar rules configuration.".into()); }
        let mut types = BTreeSet::new();
        for entry in &self.types {
            crate::kanban_snapshot::validate_id(&entry.space_id)?;
            crate::kanban_snapshot::validate_id(&entry.type_key)?;
            if !types.insert((&entry.space_id, &entry.type_key)) || entry.overrides.len() > 1000 {
                return Err("Duplicate or excessive calendar type rules.".into());
            }
            validate_rule(&entry.default)?;
            let mut targets = BTreeSet::new();
            for item in &entry.overrides {
                item.target.validate()?;
                if item.target.space_id != entry.space_id || !targets.insert(&item.target) {
                    return Err("Calendar overrides must be unique and belong to the type's space.".into());
                }
                validate_rule(&item.rule)?;
            }
        }
        Ok(())
    }
}
fn validate_rule(rule: &Rule) -> Result<(), String> {
    if let Rule::Property { key } = rule { crate::kanban_snapshot::validate_id(key)?; }
    Ok(())
}
pub fn configuration(data: &Value) -> Result<Configuration, String> {
    let config = match data.get("calendarColorRules") {
        None => Configuration::default(),
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| "The saved calendar rules are malformed.")?,
    };
    config.validate()?;
    Ok(config)
}
fn enc(value: &str) -> String { crate::api::encode_segment(value) }
fn name(value: &Value) -> &str { value["name"].as_str().filter(|name| !name.trim().is_empty()).unwrap_or("Untitled") }
fn required(value: &Value, key: &str) -> Result<String, String> {
    value[key].as_str().filter(|s| !s.is_empty()).map(String::from)
        .ok_or_else(|| format!("Calendar discovery received a missing {key}."))
}

fn type_properties(document: &Value, properties: &[Value]) -> Vec<Value> {
    let definitions = document.get("property_definitions")
        .or_else(|| document.pointer("/type_settings/property_definitions"));
    let keys: BTreeSet<&str> = definitions.and_then(Value::as_array).into_iter().flatten()
        .filter_map(|p| p["property"].as_str().or_else(|| p["key"].as_str())).collect();
    properties.iter().filter(|p| matches!(p["format"].as_str(), Some("select" | "checkbox"))
        && p["key"].as_str().is_some_and(|key| keys.contains(key))).cloned().collect()
}
fn calendar_views(space: &str, space_name: &str, document: &Value) -> Vec<Value> {
    let Some(object_id) = document["id"].as_str() else { return Vec::new(); };
    let mut views = Vec::new();
    for block in document["blocks"].as_array().into_iter().flatten() {
        let Some(block_id) = block["id"].as_str() else { continue; };
        for view in block["views"].as_array().into_iter().flatten() {
            if view["type"] != "calendar" && view["layout"] != "calendar" { continue; }
            let Some(view_id) = view["id"].as_str() else { continue; };
            views.push(json!({"target": {"spaceId":space,"objectId":object_id,"blockId":block_id,"viewId":view_id},
                "name":name(view),"space":space_name,"source":name(document),"kind":"Calendar"}));
        }
    }
    views
}
// Bound API work while preserving input order and per-document failures.
fn parallel_metadata<'a, T: Send>(rows: &'a [Value], read: impl Fn(&Value) -> T + Sync) -> Vec<(&'a Value, T)> {
    use std::sync::{atomic::{AtomicUsize, Ordering}, Mutex};
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(rows.len()));
    std::thread::scope(|scope| {
        for _ in 0..rows.len().min(4) {
            let read = &read; let next = &next; let results = &results;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(row) = rows.get(index) else { break; };
                let result = read(row);
                results.lock().unwrap().push((index, result));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(index, result)| (&rows[index], result)).collect()
}

pub fn discover(client: &Client) -> Result<Value, String> {
    let mut types = Vec::new();
    let mut views = Vec::new();
    let mut info = Vec::new();
    for space in client.pages("GET", "/v2/spaces?ids=full", None)? {
        let sid = required(&space, "id")?;
        let properties = client.pages("GET", &format!("/v2/spaces/{}/properties", enc(&sid)), None)?;
        let rows = client.pages("GET", &format!("/v2/spaces/{}/types?ids=full", enc(&sid)), None)?;
        for result in parallel_metadata(&rows, |row| -> Result<_, String> {
            let key = required(row, "key")?;
            let document = client.request("GET", &format!("/v2/spaces/{}/types/{}?ids=full", enc(&sid), enc(&key)), None);
            Ok((key, document))
        }) {
            let (row, result) = result;
            let (key, document) = result?;
            let eligible = match document {
                Ok(mut document) => {
                    if document["name"].as_str().is_none() { document["name"] = json!(name(row)); }
                    views.extend(calendar_views(&sid, name(&space), &document));
                    type_properties(&document, &properties)
                }
                Err(error) => { info.push(format!("Could not read type '{}'. {error}", name(row))); Vec::new() }
            };
            types.push(json!({"spaceId":sid,"space":name(&space),"key":key,"name":name(row),"properties":eligible}));
        }
        let rows = client.pages("POST", &format!("/v2/spaces/{}/search?ids=full", enc(&sid)), Some(&json!({})))?;
        for (row, result) in parallel_metadata(&rows, |row| -> Result<_, String> {
            let id = required(row, "id")?;
            Ok(client.calendar_document(&format!("/v2/spaces/{}/objects/{}?ids=full&include=blocks", enc(&sid), enc(&id))))
        }) {
            match result? {
                Ok(mut document) => {
                    if document["name"].as_str().is_none() { document["name"] = json!(name(row)); }
                    views.extend(calendar_views(&sid, name(&space), &document));
                }
                Err(error) => info.push(format!("Could not inspect calendar views in '{}'. {error}", name(row))),
            }
        }
    }
    let mut seen = BTreeSet::new();
    views.retain(|view| seen.insert(view["target"].to_string()));
    info.extend(client.take_info());
    Ok(json!({"types":types,"views":views,"info":info}))
}

// None means fall through. Some(None) explicitly suppresses coloring.
fn resolve(rule: &Rule, object: &Value, eligible: &[Value], options: &BTreeMap<String, Vec<Value>>) -> Option<Option<String>> {
    match rule {
        Rule::Inherit => None,
        Rule::None => Some(None),
        Rule::Property { key } => {
            let property = eligible.iter().find(|p| p["key"] == *key)?;
            let raw = crate::api::object_property(object, key, "")?;
            let format = property["format"].as_str()?;
            let value = raw.get(format).unwrap_or(raw);
            if format == "checkbox" {
                return value.as_bool().map(|value| Some(if value { "Green" } else { "Grey" }.into()));
            }
            // v2 search serves single-select properties as arrays too. Only a
            // single value is valid here; never choose arbitrarily from many.
            let value = match value.as_array() {
                Some(values) if values.len() == 1 => &values[0],
                Some(_) => return None,
                None => value,
            };
            let selected = value.as_str().or_else(|| value["name"].as_str())?;
            let matching = options.get(key)?.iter().filter(|option| option["name"] == selected).collect::<Vec<_>>();
            if matching.len() != 1 { return None; }
            let color = matching[0]["color"].as_str()?;
            let color = match color { "orange" => "Amber", "ice" => "Sky", "lime" => "Green", other => other };
            let canonical = crate::api::canonical_calendar_color(color).ok()?;
            crate::css::validate_calendar_color(&canonical).ok()?;
            Some(Some(canonical))
        }
    }
}
pub fn snapshot(key: &str, ip: &str, port: u16, config: &Configuration) -> Result<Value, String> {
    config.validate()?;
    let mut base = crate::api::calendar_colors(key, ip, port)?;
    let mut overrides = Vec::new();
    let mut info = Vec::new();
    let client = Client::with_connection(key, ip, port);
    for entry in &config.types {
        let space = enc(&entry.space_id);
        let document = match client.request("GET", &format!("/v2/spaces/{space}/types/{}?ids=full", enc(&entry.type_key)), None) {
            Ok(value) => value,
            Err(error) if error.contains("404") || error.contains("not found") => { info.push(format!("Unavailable calendar type '{}'.", entry.type_key)); continue; }
            Err(error) => return Err(error),
        };
        let properties = client.pages("GET", &format!("/v2/spaces/{space}/properties"), None)?;
        let eligible = type_properties(&document, &properties);
        let mut options = BTreeMap::new();
        let rules = std::iter::once(&entry.default).chain(entry.overrides.iter().map(|o| &o.rule));
        let mut fields = BTreeSet::new();
        for rule in rules {
            if let Rule::Property { key } = rule {
                if !fields.insert(key.clone()) { continue; }
                if let Some(property) = eligible.iter().find(|p| p["key"] == *key) {
                    if property["format"] == "select" {
                        let values = client.pages("GET", &format!("/v2/spaces/{space}/properties/{}/options?ids=full", enc(key)), None)?;
                        let mut names = BTreeSet::new();
                        for value in &values {
                            if !names.insert(value["name"].clone().to_string()) {
                                info.push(format!("Ambiguous option name on calendar property '{key}'. Matching objects use their fallback color."));
                            }
                            if value["color"].as_str().and_then(|color| crate::api::canonical_calendar_color(match color { "orange" => "Amber", "ice" => "Sky", "lime" => "Green", other => other }).ok()).is_none() {
                                info.push(format!("Unavailable option color on calendar property '{key}'. Matching objects use their fallback color."));
                            }
                        }
                        options.insert(key.clone(), values);
                    }
                } else { info.push(format!("Unavailable calendar property '{key}' on '{}'.", entry.type_key)); }
            }
        }
        if fields.is_empty() && matches!(entry.default, Rule::Inherit) && entry.overrides.is_empty() { continue; }
        // Let v2 resolve the canonical type. Comparing the search row spelling
        // locally can skip valid objects and leave their legacy color in place.
        let objects = client.pages("POST", &format!("/v2/spaces/{space}/search?ids=full"), Some(&json!({"type": document.pointer("/type_settings/api_key").and_then(Value::as_str).filter(|key| !key.is_empty()).or_else(|| document["internal_key"].as_str().filter(|key| !key.is_empty())).unwrap_or(&entry.type_key), "fields":fields.into_iter().collect::<Vec<_>>()})))?;
        for object in objects {
            let id = required(&object, "id")?;
            let fallback = resolve(&entry.default, &object, &eligible, &options);
            let legacy = base.get(&id).cloned();
            if let Some(color) = &fallback {
                if let Some(color) = color { base[&id] = json!(color); }
                else { base.as_object_mut().ok_or("Invalid calendar cache.")?.remove(&id); }
            }
            for item in &entry.overrides {
                let resolved = resolve(&item.rule, &object, &eligible, &options).or_else(|| fallback.clone());
                let color = match resolved { Some(color) => json!(color), None => legacy.clone().unwrap_or(Value::Null) };
                // Inheritance needs no scoped selector. Explicit rules do, including no color.
                if !matches!(item.rule, Rule::Inherit) {
                    overrides.push(json!({"target":item.target,"objectId":id,"color":color}));
                    if overrides.len() > 100_000 { return Err("The calendar color snapshot is too large to publish safely.".into()); }
                }
            }
        }
    }
    info.extend(client.take_info());
    Ok(json!({"base":base,"overrides":overrides,"info":info}))
}

#[tauri::command]
pub fn load_calendar_rules() -> Result<Configuration, String> {
    let install = crate::resolve_install_directory(&crate::app_data_directory())?;
    configuration(&crate::read_json_file(&install.join("blob/data.json"))?)
}
#[tauri::command]
pub async fn discover_calendar_colors() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| discover(&Client::connect()?)).await
        .map_err(|_| "Calendar discovery stopped. Reload to retry.".to_string())?
}
#[tauri::command]
pub async fn save_calendar_rules(configuration: Configuration, settings: Option<crate::SettingsDraft>) -> Result<crate::SaveResult, String> {
    configuration.validate()?;
    let guard = crate::acquire_save_guard("Wait for the current settings save to finish.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let install = crate::resolve_install_directory(&crate::app_data_directory())?;
        let data = crate::read_json_file(&install.join("blob/data.json"))?;
        crate::save_settings_with_calendar_rules(settings.unwrap_or_else(|| crate::settings_from_data(&data)), Some(configuration))
    }).await.map_err(|_| "Calendar save stopped. Your pending edits are kept.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_reads_overlap_and_preserve_order() {
        use std::sync::{atomic::{AtomicUsize, Ordering}, Barrier};
        let rows = (0..12).map(|n| json!(n)).collect::<Vec<_>>();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let barrier = Barrier::new(4);
        let results = parallel_metadata(&rows, |row| {
            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(count, Ordering::SeqCst);
            barrier.wait();
            active.fetch_sub(1, Ordering::SeqCst);
            row.as_u64().unwrap()
        });
        assert_eq!(peak.load(Ordering::SeqCst), 4);
        assert_eq!(results.into_iter().map(|(_, value)| value).collect::<Vec<_>>(), (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn calendar_rules_resolve_only_typed_properties_and_unambiguous_colors() {
        let properties = vec![json!({"key":"done","format":"checkbox"}), json!({"key":"status","format":"select"}), json!({"key":"tags","format":"multi_select"})];
        let eligible = type_properties(&json!({"type_settings":{"property_definitions":[{"property":"done"},{"property":"status"},{"property":"tags"}]}}), &properties);
        assert_eq!(eligible.len(), 2);
        let checkbox = Rule::Property { key: "done".into() };
        let select = Rule::Property { key: "status".into() };
        let mut options = BTreeMap::from([("status".into(), vec![json!({"name":"Active","color":"lime"})])]);
        assert_eq!(resolve(&checkbox, &json!({"properties":{"done":true}}), &eligible, &options), Some(Some("Green".into())));
        assert_eq!(resolve(&checkbox, &json!({"properties":{"done":{"checkbox":false}}}), &eligible, &options), Some(Some("Grey".into())));
        assert_eq!(resolve(&checkbox, &json!({"properties":{"done":null}}), &eligible, &options), None);
        assert_eq!(resolve(&checkbox, &json!({"properties":{}}), &eligible, &options), None);
        assert_eq!(resolve(&select, &json!({"properties":{"status":"Active"}}), &eligible, &options), Some(Some("Green".into())));
        for object in [
            json!({"properties":{"status":["Active"]}}),
            json!({"properties":{"status":{"select":["Active"]}}}),
            json!({"properties":[{"key":"status","select":["Active"]}]}),
        ] {
            assert_eq!(resolve(&select, &object, &eligible, &options), Some(Some("Green".into())));
        }
        for value in [json!([]), json!(["Active","Other"]), json!([null]), json!(["Unknown"])] {
            assert_eq!(resolve(&select, &json!({"properties":{"status":value}}), &eligible, &options), None);
        }
        options.get_mut("status").unwrap().push(json!({"name":"Active","color":"red"}));
        assert_eq!(resolve(&select, &json!({"properties":{"status":"Active"}}), &eligible, &options), None);
        assert_eq!(resolve(&select, &json!({"properties":{"status":["Active"]}}), &eligible, &options), None);
        assert_eq!(resolve(&Rule::None, &json!({}), &eligible, &options), Some(None));
        assert_eq!(resolve(&Rule::Inherit, &json!({}), &eligible, &options), None);
        assert_eq!(resolve(&Rule::Property{key:"missing".into()}, &json!({}), &eligible, &options), None);
    }
    #[test]
    fn calendar_rules_validate_and_preserve_stable_identities() {
        let target = json!({"spaceId":"space","objectId":"object","blockId":"block","viewId":"view"});
        let value = json!({"version":1,"types":[{"spaceId":"space","typeKey":"task","default":{"mode":"property","key":"status"},"overrides":[{"target":target,"rule":{"mode":"none"}}]}]});
        let config: Configuration = serde_json::from_value(value.clone()).unwrap();
        config.validate().unwrap(); assert_eq!(serde_json::to_value(&config).unwrap(), value);
        let mut invalid = config.clone(); invalid.types.push(config.types[0].clone()); assert!(invalid.validate().is_err());
        let mut invalid = config; invalid.types[0].overrides[0].target.space_id = "other".into(); assert!(invalid.validate().is_err());
        assert!(configuration(&json!({"calendarColorRules":{"version":2,"types":[]}})).is_err());
        assert_eq!(configuration(&json!({})).unwrap().version, 1);
    }
    #[test]
    fn calendar_discovery_uses_calendar_layout_without_kanban_requirements() {
        let views = calendar_views("space", "Work", &json!({"id":"host","name":"Tasks","blocks":[{"id":"inline","object_id":"source","views":[{"id":"calendar-a","type":"calendar","name":"Month"},{"id":"calendar-b","layout":"calendar","name":"Week"},{"id":"board","type":"kanban"}]}]}));
        assert_eq!(views.len(), 2); assert_eq!(views[0]["target"]["objectId"], "host");
    }

    #[test]
    fn calendar_v2_fixture_covers_discovery_paging_precedence_and_outages() {
        use std::{io::{Read, Write}, net::TcpListener, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false)); let done = stop.clone();
        let fail = Arc::new(AtomicBool::new(false)); let failure = fail.clone();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !done.load(Ordering::Relaxed) && Instant::now() < deadline {
                let Ok((mut stream, _)) = listener.accept() else { std::thread::sleep(Duration::from_millis(1)); continue; };
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut bytes = Vec::new(); let mut buffer = [0;4096];
                loop {
                    let n = stream.read(&mut buffer).unwrap(); if n == 0 { break; } bytes.extend_from_slice(&buffer[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let size = headers.lines().find_map(|l| l.strip_prefix("Content-Length: ")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                        if bytes.len() >= end + 4 + size { break; }
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                assert!(request.contains("Authorization: Bearer fixture-key"));
                let path = request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                assert!(path.starts_with("/v2/"));
                let page = |data: Value| json!({"data":data,"has_more":false});
                let properties = json!([
                    {"key":"legacy","name":"Calendar Color","format":"text"},
                    {"key":"status","name":"Status","format":"select"},
                    {"key":"done","name":"Done","format":"checkbox"},
                    {"key":"tags","name":"Tags","format":"multi_select"}
                ]);
                let document = json!({"id":"type-task","name":"Task","type_settings":{"property_definitions":[{"property":"status"},{"property":"done"},{"property":"tags"}]},"blocks":[{"id":"calendar","views":[{"id":"month","name":"Month","type":"calendar"}]}]});
                let response = if path.starts_with("/v2/spaces?") { page(json!([{"id":"space","name":"Work"}])) }
                else if path.starts_with("/v2/spaces/space/types?") { page(json!([{"key":"task","name":"Task"}])) }
                else if path.starts_with("/v2/spaces/space/types/task?") { document }
                else if path.starts_with("/v2/spaces/space/properties/status/options?") { page(json!([{"name":"Active","color":"blue"}])) }
                else if path.starts_with("/v2/spaces/space/properties?") { page(properties) }
                else if path.starts_with("/v2/spaces/space/objects/") { json!({"id":"event","name":"Event","blocks":[]}) }
                else if path.starts_with("/v2/spaces/space/search?") {
                    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
                    let payload: Value = serde_json::from_str(body).unwrap();
                    if payload["fields"].as_array().is_some_and(|fields| fields.iter().any(|field| field == "done")) {
                        assert_eq!(payload["type"], "task", "Property rules must use a server-side type filter");
                    }
                    if path.contains("offset=0") { json!({"data":[{"id":"event","type":"type-task-full-id","properties":{"legacy":"Pink","done":false,"status":["Active"]}}],"has_more":true}) }
                    else { page(json!([{"id":"empty","type":"task","properties":{"legacy":"Purple","done":true,"status":[]}},{"id":"missing","type":"task","properties":{"legacy":"Red"}}])) }
                } else { panic!("Unexpected fixture request {path}"); };
                let body = serde_json::to_vec(&response).unwrap();
                let status = if failure.load(Ordering::Relaxed) { "503 Service Unavailable" } else { "200 OK" };
                write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap(); stream.write_all(&body).unwrap();
            }
        });
        let result = (|| -> Result<(), String> {
            let client = Client::with_connection("fixture-key", "127.0.0.1", port);
            let catalog = discover(&client)?;
            assert_eq!(catalog["types"][0]["properties"].as_array().unwrap().len(), 2);
            assert_eq!(catalog["views"].as_array().unwrap().len(), 1);
            let target = |view| Target { space_id:"space".into(), object_id:"type-task".into(), block_id:"calendar".into(), view_id:view };
            let mut config = Configuration { version:1, types:vec![TypeRule { space_id:"space".into(), type_key:"task".into(),
                default:Rule::Property {key:"done".into()}, overrides:vec![Override { target:target("month".into()), rule:Rule::Property { key:"status".into() } }, Override { target:target("none".into()), rule:Rule::None }] }] };
            let colors = snapshot("fixture-key", "127.0.0.1", port, &config)?;
            assert_eq!(colors["base"]["event"], "Grey"); assert_eq!(colors["base"]["empty"], "Green"); assert_eq!(colors["base"]["missing"], "Red");
            assert!(colors["overrides"].as_array().unwrap().iter().any(|v| v["objectId"] == "event" && v["color"] == "Blue"));
            assert!(colors["overrides"].as_array().unwrap().iter().any(|v| v["objectId"] == "empty" && v["color"] == "Green"));
            assert!(colors["overrides"].as_array().unwrap().iter().any(|v| v["objectId"] == "missing" && v["color"] == "Red"));
            assert!(colors["overrides"].as_array().unwrap().iter().any(|v| v["target"]["viewId"] == "none" && v["color"].is_null()));
            config.types[0].default = Rule::Property { key: "status".into() };
            let colors = snapshot("fixture-key", "127.0.0.1", port, &config)?;
            assert_eq!(colors["base"]["event"], "Blue", "A single-select array must override the legacy Pink color");
            assert_eq!(colors["base"]["empty"], "Purple", "An empty selection must use the legacy fallback");
            let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let root = workspace.join(".test-run/calendar-property");
            std::fs::create_dir_all(&root).unwrap();
            let mut data = json!({"calendarObjectColors": colors["base"], "calendarViewColors": []});
            crate::apply_settings(&mut data, &root, &crate::SettingsDraft { accent_mode: "native".into(), accent_color: "Default".into(), accent_preset: String::new(), ..crate::SettingsDraft::default() });
            let css = crate::css::compile(workspace, &root, &data)?;
            std::fs::write(root.join("array-priority.css"), css).unwrap();
            config.types[0].default = Rule::None;
            let colors = snapshot("fixture-key", "127.0.0.1", port, &config)?;
            assert!(colors["base"].as_object().unwrap().is_empty());
            fail.store(true, Ordering::Relaxed);
            assert!(snapshot("fixture-key", "127.0.0.1", port, &config).is_err());
            Ok(())
        })();
        stop.store(true, Ordering::Relaxed); server.join().unwrap(); result.unwrap();
    }
}
