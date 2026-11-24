//! Create objects in query and collection destinations through saved space permissions.
use crate::{
    anyblob_content,
    codex::{Configuration, Permission},
    kanban_api::Client,
};
use blobui_mcp::{content::ContentRequest, Operation};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

mod query;
#[cfg(test)]
mod query_tests;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Destination {
    space_id: String,
    kind: String,
    id: String,
    name: String,
}

static OPERATIONS: Mutex<()> = Mutex::new(());
fn normalize(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn encode(s: &str) -> String {
    crate::api::encode_segment(s)
}
fn get(
    client: &Client,
    path: &str,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    check()?;
    let value = client.content_request("GET", path, None, None, None, None)?;
    if value.get("_http_error").is_some() {
        return Err(format!("Destination unavailable: {value}"));
    }
    Ok(value)
}
fn base(space: &str) -> String {
    format!("/v2/spaces/{}", encode(space))
}
fn creation_prompt(req: &ContentRequest, destination: &Destination) -> Operation {
    let mut content = req.clone();
    content.arguments["_approval_context"] = json!({
        "space": destination.space_id,
        "destination": if destination.name.trim().is_empty() { &destination.id } else { &destination.name },
        "destination_kind": destination.kind,
    });
    Operation::Content { content }
}
fn destination_path(destination: &Destination) -> String {
    format!(
        "{}/{}/{}",
        base(&destination.space_id),
        if destination.kind == "query" {
            "queries"
        } else {
            "collections"
        },
        encode(&destination.id)
    )
}
fn definition(
    client: &Client,
    destination: &Destination,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    let path = destination_path(destination);
    let doc = get(
        client,
        &format!(
            "{}/objects/{}?ids=full",
            base(&destination.space_id),
            encode(&destination.id)
        ),
        check,
    )?;
    if doc["id"] != destination.id {
        return Err("Destination ID did not match the requested destination.".into());
    }
    if destination.kind == "collection" {
        if doc["type"] != "collection" {
            return Err("Selected destination is not a collection.".into());
        }
        return Ok(json!({"document":doc}));
    }
    if !matches!(doc["type"].as_str(), Some("set" | "query")) {
        return Err("Selected destination is not a query.".into());
    }
    check()?;
    let views = client.pages("GET", &format!("{path}/views?ids=full"), None)?;
    let mut ids = std::collections::BTreeSet::new();
    for view in &views {
        let id = view["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("Query view has no ID.")?;
        if !ids.insert(id) {
            return Err("Query views changed during pagination.".into());
        }
    }
    Ok(json!({"document":doc,"views":views}))
}

// Unknown predicates fail closed. OR can prove a different supported branch.
#[cfg(test)]
fn prove(
    node: &Value,
    body: &Value,
    properties: &[Value],
    options: &Value,
    depth: usize,
) -> Result<(), String> {
    prove_with_types(node, body, properties, options, depth, &[])
}
fn prove_with_types(
    node: &Value,
    body: &Value,
    properties: &[Value],
    options: &Value,
    depth: usize,
    types: &[Value],
) -> Result<(), String> {
    if depth > 32 {
        return Err("Filter nesting exceeds the supported depth.".into());
    }
    if let Some(nodes) = node.as_array() {
        for n in nodes {
            prove_with_types(n, body, properties, options, depth + 1, types)?;
        }
        return Ok(());
    }
    if let Some(op) = node["operator"].as_str() {
        if node.as_object().is_none_or(|o| {
            o.keys()
                .any(|k| !["operator", "filters", "id"].contains(&k.as_str()))
        }) {
            return Err("Unsupported filter group fields.".into());
        }
        let nodes = node["filters"]
            .as_array()
            .filter(|a| !a.is_empty())
            .ok_or("Empty or invalid filter group.")?;
        if op == "and" {
            for n in nodes {
                prove_with_types(n, body, properties, options, depth + 1, types)?;
            }
            return Ok(());
        }
        if op == "or" {
            let mut reasons = Vec::new();
            for n in nodes {
                match prove_with_types(n, body, properties, options, depth + 1, types) {
                    Ok(()) => return Ok(()),
                    Err(e) => reasons.push(e),
                }
            }
            return Err(format!("No OR branch is provable: {}", reasons.join("; ")));
        }
        return Err(format!("Unsupported filter operator {op}."));
    }
    let key = node["property"]
        .as_str()
        .ok_or("Filter has no property key.")?;
    let prop = query::property(key, properties)?;
    let canonical = prop["key"].as_str().ok_or("Property has no API key.")?;
    let expected = query::filter_values(node, prop, options, types)?;
    if canonical == "type" {
        let ty = query::resolve_type(body["type"].as_str().ok_or("Missing body.type.")?, types)?;
        return if expected.iter().any(|v| v == &ty["id"]) {
            Ok(())
        } else {
            Err("Object type does not match the view's type filter.".into())
        };
    }
    if canonical == "name"
        && body.get("name").is_none()
        && body["markdown"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
    {
        return Err("A name filter with Markdown requires an explicit top-level name to avoid heading-derived changes.".into());
    }
    let value = query::supplied(prop, body, properties)?
        .ok_or_else(|| format!("Property {key} must be explicitly supplied exactly once."))?;
    let matches = match prop["format"].as_str() {
        Some("text" | "checkbox") => expected.iter().any(|v| v == value),
        Some("number") => {
            value.is_number()
                && expected.iter().any(|v| {
                    v == value
                        || (value
                            .as_f64()
                            .is_some_and(|n| n.abs() <= 9_007_199_254_740_991.0)
                            && v.as_f64()
                                .is_some_and(|n| n.abs() <= 9_007_199_254_740_991.0)
                            && value.as_f64() == v.as_f64())
                })
        }
        Some("select") => {
            let name = value
                .as_str()
                .ok_or_else(|| format!("Select {key} requires one option name."))?;
            let opts = options[canonical]
                .as_array()
                .ok_or("Select metadata unavailable.")?;
            opts.iter().filter(|o| o["name"] == name).count() == 1
                && expected.iter().any(|v| v == name)
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(format!(
            "Property {key} does not match the view requirement."
        ))
    }
}
fn metadata(
    client: &Client,
    destination: &Destination,
    check: &impl Fn() -> Result<(), String>,
) -> Result<(Vec<Value>, Value), String> {
    check()?;
    let properties = client.pages(
        "GET",
        &format!("{}/properties?ids=full", base(&destination.space_id)),
        None,
    )?;
    let mut options = json!({});
    for p in &properties {
        if p["format"] == "select" {
            let key = p["key"].as_str().ok_or("Property has no API key.")?;
            check()?;
            options[key] = json!(client.pages(
                "GET",
                &format!(
                    "{}/properties/{}/options?ids=full",
                    base(&destination.space_id),
                    encode(key)
                ),
                None
            )?);
        }
    }
    Ok((properties, options))
}
fn query_proof(
    client: &Client,
    destination: &Destination,
    def: &Value,
    req: &ContentRequest,
    files_enabled: bool,
    check: &impl Fn() -> Result<(), String>,
) -> Result<String, String> {
    let (response, canonical_type) = query::assess_with_type(
        client,
        destination,
        def,
        req,
        files_enabled,
        check,
    )?;
    match response {
        None => Ok(canonical_type),
        Some(response) => Err(response["message"]
            .as_str()
            .unwrap_or("Query requirements must be satisfied before creation.")
            .into()),
    }
}

fn member(
    client: &Client,
    destination: &Destination,
    id: &str,
    check: &impl Fn() -> Result<(), String>,
) -> Result<bool, String> {
    let path = destination_path(destination);
    let urls = if destination.kind == "query" {
        let def = definition(client, destination, check)?;
        def["views"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                format!(
                    "{path}/objects?ids=full&view={}",
                    encode(v["id"].as_str().unwrap())
                )
            })
            .collect::<Vec<_>>()
    } else {
        vec![format!("{path}/objects?ids=full")]
    };
    for url in urls {
        check()?;
        if client
            .pages("GET", &url, None)?
            .iter()
            .any(|row| row["id"] == id)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn record_path(req: &ContentRequest) -> Result<PathBuf, String> {
    let identity = json!([
        req.arguments["space_id"],
        req.arguments["destination"],
        req.arguments["idempotency_key"]
    ]);
    let digest = format!("{:x}", Sha256::digest(identity.to_string().as_bytes()));
    let dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("No application directory.")?
        .join("anyblob-creation");
    crate::reject_reparse_component(&dir)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot store durable creation records: {e}"))?;
    crate::reject_reparse_component(&dir)?;
    Ok(dir.join(format!("{digest}.json")))
}
// Append-only, flushed journal. A torn final line is an uncertain operation, never a retry.
fn save(path: &Path, record: &Value) -> Result<(), String> {
    crate::reject_reparse_component(path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{record}")
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Cannot persist creation outcome: {e}"))
}
fn load(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    crate::reject_reparse_component(path)?;
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if !text.ends_with('\n') {
        return Err(
            "Creation record is incomplete. Outcome unknown; do not retry automatically.".into(),
        );
    }
    text.lines()
        .last()
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|_| "Creation record is unreadable. Do not retry automatically.".into())
        })
        .transpose()
}
fn result(record: &Value, placement: &str, error: Option<String>) -> Value {
    json!({"outcome":if placement == "confirmed" {"confirmed"} else {"placement_failed"},"receipt":record["creation"]["receipt"],"object_id":record["object_id"],"creation":record["creation"],"placement":{"outcome":placement,"error":error},"idempotency_key":record["key"],"retry_automatically":false})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anyblob_content::tests::fixture;
    pub(super) fn page(rows: Value) -> Value {
        json!({"data":rows,"has_more":false})
    }
    pub(super) fn source(kind: &str, _access: &str) -> Destination {
        Destination {
            space_id: "s".into(),
            kind: kind.into(),
            id: "dest".into(),
            name: "Destination".into(),
        }
    }
    pub(super) fn configuration(access: &str) -> Configuration {
        Configuration {
            spaces: std::collections::BTreeMap::from([(
                "s".into(),
                Permission {
                    access: access.into(),
                    approval: "fullAccess".into(),
                    files_enabled: false,
                },
            )]),
            ..Default::default()
        }
    }
    pub(super) fn req(kind: &str) -> ContentRequest {
        let key = format!(
            "fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        ContentRequest {
            tool: "create_object".into(),
            arguments: json!({"space_id":"s","destination":{"kind":kind,"id":"dest"},"idempotency_key":key,"body":{"type":"page","template":"none","name":"Created"}}),
        }
    }
    pub(super) fn doc(kind: &str) -> Value {
        if kind == "query" {
            json!({"id":"dest","type":"set","query_source":{"types":["page"]}})
        } else {
            json!({"id":"dest","type":"collection","etag":"current"})
        }
    }
    fn query_reads() -> Vec<Value> {
        vec![
            doc("query"),
            page(
                json!([{"id":"first","filters":[{"property":"title","condition":"contains","value":"unsupported"}]},{"id":"later","filters":[]}]),
            ),
            page(json!([{"id":"type-page","key":"page"}])),
            page(json!([])),
        ]
    }
    fn schema() -> Value {
        json!({"schema":{"type":"object","properties":{"op":{"const":"add_items"},"items":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":1}},"required":["op","items"],"additionalProperties":false}})
    }
    pub(super) fn normal_access_responses(id: &str) -> Vec<Value> {
        let document =
            json!({"id":id,"type":"page","properties":{"name":"Created"},"etag":"object-etag"});
        vec![
            document.clone(),
            document,
            page(json!([{"key":"name","format":"text"}])),
            json!({"schema":{"type":"object","properties":{"op":{"const":"set_properties"},"set":{"type":"object"}},"required":["op","set"]}}),
            json!({"valid":true}),
            json!({"applied":true}),
        ]
    }
    pub(super) fn normal_read_and_edit(
        client: &Client,
        config: &Configuration,
        id: &str,
    ) {
        let read = ContentRequest {
            tool: "read_document".into(),
            arguments: json!({"space_id":"s","object_id":id}),
        };
        let mut read = anyblob_content::prepare(client, &read, config, &|| Ok(())).unwrap();
        let validation = anyblob_content::preflight(client, &mut read, config, &|| Ok(())).unwrap();
        assert!(validation.is_none());
        assert_eq!(
            anyblob_content::execute_validated(
                client,
                &mut read,
                &|| Ok(()),
                &|| Ok(()),
                config,
            )
            .unwrap()["id"],
            id
        );
        let edit = ContentRequest {
            tool: "patch_object".into(),
            arguments: json!({"space_id":"s","object_id":id,"etag":"object-etag","body":{"ops":[{"op":"set_properties","set":{"name":"Edited"}}]}}),
        };
        let mut edit = anyblob_content::prepare(client, &edit, config, &|| Ok(())).unwrap();
        anyblob_content::inspect(client, &mut edit, config, &|| Ok(())).unwrap();
        let validation = anyblob_content::preflight(client, &mut edit, config, &|| Ok(())).unwrap();
        assert!(validation.is_none());
        assert_eq!(
            anyblob_content::execute_validated(
                client,
                &mut edit,
                &|| Ok(()),
                &|| Ok(()),
                config,
            )
            .unwrap()["outcome"],
            "confirmed"
        );
    }

    #[test]
    fn nested_predicates_require_explicit_supported_values_and_resolve_names() {
        let props = vec![
            json!({"key":"name","name":"Name","format":"text"}),
            json!({"key":"title","name":"Title","format":"text"}),
            json!({"key":"count","format":"number"}),
            json!({"key":"done","format":"checkbox"}),
            json!({"key":"status","name":"Status","format":"select"}),
        ];
        let options = json!({"status":[{"id":"option","name":"Ready"}]});
        let body = json!({"properties":{"Title":"Match","count":1,"done":false,"Status":"Ready"}});
        let good = json!({"operator":"and","filters":[{"property":"title","condition":"equal","value":"Match"},{"operator":"or","filters":[{"property":"title","condition":"contains","value":"unknown"},{"property":"status","condition":"equal","value":"Ready"}]},{"property":"count","condition":"equal","value":1.0},{"property":"done","condition":"equal","value":false}]});
        assert!(prove(&good, &body, &props, &options, 0).is_ok());
        for bad in [
            json!({"property":"title","condition":"equal","value":"Missing"}),
            json!({"property":"missing","condition":"equal","value":"x"}),
            json!({"property":"title","condition":"not_equal","value":"x"}),
            json!({"property":"title","condition":"equal","value":"Match","negate":true}),
            json!({"operator":"and","filters":[good.clone(),{"property":"done","condition":"exists"}]}),
            json!({"operator":"or","filters":[]}),
            Value::Null,
        ] {
            assert!(prove(&bad, &body, &props, &options, 0).is_err(), "{bad}");
        }
        assert!(prove(&good, &json!({}), &props, &options, 0)
            .unwrap_err()
            .contains("explicitly"));
        assert!(prove(
            &json!({"property":"status","condition":"equal","value":"Ready"}),
            &json!({"properties":{"status":"New"}}),
            &props,
            &options,
            0
        )
        .is_err());
        assert!(prove(&json!([]), &json!({}), &props, &options, 0).is_ok());
        assert!(prove(
            &json!({"property":"name","condition":"equal","value":"Required"}),
            &json!({"properties":{"name":"Required"},"markdown":"# Other"}),
            &props,
            &options,
            0
        )
        .unwrap_err()
        .contains("heading"));
        assert!(prove(
            &json!({"property":"title","condition":"equal","value":"_filter_template_2_"}),
            &json!({"properties":{"title":"_filter_template_2_"}}),
            &props,
            &options,
            0
        )
        .unwrap_err()
        .contains("dynamic"));
        let n = json!({"property":"count","condition":"equal","value":9007199254740993u64});
        assert!(prove(
            &n,
            &json!({"properties":{"count":9007199254740992u64}}),
            &props,
            &options,
            0
        )
        .is_err());
    }
    #[test]
    fn query_creation_matches_view_membership_before_global_access_applies() {
        let mut responses = query_reads();
        responses.extend(query_reads()); // dry run authorization
        responses.push(json!({"valid":true}));
        responses.extend(query_reads()); // commit authorization
        responses.push(json!({"id":"new-object"}));
        responses.extend(vec![
            doc("query"),
            page(json!([{"id":"later"}])),
            page(json!([{"id":"new-object"}])),
        ]);
        responses.extend(normal_access_responses("new-object"));
        let (client, capture, server) = fixture(responses);
        let p = configuration("createDelete");
        let result = dispatch_global(&client, &p,
            &req("query"),
            &|| Ok(()),
            &|_, _| panic!("full access"),
        )
        .unwrap();
        assert_eq!(result["creation"]["outcome"], "confirmed");
        assert_eq!(result["placement"]["outcome"], "confirmed");
        let edit = ContentRequest {
            tool: "patch_object".into(),
            arguments: json!({"space_id":"s","object_id":"new-object"}),
        };
        assert!(anyblob_content::permission(&p, &edit, false).is_ok());
        let unrelated = ContentRequest {
            tool: "read_document".into(),
            arguments: json!({"space_id":"s","object_id":"unrelated"}),
        };
        assert!(anyblob_content::permission(&p, &unrelated, false).is_ok());
        normal_read_and_edit(&client, &p, "new-object");
        server.join().unwrap();
        let capture = capture.lock().unwrap();
        assert_eq!(
            capture
                .iter()
                .filter(|r| String::from_utf8_lossy(r).starts_with("POST "))
                .count(),
            2
        );
    }
    #[test]
    fn query_rejects_templates_missing_values_and_mismatched_types_before_writes() {
        for case in ["template", "type", "missing", "unknown"] {
            let mut request = req("query");
            if case == "template" {
                request.arguments["body"]["template"] = json!("default");
            }
            if case == "type" {
                request.arguments["body"]["type"] = json!("task");
            }
            let mut responses = vec![
                doc("query"),
                page(
                    json!([{"id":"all","filters":[{"property":"title","condition":if case=="unknown" {"contains"} else {"equal"},"value":"required"}]}]),
                ),
            ];
            if case != "template" {
                responses.push(page(
                    json!([{"id":"type-page","key":"page"},{"id":"type-task","key":"task"}]),
                ));
            }
            if case != "template" {
                responses.push(page(json!([{"key":"title","format":"text"}])));
            }
            let (client, capture, server) = fixture(responses);
            let response = dispatch_global(&client, &configuration("createDelete"),
                &request,
                &|| Ok(()),
                &|_, _| panic!("no approval before proof"),
            );
            if case == "template" {
                assert!(response.is_err());
            } else {
                assert_eq!(
                    response.unwrap()["outcome"],
                    if case == "unknown" {
                        "unsupported_query"
                    } else {
                        "needs_input"
                    }
                );
            }
            server.join().unwrap();
            assert!(capture
                .lock()
                .unwrap()
                .iter()
                .all(|r| String::from_utf8_lossy(r).starts_with("GET ")));
        }
    }
    #[test]
    fn collection_creation_places_only_new_id_then_normal_access_applies() {
        let mut responses = vec![
            doc("collection"),
            json!({"valid":true}),
            doc("collection"),
            json!({"id":"created"}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"applied":true}),
            page(json!([{"id":"created"}])),
        ];
        responses.extend(normal_access_responses("created"));
        let (client, capture, server) = fixture(responses);
        let p = configuration("createDelete");
        let result = dispatch_global(&client, &p,
            &req("collection"),
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(result["object_id"], "created");
        assert_eq!(result["placement"]["outcome"], "confirmed");
        assert!(anyblob_content::permission(
            &p,
            &ContentRequest {
                tool: "patch_object".into(),
                arguments: json!({"space_id":"s","object_id":"created"})
            },
            false
        )
        .is_ok());
        assert!(anyblob_content::permission(
            &p,
            &ContentRequest {
                tool: "create_property".into(),
                arguments: json!({"space_id":"s"})
            },
            false
        )
        .is_ok());
        normal_read_and_edit(&client, &p, "created");
        server.join().unwrap();
        let capture = capture.lock().unwrap();
        let patches = capture
            .iter()
            .map(|r| String::from_utf8_lossy(r))
            .filter(|r| r.starts_with("PATCH /v2/spaces/s/objects/dest"))
            .collect::<Vec<_>>();
        assert_eq!(patches.len(), 2);
        for patch in patches {
            assert!(patch.contains("If-Match: current"));
            assert!(patch.contains("\"items\":[\"created\"]"));
        }
    }
    #[test]
    fn collection_destination_changes_before_commit_prevent_creation() {
        let request = req("collection");
        let mut changed = doc("collection");
        changed["etag"] = json!("changed");
        let (client, capture, server) = fixture(vec![
            doc("collection"),
            json!({"valid":true}),
            changed,
        ]);
        let error = dispatch_global(&client, &configuration("createDelete"),
            &request,
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap_err();
        assert!(error.contains("Creation destination changed"), "{error}");
        server.join().unwrap();
        let requests = capture.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(String::from_utf8_lossy(&requests[0]).starts_with("GET /v2/spaces/s/objects/dest?ids=full "));
        assert!(String::from_utf8_lossy(&requests[1]).starts_with("POST /v2/spaces/s/objects?dry_run=true "));
        assert!(String::from_utf8_lossy(&requests[2]).starts_with("GET /v2/spaces/s/objects/dest?ids=full "));
        assert!(requests.iter().all(|r| !String::from_utf8_lossy(r).starts_with("POST /v2/spaces/s/objects ")));
        assert!(!record_path(&request).unwrap().exists());
    }
    #[test]
    fn collection_conflict_recovers_without_creating_twice_and_binds_payload() {
        let request = req("collection");
        let p = configuration("createDelete");
        let (client, capture, server) = fixture(vec![
            doc("collection"),
            json!({"valid":true}),
            doc("collection"),
            json!({"id":"kept"}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"fixture_status":409,"message":"ETag conflict"}),
        ]);
        let result = dispatch_global(&client, &p,
            &request,
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(result["creation"]["outcome"], "confirmed");
        assert_eq!(result["placement"]["outcome"], "failed");
        assert_eq!(result["object_id"], "kept");
        server.join().unwrap();
        assert_eq!(
            capture
                .lock()
                .unwrap()
                .iter()
                .filter(|r| String::from_utf8_lossy(r).starts_with("POST "))
                .count(),
            2
        );
        let (client, capture, server) = fixture(vec![
            doc("collection"),
            json!({"valid":true}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"applied":true}),
            page(json!([{"id":"kept"}])),
        ]);
        let result = dispatch_global(&client, &p,
            &request,
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(result["placement"]["outcome"], "confirmed");
        server.join().unwrap();
        assert!(capture
            .lock()
            .unwrap()
            .iter()
            .all(|r| !String::from_utf8_lossy(r).starts_with("POST /v2/spaces/s/objects ")));
        let mut changed = request.clone();
        changed.arguments["body"]["name"] = json!("Different");
        let (client, _, server) = fixture(vec![doc("collection"), json!({"valid":true})]);
        assert!(dispatch_global(&client, &p,
            &changed,
            &|| Ok(()),
            &|_, _| Ok(())
        )
        .unwrap_err()
        .contains("different payload"));
        server.join().unwrap();
        assert!(dispatch_global(
            &client,
            &configuration("read"),
            &request,
            &|| Ok(()),
            &|_, _| panic!("read access must be denied")
        )
        .unwrap_err()
        .contains("Create-Delete"));
    }
    #[test]
    fn uncertain_collection_write_is_not_retried_after_restart() {
        for uncertain_create in [true, false] {
            let request = req("collection");
            let p = configuration("createDelete");
            let mut responses = vec![doc("collection"), json!({"valid":true}), doc("collection")];
            if uncertain_create {
                responses.push(json!("disconnect"));
            } else {
                responses.extend(vec![
                    json!({"id":"kept"}),
                    page(json!([])),
                    doc("collection"),
                    schema(),
                    json!({"valid":true}),
                    json!("disconnect"),
                ]);
            }
            let (client, _, server) = fixture(responses);
            let result = dispatch_global(&client, &p,
                &request,
                &|| Ok(()),
                &|_, _| Ok(()),
            )
            .unwrap();
            assert!(result["outcome"] == "unknown" || result["placement"]["outcome"] == "unknown");
            server.join().unwrap();
            let (client, capture, server) = fixture(vec![doc("collection"), json!({"valid":true})]);
            let result = dispatch_global(&client, &p,
                &request,
                &|| Ok(()),
                &|_, _| Ok(()),
            )
            .unwrap();
            assert_eq!(result["placement"]["outcome"], "unknown");
            server.join().unwrap();
            assert_eq!(capture.lock().unwrap().len(), 2);
        }
    }
    #[test]
    fn global_permission_approval_names_the_destination_and_space() {
        for kind in ["query", "collection"] {
            let mut p = configuration("createDelete");
            p.spaces.get_mut("s").unwrap().approval = "alwaysAsk".into();
            let responses = if kind == "query" {
                let mut responses = query_reads();
                responses.extend(query_reads());
                responses.push(json!({"valid":true}));
                responses
            } else {
                vec![doc(kind),json!({"valid":true})]
            };
            let (client, capture, server) = fixture(responses);
            let error = dispatch_global(&client, &p, &req(kind), &|| Ok(()), &|op, _| {
                assert_eq!(blobui_mcp::approval::describe(op), format!(
                    "An agent wants to create object 'Created' in {kind} 'dest' in s."));
                if kind=="collection" {assert!(String::from_utf8_lossy(&capture.lock().unwrap()[1]).starts_with("POST /v2/spaces/s/objects?dry_run=true "));}
                Err("denied".into())
            }).unwrap_err();
            assert!(error.contains("denied"));
            server.join().unwrap();
            if kind=="query" {
                let requests=capture.lock().unwrap();
                assert_eq!(requests.len(),9);
                assert!(requests[..8].iter().all(|r| String::from_utf8_lossy(r).starts_with("GET ")));
                assert!(String::from_utf8_lossy(&requests[8]).starts_with("POST /v2/spaces/s/objects?dry_run=true "));
            } else {
                let requests=capture.lock().unwrap();
                assert_eq!(requests.len(),2);
                assert!(String::from_utf8_lossy(&requests[1]).starts_with("POST /v2/spaces/s/objects?dry_run=true "));
            }
        }
    }
    #[test]
    fn global_creation_targets_use_space_create_delete_and_find_empty_sources() {
        let config = Configuration { spaces: std::collections::BTreeMap::from([("s".into(), crate::codex::Permission {
            access:"createDelete".into(), approval:"fullAccess".into(), files_enabled:false,
        })]), ..Default::default() };
        let request = ContentRequest { tool:"list_creation_targets".into(), arguments:json!({"space_id":"s"}) };
        let (client, capture, server) = fixture(vec![
            page(json!([{"id":"empty","type":"collection","name":"Empty"},{"id":"other","type":"page","name":"Page"}])) ,
            json!({"id":"empty","type":"collection","etag":"empty-etag"}),
        ]);
        let result = dispatch_global(&client, &config, &request, &|| Ok(()), &|_, _| panic!("full access does not prompt")).unwrap();
        assert_eq!(result, json!({"targets":[{"kind":"collection","id":"empty","name":"Empty","requirements":null}]}));
        server.join().unwrap();
        let requests = capture.lock().unwrap();
        assert!(requests[0].starts_with(b"POST /v2/spaces/s/search?ids=full&limit=100&offset=0 "));
        assert!(requests[1].starts_with(b"GET /v2/spaces/s/objects/empty?ids=full "));
    }
    #[test]
    fn global_collection_creation_uses_global_access_and_rejects_read_only_access() {
        let request = req("collection");
        let readonly = Configuration { spaces: std::collections::BTreeMap::from([("s".into(), crate::codex::Permission {
            access:"readWrite".into(), approval:"fullAccess".into(), files_enabled:false,
        })]), ..Default::default() };
        let client = Client::with_connection("fixture", "127.0.0.1", 1);
        assert!(dispatch_global(&client, &readonly, &request, &|| Ok(()), &|_, _| panic!("denied access must not prompt"))
            .unwrap_err().contains("Create-Delete"));

        let config = Configuration { spaces: std::collections::BTreeMap::from([("s".into(), crate::codex::Permission {
            access:"createDelete".into(), approval:"fullAccess".into(), files_enabled:false,
        })]), ..Default::default() };
        let responses = vec![
            doc("collection"),
            json!({"valid":true}),
            doc("collection"),
            json!({"id":"created"}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"applied":true}),
            page(json!([{"id":"created"}])),
        ];
        let (client, capture, server) = fixture(responses);
        let result = dispatch_global(&client, &config, &request, &|| Ok(()), &|_, _| panic!("full access does not prompt")).unwrap();
        assert_eq!(result["object_id"], "created");
        assert_eq!(result["placement"]["outcome"], "confirmed");
        server.join().unwrap();
        assert!(capture.lock().unwrap().iter().any(|row| String::from_utf8_lossy(row).starts_with("POST /v2/spaces/s/objects?dry_run=true ")));
    }
    #[test]
    fn approval_denial_cancellation_files_and_schema_payloads_do_not_write() {
        let request = req("collection");
        let mut p = configuration("createDelete");
        p.spaces.get_mut("s").unwrap().approval = "approveForMe".into();
        let (client, capture, server) = fixture(vec![doc("collection"),json!({"valid":true})]);
        assert!(
            dispatch_global(&client, &p, &request, &|| Ok(()), &|_, _| Err(
                "denied".into()
            ))
            .unwrap_err()
            .contains("denied")
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
        let client = Client::with_connection("fixture", "127.0.0.1", 1);
        assert!(dispatch_global(&client, &p,
            &request,
            &|| Err("cancelled".into()),
            &|_, _| Ok(())
        )
        .unwrap_err()
        .contains("cancelled"));
        let mut files = request.clone();
        files.arguments["body"]["markdown"] = json!("![image](x)");
        assert!(
            dispatch_global(&client, &p, &files, &|| Ok(()), &|_, _| Ok(()))
                .unwrap_err()
                .contains("permissions")
        );
        for kind in ["property", "object_type", "template"] {
            let mut schema = request.clone();
            schema.arguments["body"]["kind"] = json!(kind);
            assert!(
                dispatch_global(&client, &p, &schema, &|| Ok(()), &|_, _| Ok(()))
                    .unwrap_err()
                    .contains("schemas")
            );
        }
    }
    #[test]
    fn invalid_destination_create_fails_before_approval_or_creation() {
        let mut p=configuration("createDelete");
        p.spaces.get_mut("s").unwrap().approval="alwaysAsk".into();
        let (client,capture,server)=fixture(vec![
            doc("collection"),
            json!({"fixture_status":400,"code":"validation_failed","message":"Request validation failed","issues":[]}),
        ]);
        let error=dispatch_global(&client, &p,&req("collection"),&||Ok(()),&|_,_|panic!("Invalid destination creates must fail before approval")).unwrap_err();
        assert!(error.contains("Cannot create the object"),"{error}");
        assert!(error.contains("validation_failed"),"{error}");
        server.join().unwrap();
        let requests=capture.lock().unwrap();
        assert_eq!(requests.len(),2);
        assert!(String::from_utf8_lossy(&requests[0]).starts_with("GET /v2/spaces/s/objects/dest?ids=full "));
        assert!(String::from_utf8_lossy(&requests[1]).starts_with("POST /v2/spaces/s/objects?dry_run=true "));
        assert!(requests.iter().all(|r| !String::from_utf8_lossy(r).starts_with("POST /v2/spaces/s/objects ")));
    }
    #[test]
    fn destination_missing_option_error_names_the_value_without_recommending_a_rejected_flag() {
        let mut request = req("collection");
        request.arguments["body"]["properties"] = json!({"status":"Todo"});
        let mut p = configuration("createDelete");
        p.spaces.get_mut("s").unwrap().approval = "alwaysAsk".into();
        let (client, capture, server) = fixture(vec![
            doc("collection"),
            page(json!([{"key":"status","name":"Status","format":"select"}])),
            json!({
                "fixture_status":400,
                "code":"validation_failed",
                "message":"Request validation failed",
                "issues":[{"path":"body.properties.status","message":"Invalid option"}]
            }),
        ]);
        let error = dispatch_global(&client, &p,
            &request,
            &|| Ok(()),
            &|_, _| panic!("Invalid destination options must fail before approval"),
        )
        .unwrap_err();
        assert!(error.contains("status") && error.contains("\"Todo\""), "{error}");
        assert!(error.contains("Select options cannot be created as destination objects."), "{error}");
        assert!(!error.contains("create_missing_options=true"), "{error}");
        server.join().unwrap();
        let requests = capture.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(String::from_utf8_lossy(&requests[0]).starts_with("GET /v2/spaces/s/objects/dest?ids=full "));
        assert!(String::from_utf8_lossy(&requests[1]).starts_with("GET /v2/spaces/s/properties?ids=full&limit=100&offset=0 "));
        assert!(String::from_utf8_lossy(&requests[2]).starts_with("POST /v2/spaces/s/objects?dry_run=true "));
        assert!(requests.iter().all(|r| {
            let request = String::from_utf8_lossy(r);
            !request.starts_with("GET /v2/spaces/s/properties/status/options")
                && !request.starts_with("POST /v2/spaces/s/objects ")
        }));
    }
    #[test]
    fn concurrent_query_changes_reject_before_commit_or_return_retained_receipt() {
        let mut responses = query_reads();
        responses.extend(vec![doc("query"), page(json!([{"id":"changed"}]))]);
        let (client, capture, server) = fixture(responses);
        let request = req("query");
        assert!(dispatch_global(&client, &configuration("createDelete"),
            &request,
            &|| Ok(()),
            &|_, _| Ok(())
        )
        .unwrap_err()
        .contains("changed"));
        server.join().unwrap();
        assert!(capture
            .lock()
            .unwrap()
            .iter()
            .all(|r| String::from_utf8_lossy(r).starts_with("GET ")));
        assert!(!record_path(&request).unwrap().exists());

        let mut responses = query_reads();
        responses.extend(query_reads());
        responses.push(json!({"valid":true}));
        responses.extend(query_reads());
        responses.push(json!({"id":"retained"}));
        responses.extend(vec![
            doc("query"),
            page(json!([{"id":"changed"}])),
            page(json!([])),
        ]);
        let (client, capture, server) = fixture(responses);
        let result = dispatch_global(&client, &configuration("createDelete"),
            &req("query"),
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(result["object_id"], "retained");
        assert_eq!(result["creation"]["outcome"], "confirmed");
        assert_eq!(result["placement"]["outcome"], "failed");
        server.join().unwrap();
        assert!(capture
            .lock()
            .unwrap()
            .iter()
            .all(|r| !String::from_utf8_lossy(r).starts_with("DELETE ")));
    }
    #[test]
    fn collection_dry_run_never_places_or_resumes_and_custom_files_are_checked() {
        let p = configuration("createDelete");
        let mut request = req("collection");
        request.arguments["dry_run"] = json!(true);
        let (client, capture, server) = fixture(vec![doc("collection"), json!({"valid":true})]);
        assert_eq!(
            dispatch_global(&client, &p, &request, &|| Ok(()), &|_, _| Ok(())).unwrap()["outcome"],
            "validated"
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
        assert!(!record_path(&request).unwrap().exists());
        let path = record_path(&request).unwrap();
        let mut payload = request.arguments.clone();
        payload.as_object_mut().unwrap().remove("dry_run");
        save(&path,&json!({"fingerprint":format!("{:x}",Sha256::digest(payload.to_string().as_bytes())),"state":"created","object_id":"earlier","creation":{"outcome":"confirmed"}})).unwrap();
        let (client, capture, server) = fixture(vec![doc("collection")]);
        assert_eq!(
            dispatch_global(&client, &p, &request, &|| Ok(()), &|_, _| Ok(())).unwrap()
                ["placement"]["outcome"],
            "not_dispatched"
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 1);
        let mut files = req("collection");
        files.arguments["body"]["properties"] = json!({"custom":"file-id"});
        let (client, capture, server) = fixture(vec![
            doc("collection"),
            page(json!([{"key":"custom","format":"files"}])),
        ]);
        assert!(
            dispatch_global(&client, &p, &files, &|| Ok(()), &|_, _| Ok(()))
                .unwrap_err()
                .contains("permissions")
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
    }
    #[test]
    fn completed_duplicate_and_revoked_recovery_never_create_another_object() {
        let request = req("collection");
        let p = configuration("createDelete");
        let (client, _, server) = fixture(vec![
            doc("collection"),
            json!({"valid":true}),
            doc("collection"),
            json!({"id":"created"}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"applied":true}),
            page(json!([{"id":"created"}])),
        ]);
        dispatch_global(&client, &p, &request, &|| Ok(()), &|_, _| {
            Ok(())
        })
        .unwrap();
        server.join().unwrap();
        let (client, capture, server) =
            fixture(vec![doc("collection"), json!({"valid":true}), page(json!([{"id":"created"}]))]);
        assert_eq!(
            dispatch_global(&client, &p, &request, &|| Ok(()), &|_, _| Ok(
                ()
            ))
            .unwrap()["placement"]["outcome"],
            "confirmed"
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 3);
        let client = Client::with_connection("fixture", "127.0.0.1", 1);
        for access in ["none", "read", "readWrite"] {
            assert!(dispatch_global(
                &client,
                &configuration(access),
                &request,
                &|| Ok(()),
                &|_, _| panic!("revoked")
            )
            .unwrap_err()
            .contains("Create-Delete"));
        }
    }
    #[test]
    fn torn_journal_tail_blocks_recovery() {
        let request = req("collection");
        let path = record_path(&request).unwrap();
        fs::write(&path, "{\"state\":\"placing\"").unwrap();
        assert!(load(&path).unwrap_err().contains("incomplete"));
    }
}

fn create_at_destination(
    client: &Client,
    config: &Configuration,
    destination: &Destination,
    permission: &Permission,
    req: &ContentRequest,
    check: &impl Fn() -> Result<(), String>,
    approve: &impl Fn(&Operation, &dyn Fn() -> Result<(), String>) -> Result<(), String>,
) -> Result<Value, String> {
    req.validate()?;
    check()?;
    if req.arguments["space_id"].as_str() != Some(destination.space_id.as_str())
        || req.arguments["destination"]["kind"].as_str() != Some(destination.kind.as_str())
        || req.arguments["destination"]["id"].as_str() != Some(destination.id.as_str())
    {
        return Err("Creation destination does not match the request.".into());
    }
    let body = &req.arguments["body"];
    if body["kind"].as_str().is_some_and(|kind| kind != "object")
        || [
            "query_source",
            "type_settings",
            "property_settings",
            "template_for",
            "collection_items",
            "items",
        ]
        .iter()
        .any(|key| body.get(*key).is_some())
        || matches!(
            body["type"].as_str(),
            Some(
                "objectType"
                    | "object_type"
                    | "property"
                    | "relation"
                    | "set"
                    | "query"
                    | "collection"
                    | "template"
            )
        )
    {
        return Err("Destination creation cannot create schemas, source definitions or collection membership payloads.".into());
    }
    let mut prepared = anyblob_content::prepare(client, req, config, check)?;
    let destination_definition = definition(client, destination, check)?;
    if destination.kind == "query" {
        let (response, canonical_type) =
            query::assess_with_type(client, destination, &destination_definition, req, permission.files_enabled, check)?;
        prepared.request.arguments["body"]["type"] = json!(canonical_type);
        prepared.files |= anyblob_content::needs_files(&prepared.request.arguments["body"]);
        anyblob_content::permission(config, &prepared.request, prepared.files)?;
        if let Some(response) = response {
            // A follow-up can reuse a key only if no earlier creation bound it.
            let path = record_path(req)?;
            if let Some(record) = load(&path)? {
                let mut payload = req.arguments.clone();
                payload.as_object_mut().unwrap().remove("dry_run");
                let fingerprint = format!("{:x}", Sha256::digest(payload.to_string().as_bytes()));
                if record["fingerprint"] != fingerprint {
                    return Err("Idempotency key was already used with a different payload.".into());
                }
                return Ok(result(&record, "unknown", Some("An earlier creation attempt exists. Requirements cannot authorize a new payload or an automatic retry.".into())));
            }
            if permission.approval == "alwaysAsk" && req.arguments["dry_run"] != true {
                approve(
                    &creation_prompt(req, destination),
                    check,
                )?;
            }
            check()?;
            return Ok(response);
        }
    }
    anyblob_content::inspect(client, &mut prepared, config, check)?;
    if req.arguments["dry_run"] == true {
        let path = record_path(req)?;
        let mut payload = req.arguments.clone();
        payload.as_object_mut().unwrap().remove("dry_run");
        let fingerprint = format!("{:x}", Sha256::digest(payload.to_string().as_bytes()));
        let _lock = OPERATIONS
            .lock()
            .map_err(|_| "Creation operation lock unavailable.")?;
        check()?;
        if let Some(record) = load(&path)? {
            if record["fingerprint"] != fingerprint {
                return Err("Idempotency key was already used with a different payload.".into());
            }
            return Ok(json!({"outcome":"not_dispatched","dry_run":true,"creation":record["creation"],"placement":{"outcome":"not_dispatched"},"idempotency_key":req.arguments["idempotency_key"]}));
        }
    }
    let needs_confirmation = req.requires_confirmation(&permission.approval);
    let dispatch_type = prepared.request.arguments["body"]["type"].clone();
    let fresh_state_check = || -> Result<(), String> {
        check()?;
        let current = definition(client, destination, check)?;
        if current != destination_definition {
            return Err("Creation destination changed before creation. Retry with current requirements.".into());
        }
        if destination.kind == "query"
            && query_proof(client, destination, &current, req, permission.files_enabled, check)? != dispatch_type
        {
            return Err("Type identity changed before creation. Retry with current metadata.".into());
        }
        Ok(())
    };
    let validation = if destination.kind == "query" {
        anyblob_content::preflight(client, &mut prepared, config, &fresh_state_check)?
    } else {
        anyblob_content::preflight(client, &mut prepared, config, check)?
    };
    if needs_confirmation {
        approve(
            &creation_prompt(req, destination),
            check,
        )?;
        check()?;
        let current = definition(client, destination, check)?;
        if current != destination_definition {
            return Err("Creation destination changed before creation. Retry with current requirements.".into());
        }
        if destination.kind == "query" {
            let (response, canonical_type) =
                query::assess_with_type(client, destination, &current, req, permission.files_enabled, check)?;
            if let Some(response) = response {
                return Ok(response);
            }
            if prepared.request.arguments["body"]["type"] != canonical_type {
                return Err("Type identity changed before creation. Retry with current metadata.".into());
            }
        }
        anyblob_content::inspect(client, &mut prepared, config, check)?;
        if req.arguments["dry_run"] != true {
            anyblob_content::preflight(client, &mut prepared, config, &fresh_state_check)?;
        }
    }
    if let Some(result) = validation {
        check()?;
        return Ok(result);
    }
    // Serialize recovery and new operations across threads. The bridge is a single instance.
    let _lock = OPERATIONS
        .lock()
        .map_err(|_| "Creation operation lock unavailable.")?;
    check()?;
    let path = record_path(req)?;
    let mut payload = req.arguments.clone();
    payload.as_object_mut().unwrap().remove("dry_run");
    let fingerprint = format!("{:x}", Sha256::digest(payload.to_string().as_bytes()));
    let old = load(&path)?;
    if old
        .as_ref()
        .is_some_and(|r| r["fingerprint"] != fingerprint)
    {
        return Err("Idempotency key was already used with a different payload.".into());
    }
    if req.arguments["dry_run"] == true && old.is_some() {
        return Ok(
            json!({"outcome":"not_dispatched","dry_run":true,"creation":old.as_ref().unwrap()["creation"],"placement":{"outcome":"not_dispatched"},"idempotency_key":req.arguments["idempotency_key"]}),
        );
    }
    let mut record = old.unwrap_or_else(
        || json!({"fingerprint":fingerprint,"key":req.arguments["idempotency_key"],"state":"new"}),
    );
    if matches!(
        record["state"].as_str(),
        Some("creating" | "placing" | "unknown")
    ) {
        return Ok(result(
            &record,
            "unknown",
            Some(
                "A previous write outcome is uncertain. No automatic retry was dispatched.".into(),
            ),
        ));
    }
    if record["state"] == "new" {
        let mut pending = record.clone();
        pending["state"] = json!("creating");
        let before_commit = || {
            fresh_state_check()?;
            save(&path, &pending)
        };
        let creation = anyblob_content::execute_validated(
            client,
            &mut prepared,
            check,
            &before_commit,
            config,
        );
        let creation = match creation {
            Ok(c) => c,
            Err(e) => {
                if load(&path)?.is_some_and(|r| r["state"] == "creating") {
                    return Ok(result(&record, "unknown", Some(e)));
                }
                return Err(e);
            }
        };
        if req.arguments["dry_run"] == true {
            return Ok(creation);
        }
        record["creation"] = creation.clone();
        if creation["outcome"] != "confirmed" {
            record["state"] = json!(if creation["outcome"] == "rejected" {
                "rejected"
            } else {
                "unknown"
            });
            save(&path, &record)?;
            return Ok(creation);
        }
        let id = creation["receipt"]["id"]
            .as_str()
            .filter(|id| !id.is_empty());
        if let Some(id) = id {
            record["object_id"] = json!(id);
            record["state"] = json!("created");
        } else {
            record["state"] = json!("unknown");
        }
        if let Err(e) = save(&path, &record) {
            return Ok(result(&record, "unknown", Some(e)));
        }
    }
    if record["state"] == "rejected" {
        return Ok(record["creation"].clone());
    }
    let Some(id) = record["object_id"].as_str().map(String::from) else {
        return Ok(result(
            &record,
            "unknown",
            Some("Creation receipt did not contain a confirmed object ID.".into()),
        ));
    };
    let placement = (|| -> Result<(), String> {
        check()?;
        if member(client, destination, &id, check)? {
            return Ok(());
        }
        if destination.kind == "query" {
            return Err("Created object does not belong to a current query view. It was retained; creation did not grant additional permissions.".into());
        }
        let current = self::definition(client, destination, check)?;
        let etag = current["document"]["etag"]
            .as_str()
            .ok_or("Collection omitted its current ETag.")?;
        let op = json!({"op":"add_items","items":[id]});
        let schema = get(client, "/v2/schemas/ops/add_items", check)?;
        if !schema["schema"].is_object() {
            return Err("Connected Anytype omitted the add_items schema.".into());
        }
        blobui_mcp::content::validate_schema(&schema["schema"], &op, "add_items")?;
        let body = json!({"ops":[op]});
        let api_path = format!("{}/objects/{}", base(&destination.space_id), encode(&destination.id));
        check()?;
        let dry = client.content_request(
            "PATCH",
            &format!("{api_path}?dry_run=true"),
            Some(&body),
            Some(etag),
            None,
            None,
        )?;
        if dry.get("_http_error").is_some() {
            return Err(format!("Collection placement refused: {dry}"));
        }
        check()?;
        record["state"] = json!("placing");
        save(&path, &record)?;
        check()?;
        let key = format!(
            "{:x}",
            Sha256::digest(format!("placement:{}:{}:{etag}", destination.space_id, req.arguments).as_bytes())
        );
        let placed = client.content_request(
            "PATCH",
            &api_path,
            Some(&body),
            Some(etag),
            Some(&key),
            None,
        );
        match placed {
            Ok(v) if v.get("_http_error").is_some() => {
                record["state"] = json!("created");
                return Err(format!("Collection placement refused: {v}"));
            }
            Ok(_) => record["state"] = json!("created"),
            Err(e) => return Err(e),
        }
        if !member(client, destination, &id, check)? {
            return Err(
                "Collection write was confirmed but membership could not be verified.".into(),
            );
        }
        Ok(())
    })();
    let (outcome, error) = match placement {
        Ok(()) => {
            record["state"] = json!("complete");
            ("confirmed", None)
        }
        Err(e) => (
            if record["state"] == "placing" {
                "unknown"
            } else {
                "failed"
            },
            Some(e),
        ),
    };
    let answer = result(&record, outcome, error);
    if let Err(e) = save(&path, &record) {
        return Ok(result(&record, "unknown", Some(e)));
    }
    Ok(answer)
}

pub fn dispatch_global(
    client: &Client,
    config: &Configuration,
    req: &ContentRequest,
    check: &impl Fn() -> Result<(), String>,
    approve: &impl Fn(&Operation, &dyn Fn() -> Result<(), String>) -> Result<(), String>,
) -> Result<Value, String> {
    req.validate()?;
    if !matches!(req.tool.as_str(), "create_object" | "list_creation_targets") {
        return Err("Global creation destinations only support create_object and list_creation_targets.".into());
    }
    let space_id = req.arguments["space_id"].as_str().ok_or("Missing space ID.")?;
    let permission = config.spaces.get(space_id)
        .filter(|permission| permission.permits("createDelete"))
        .cloned()
        .ok_or("Access denied by saved BlobUI space permissions. Save Create-Delete access first.")?;
    check()?;

    if req.tool == "list_creation_targets" {
        let rows = client.pages("POST", &format!("{}/search?ids=full", base(space_id)), Some(&json!({})))?;
        let mut seen = std::collections::BTreeSet::new();
        let mut targets = Vec::new();
        for row in rows {
            check()?;
            let kind = match row["type"].as_str() {
                Some("collection") => "collection",
                Some("query" | "set") => "query",
                _ => continue,
            };
            let id = row["id"].as_str().filter(|id| !id.is_empty())
                .ok_or("Anytype returned a creation destination without an ID.")?;
            if !seen.insert((kind.to_string(), id.to_string())) { continue; }
            let destination = Destination {
                space_id: space_id.into(),
                kind: kind.into(),
                id: id.into(),
                name: row["name"].as_str().unwrap_or("").into(),
            };
            let def = definition(client, &destination, check)?;
            let requirements = if destination.kind == "query" {
                query::discovery(client, &destination, &def, check)?
            } else {
                Value::Null
            };
            targets.push(json!({"kind":destination.kind,"id":destination.id,"name":destination.name,"requirements":requirements}));
        }
        check()?;
        return Ok(json!({"targets":targets}));
    }

    let kind = req.arguments.pointer("/destination/kind").and_then(Value::as_str)
        .ok_or("Missing destination kind.")?;
    let id = req.arguments.pointer("/destination/id").and_then(Value::as_str)
        .filter(|id| !id.is_empty()).ok_or("Missing destination ID.")?;
    if !matches!(kind, "query" | "collection") {
        return Err("Creation destination must be a query or collection.".into());
    }
    let destination = Destination {
        space_id: space_id.into(),
        kind: kind.into(),
        id: id.into(),
        name: id.into(),
    };
    create_at_destination(client, config, &destination, &permission, req, check, approve)
}
