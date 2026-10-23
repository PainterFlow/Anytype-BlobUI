//! Read-only Anytype adapter. Full IDs are retained; incomplete pages fail closed.
use crate::{
    api,
    calculator::Calculator,
    kanban::{CategoryResult, Property, Target, View},
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub struct Client {
    key: crate::SecretString,
    ip: String,
    port: u16,
}
impl Client {
    pub fn connect() -> Result<Self, String> {
        let install = crate::resolve_install_directory(&crate::app_data_directory())?;
        let data = crate::read_json_file(&install.join("blob/data.json"))?;
        if data
            .pointer("/apiConnection/enabled")
            .and_then(Value::as_bool)
            == Some(false)
            || !install
                .join("blob/credentials/anytype-api-key.dpapi")
                .is_file()
        {
            return Err(
                "Add an Anytype API key in Advanced Settings to use Kanban formulas.".into(),
            );
        }
        Ok(Self {
            key: crate::SecretString::new(crate::read_saved_api_key_at(&install)?),
            ip: crate::normalize_api_ip(data.pointer("/apiConnection/ip").and_then(Value::as_str))?,
            port: data
                .pointer("/apiConnection/port")
                .and_then(Value::as_u64)
                .and_then(|p| u16::try_from(p).ok())
                .filter(|p| *p > 0)
                .unwrap_or(31009),
        })
    }
    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        api::request(&self.key, &self.ip, self.port, method, path, body)
    }
    pub fn pages(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Vec<Value>, String> {
        complete_pages(|offset| {
            let separator = if path.contains('?') { '&' } else { '?' };
            self.request(
                method,
                &format!("{path}{separator}limit=100&offset={offset}"),
                body,
            )
        })
    }
    fn document(&self, space: &str, object: &str) -> Result<Value, String> {
        self.request(
            "GET",
            &format!("/v2/spaces/{}/objects/{}?ids=full", enc(space), enc(object)),
            None,
        )
    }
    fn properties(&self, space: &str) -> Result<Vec<Property>, String> {
        self.pages(
            "GET",
            &format!("/v2/spaces/{}/properties", enc(space)),
            None,
        )?
        .iter()
        .map(|p| {
            Ok(Property {
                key: required(p, "key")?,
                name: required(p, "name")?,
                format: required(p, "format")?,
            })
        })
        .collect()
    }

    pub fn discover(&self) -> Result<crate::kanban::Catalog, String> {
        let mut views = Vec::new();
        let mut unreadable = 0usize;
        for space in self.pages("GET", "/v2/spaces?ids=full", None)? {
            let sid = required(&space, "id")?;
            let name = space.get("name").and_then(Value::as_str).unwrap_or("Space");
            let properties = self.properties(&sid)?;
            let rows = self.pages(
                "POST",
                &format!("/v2/spaces/{}/search", enc(&sid)),
                Some(&json!({})),
            )?;
            for row in rows {
                let oid = required(&row, "id")?;
                let document = match self.document(&sid, &oid) {
                    Ok(document) => document,
                    Err(_) => {
                        unreadable += 1;
                        continue;
                    }
                };
                let source = row
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Untitled");
                let kind = list_kind(&document);
                let exposed = if let Some(kind) = kind {
                    match self.pages(
                        "GET",
                        &format!(
                            "/v2/spaces/{}/{}/{}/views?ids=full",
                            enc(&sid),
                            kind,
                            enc(&oid)
                        ),
                        None,
                    ) {
                        Ok(value) => Some(value),
                        Err(_) => {
                            unreadable += 1;
                            None
                        }
                    }
                } else {
                    None
                };
                for block in document
                    .get("blocks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    for raw in block
                        .get("views")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if raw
                            .get("type")
                            .or_else(|| raw.get("layout"))
                            .and_then(Value::as_str)
                            != Some("kanban")
                        {
                            continue;
                        }
                        let (Ok(block_id), Ok(view_id)) =
                            (required(block, "id"), required(raw, "id"))
                        else {
                            unreadable += 1;
                            continue;
                        };
                        let target = Target {
                            space_id: sid.clone(),
                            object_id: oid.clone(),
                            block_id,
                            view_id,
                        };
                        let mut view = View {
                            target,
                            name: raw
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("Kanban")
                                .into(),
                            space: name.into(),
                            source: source.into(),
                            kind: if kind == Some("collections") {
                                "Collection"
                            } else if kind.is_some() {
                                "Query"
                            } else {
                                "Inline / type view"
                            }
                            .into(),
                            properties: properties
                                .iter()
                                .filter(|p| p.format == "number")
                                .cloned()
                                .collect(),
                            unsupported: None,
                        };
                        let supported = (|| {
                            verify_exposed(&view.target,exposed.as_deref().ok_or("The API does not expose a filtered membership endpoint for this view.")?)?;
                            self.grouping(&view.target, raw, &properties)?;
                            Ok::<(), String>(())
                        })();
                        if let Err(error) = supported {
                            view.unsupported = Some(error);
                        }
                        views.push(view);
                    }
                }
            }
        }
        views.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.space.cmp(&b.space))
                .then(a.source.cmp(&b.source))
        });
        let mut seen = BTreeSet::new();
        if views.iter().any(|v| !seen.insert(v.target.clone())) {
            return Err(
                "Anytype returned duplicate view identities. Refresh the view list.".into(),
            );
        }
        Ok(crate::kanban::Catalog {
            views,
            warning: if unreadable > 0 {
                Some("Some documents or view endpoints could not be read. The view list may be incomplete. Check the key's space access and reload the list.".into())
            } else {
                None
            },
        })
    }

    pub fn calculate(
        &self,
        target: &Target,
        calculator: &Calculator,
    ) -> Result<Vec<CategoryResult>, String> {
        target.validate()?;
        let document = self.document(&target.space_id, &target.object_id)?;
        let kind = list_kind(&document)
            .ok_or("This view has no supported filtered membership endpoint.")?;
        let block = document["blocks"]
            .as_array()
            .and_then(|b| b.iter().find(|b| b["id"] == target.block_id))
            .ok_or("This board is unavailable. Reload the view list.")?;
        let raw = block["views"]
            .as_array()
            .and_then(|v| v.iter().find(|v| v["id"] == target.view_id))
            .ok_or("This view is unavailable. Reload the view list.")?;
        if raw["type"] != "kanban" && raw["layout"] != "kanban" {
            return Err("This view is no longer a Kanban view.".into());
        }
        let exposed = self.pages(
            "GET",
            &format!(
                "/v2/spaces/{}/{}/{}/views?ids=full",
                enc(&target.space_id),
                kind,
                enc(&target.object_id)
            ),
            None,
        )?;
        verify_exposed(target, &exposed)?;
        let properties = self.properties(&target.space_id)?;
        let keys = calculator.properties();
        if keys.iter().any(|key| {
            !properties
                .iter()
                .any(|p| p.key == *key && p.format == "number")
        }) {
            return Err("A calculation property was deleted or is no longer numeric. Choose a replacement property.".into());
        }
        let mut grouping = self.grouping(target, raw, &properties)?;
        let mut fields = keys;
        fields.push(grouping.property.clone());
        fields.sort();
        fields.dedup();
        let rows = self.pages(
            "GET",
            &format!(
                "/v2/spaces/{}/{}/{}/objects?view={}&fields={}",
                enc(&target.space_id),
                kind,
                enc(&target.object_id),
                enc(&target.view_id),
                enc(&fields.join(","))
            ),
            None,
        )?;
        let mut grouped: BTreeMap<String, Vec<Value>> = grouping
            .categories
            .keys()
            .map(|id| (id.clone(), Vec::new()))
            .collect();
        let mut unique = BTreeSet::new();
        for row in rows {
            if !unique.insert(required(&row, "id")?) {
                return Err(
                    "The board changed during pagination. Refresh results to retry.".into(),
                );
            }
            let value = row
                .get("properties")
                .and_then(|p| p.get(&grouping.property))
                .unwrap_or(&Value::Null);
            let id = grouping.category(value)?;
            if !grouping.categories.contains_key(&id) {
                grouping.categories.insert(id.clone(), String::new());
            }
            grouped.entry(id).or_default().push(row);
        }
        if grouped.len() > 4096 {
            return Err("This view has too many categories to publish safely.".into());
        }
        grouped
            .into_iter()
            .map(|(id, cards)| {
                let value = calculator.evaluate(&cards)?;
                Ok(CategoryResult {
                    target: target.clone(),
                    category_id: id,
                    value,
                })
            })
            .collect()
    }

    fn grouping(
        &self,
        target: &Target,
        view: &Value,
        properties: &[Property],
    ) -> Result<Grouping, String> {
        let property = required(view, "group_by")
            .map_err(|_| "The API does not expose this view's grouping property.".to_string())?;
        let format = properties
            .iter()
            .find(|p| p.key == property)
            .ok_or("The grouping property is unavailable.")?
            .format
            .clone();
        let mut grouping = Grouping {
            property,
            format: format.clone(),
            options: BTreeMap::new(),
            categories: BTreeMap::new(),
        };
        if format == "checkbox" {
            grouping.categories.insert("true".into(), String::new());
            grouping.categories.insert("false".into(), String::new());
            return Ok(grouping);
        }
        if !matches!(format.as_str(), "select" | "multi_select") {
            return Err("This grouping format cannot be matched to Anytype headers safely.".into());
        }
        let options = self.pages(
            "GET",
            &format!(
                "/v2/spaces/{}/properties/{}/options?ids=full",
                enc(&target.space_id),
                enc(&grouping.property)
            ),
            None,
        )?;
        // v2 usually exposes option names only. Select group IDs are the option
        // object IDs; reading those objects resolves names without API v1.
        let mut resolved = BTreeMap::new();
        for option in &options {
            if let (Some(id), Some(name)) = (
                option.get("id").and_then(Value::as_str),
                option.get("name").and_then(Value::as_str),
            ) {
                resolved.insert(name.to_string(), id.to_string());
            }
        }
        if format == "select" {
            for group in view
                .get("groups")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let id = required(group, "id")?;
                if id == "empty" {
                    continue;
                }
                let object = self.document(&target.space_id, &id).map_err(|_| {
                    "The API could not resolve a category's exact identity.".to_string()
                })?;
                let name = object
                    .pointer("/properties/name")
                    .and_then(Value::as_str)
                    .ok_or("The API did not return a category option name.")?;
                if resolved.get(name).is_some_and(|old| old != &id) {
                    return Err("Duplicate option names prevent an exact category match.".into());
                }
                resolved.insert(name.into(), id);
            }
        }
        // Some keys allow the legacy read endpoints, which expose stable option
        // IDs for multi-select. A refusal is not bypassed or treated as success.
        if options.iter().any(|o| {
            o["name"]
                .as_str()
                .is_none_or(|name| !resolved.contains_key(name))
        }) {
            if let Ok(tags) = self.pages(
                "GET",
                &format!(
                    "/v1/spaces/{}/properties/{}/tags",
                    enc(&target.space_id),
                    enc(&grouping.property)
                ),
                None,
            ) {
                for tag in tags {
                    if let (Some(id), Some(name)) = (tag["id"].as_str(), tag["name"].as_str()) {
                        if resolved.get(name).is_some_and(|old| old != id) {
                            return Err(
                                "Duplicate option names prevent an exact category match.".into()
                            );
                        }
                        resolved.insert(name.into(), id.into());
                    }
                }
            }
        }
        for option in options {
            let name = required(&option, "name")?;
            let id=resolved.get(&name).ok_or("The API does not expose every option's stable ID. This view cannot publish results safely.")?;
            crate::kanban::validate_id(id)?;
            if grouping.options.insert(name.clone(), id.clone()).is_some() {
                return Err("Duplicate option names prevent an exact category match.".into());
            }
            grouping.categories.insert(
                if format == "multi_select" {
                    md5(id)
                } else {
                    id.clone()
                },
                name,
            );
        }
        grouping.categories.insert("empty".into(), String::new());
        Ok(grouping)
    }
}

fn verify_exposed(target: &Target, exposed: &[Value]) -> Result<(), String> {
    // API routes accept a unique suffix, but storage/CSS always use the full ID
    // from the document. Ambiguous suffixes are refused, not guessed.
    let matches = exposed
        .iter()
        .filter(|v| {
            v["id"].as_str().is_some_and(|id| {
                id == target.view_id || (id.len() >= 6 && target.view_id.ends_with(id))
            })
        })
        .count();
    if matches != 1 {
        return Err(
            "The API does not expose an unambiguous filtered endpoint for this view.".into(),
        );
    }
    Ok(())
}
fn list_kind(document: &Value) -> Option<&'static str> {
    match document["type"].as_str() {
        Some("collection") => Some("collections"),
        Some("query" | "set") => Some("queries"),
        _ => None,
    }
}
fn enc(s: &str) -> String {
    api::encode_segment(s)
}
fn required(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| "Anytype did not return the required view metadata.".into())
}

pub(crate) fn complete_pages(
    mut fetch: impl FnMut(usize) -> Result<Value, String>,
) -> Result<Vec<Value>, String> {
    let mut all = Vec::new();
    let mut total = None;
    loop {
        let response = fetch(all.len())?;
        if response
            .get("warnings")
            .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()) && !v.is_null())
        {
            return Err("Anytype reported incomplete or unresolved data. Refresh results when the space is fully loaded.".into());
        }
        let data = response["data"]
            .as_array()
            .ok_or("Anytype returned a page without data.")?;
        let more = response
            .get("has_more")
            .or_else(|| response.pointer("/pagination/has_more"))
            .and_then(Value::as_bool)
            .ok_or("Anytype returned a page without completeness metadata.")?;
        let current = response
            .get("total")
            .or_else(|| response.pointer("/pagination/total"))
            .and_then(Value::as_u64);
        if let (Some(before), Some(now)) = (total, current) {
            if before != now {
                return Err(
                    "The board changed during pagination. Refresh results to retry.".into(),
                );
            }
        }
        total = current.or(total);
        if more && data.is_empty() {
            return Err("Anytype returned an incomplete page. Refresh to retry.".into());
        }
        all.extend(data.iter().cloned());
        if all.len() > 1_000_000 {
            return Err("Anytype returned too many results.".into());
        }
        if !more {
            if total.is_some_and(|n| n != all.len() as u64) {
                return Err("Anytype returned an incomplete result set.".into());
            }
            return Ok(all);
        }
    }
}

struct Grouping {
    property: String,
    format: String,
    options: BTreeMap<String, String>,
    categories: BTreeMap<String, String>,
}
impl Grouping {
    fn category(&self, value: &Value) -> Result<String, String> {
        if self.format == "checkbox" {
            return match value {
                Value::Null => Ok("false".into()),
                Value::Bool(value) => Ok(value.to_string()),
                _ => Err("A checkbox category contains an incompatible value.".into()),
            };
        }
        let names: Vec<&str> = match value {
            Value::Null => Vec::new(),
            Value::String(name) if self.format == "select" => vec![name],
            Value::Array(values) => values
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or("A category contains an incompatible option value.")
                })
                .collect::<Result<_, _>>()?,
            _ => return Err("A category contains an incompatible value.".into()),
        };
        if names.is_empty() {
            return Ok("empty".into());
        }
        if self.format == "select" && names.len() != 1 {
            return Err("A select category returned multiple values.".into());
        }
        let mut ids = names
            .iter()
            .map(|name| {
                self.options
                    .get(*name)
                    .cloned()
                    .ok_or("A category option changed. Reload views and refresh results.")
            })
            .collect::<Result<Vec<_>, _>>()?;
        ids.sort();
        ids.dedup();
        Ok(if self.format == "multi_select" {
            md5(&ids.join(""))
        } else {
            ids[0].clone()
        })
    }
}

// Anytype core/kanban/group_tag.go uses MD5 solely as a category identifier,
// not as a security primitive. Match its sorted, concatenated option IDs.
fn md5(input: &str) -> String {
    let mut bytes = input.as_bytes().to_vec();
    let bits = (bytes.len() as u64) * 8;
    bytes.push(0x80);
    while bytes.len() % 64 != 56 {
        bytes.push(0)
    }
    bytes.extend_from_slice(&bits.to_le_bytes());
    let mut state = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    let shifts = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
    for chunk in bytes.chunks_exact(64) {
        let mut words = [0u32; 16];
        for (i, part) in chunk.chunks_exact(4).enumerate() {
            words[i] = u32::from_le_bytes(part.try_into().unwrap());
        }
        let [mut a, mut b, mut c, mut d] = state;
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let k = (((i + 1) as f64).sin().abs() * 4294967296.0).floor() as u32;
            let next = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(k)
                    .wrapping_add(words[g])
                    .rotate_left(shifts[(i / 16) * 4 + i % 4]),
            );
            a = d;
            d = c;
            c = b;
            b = next;
        }
        for (i, value) in [a, b, c, d].iter().enumerate() {
            state[i] = state[i].wrapping_add(*value);
        }
    }
    state
        .iter()
        .flat_map(|n| n.to_le_bytes())
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_full_filtered_membership_and_preserves_exact_category_ids() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
            time::{Duration, Instant},
        };
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !done.load(Ordering::Relaxed) && Instant::now() < deadline {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    bytes.extend_from_slice(&buffer[..n]);
                    if n == 0 {
                        break;
                    }
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("Content-Length: "))
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                assert!(request.contains("Authorization: Bearer fixture-key"));
                let path = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let page = |data: Value| json!({"data":data,"has_more":false});
                let view = json!({"id":"view-a","type":"kanban","name":"Board","group_by":"status","groups":[{"id":"option-a"},{"id":"empty"}]});
                let response = if path.starts_with("/v2/spaces?") {
                    page(json!([{"id":"space","name":"Work"}]))
                } else if path.starts_with("/v2/spaces/space/properties/status/options") {
                    page(json!([{"name":"A"}]))
                } else if path.starts_with("/v2/spaces/space/properties") {
                    page(
                        json!([{"key":"estimate","name":"Estimate","format":"number"},{"key":"status","name":"Status","format":"select"}]),
                    )
                } else if path.starts_with("/v2/spaces/space/search") {
                    page(json!([{"id":"object-a","name":"Tasks","type":"query"}]))
                } else if path.starts_with("/v2/spaces/space/objects/object-a") {
                    json!({"id":"object-a","type":"query","blocks":[{"id":"board","views":[view]}]})
                } else if path.starts_with("/v2/spaces/space/objects/option-a") {
                    json!({"id":"option-a","kind":"relation_option","properties":{"name":"A"}})
                } else if path.starts_with("/v2/spaces/space/queries/object-a/views") {
                    page(json!([view]))
                } else if path.starts_with("/v2/spaces/space/queries/object-a/objects") {
                    assert!(path.contains("view=view-a"));
                    assert!(path.contains("fields=estimate%2Cstatus"));
                    if path.contains("offset=0") {
                        json!({"data":[{"id":"card-a","properties":{"estimate":3,"status":"A"}}],"has_more":true,"total":2})
                    } else {
                        json!({"data":[{"id":"card-b","properties":{"estimate":7,"status":null}}],"has_more":false,"total":2})
                    }
                } else {
                    panic!("Unexpected fixture route {path}")
                };
                let body = serde_json::to_vec(&response).unwrap();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let client = Client {
            key: crate::SecretString::new("fixture-key".into()),
            ip: "127.0.0.1".into(),
            port,
        };
        let catalog = client.discover().unwrap();
        assert_eq!(catalog.views.len(), 1);
        assert!(catalog.views[0].unsupported.is_none());
        let calculator = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "estimate".into(),
            operation: "total".into(),
        }])
        .unwrap();
        let results = client
            .calculate(&catalog.views[0].target, &calculator)
            .unwrap();
        assert_eq!(
            results
                .iter()
                .find(|r| r.category_id == "option-a")
                .unwrap()
                .value,
            Some(3.0)
        );
        assert_eq!(
            results
                .iter()
                .find(|r| r.category_id == "empty")
                .unwrap()
                .value,
            Some(7.0)
        );
        let deleted = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "deleted".into(),
            operation: "total".into(),
        }])
        .unwrap();
        assert!(client
            .calculate(&catalog.views[0].target, &deleted)
            .is_err());
        stop.store(true, Ordering::Relaxed);
        worker.join().unwrap();
    }
    #[test]
    fn paging_requires_complete_stable_responses() {
        let mut calls = Vec::new();
        let rows = complete_pages(|offset| {
            calls.push(offset);
            Ok(if offset == 0 {
                json!({"data":[1,2],"has_more":true,"total":3})
            } else {
                json!({"data":[3],"has_more":false,"total":3})
            })
        })
        .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(calls, vec![0, 2]);
        for response in [
            json!({"data":[],"has_more":true}),
            json!({"data":[],"has_more":false,"total":2}),
            json!({"data":[],"has_more":false,"warnings":[{}]}),
            json!({"data":[]}),
        ] {
            assert!(complete_pages(|_| Ok(response.clone())).is_err());
        }
    }
    #[test]
    fn exact_anytype_category_rules() {
        assert_eq!(md5("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(md5(""), "d41d8cd98f00b204e9800998ecf8427e");
        let mut grouping = Grouping {
            property: "tags".into(),
            format: "multi_select".into(),
            options: BTreeMap::from([
                ("A".into(), "option-a".into()),
                ("B".into(), "option-b".into()),
            ]),
            categories: BTreeMap::new(),
        };
        assert_eq!(
            grouping.category(&json!(["B", "A"])).unwrap(),
            md5("option-aoption-b")
        );
        assert_eq!(grouping.category(&json!([])).unwrap(), "empty");
        grouping.format = "select".into();
        assert_eq!(grouping.category(&json!("A")).unwrap(), "option-a");
        assert!(grouping.category(&json!(["A", "B"])).is_err());
        assert!(grouping.category(&json!("Missing")).is_err());
        grouping.format = "checkbox".into();
        assert_eq!(grouping.category(&Value::Null).unwrap(), "false");
        assert_eq!(grouping.category(&json!(true)).unwrap(), "true");
    }
}
