//! Explicit source creation. These grants never enter ordinary object authorization.
use crate::{
    anyblob_content,
    codex::{Configuration, ProjectConfiguration, SourcePermission},
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
fn creation_prompt(req: &ContentRequest, source: &SourcePermission) -> Operation {
    let mut content = req.clone();
    content.arguments["_approval_context"] = json!({
        "space": if source.space.trim().is_empty() { &source.space_id } else { &source.space },
        "destination": if source.name.trim().is_empty() { &source.id } else { &source.name },
        "destination_kind": source.kind,
    });
    Operation::Content { content }
}
fn source_path(source: &SourcePermission) -> String {
    format!(
        "{}/{}/{}",
        base(&source.space_id),
        if source.kind == "query" {
            "queries"
        } else {
            "collections"
        },
        encode(&source.id)
    )
}
fn selected<'a>(
    project: &'a ProjectConfiguration,
    req: &ContentRequest,
) -> Result<&'a SourcePermission, String> {
    project
        .sources
        .iter()
        .find(|s| {
            s.space_id == req.arguments["space_id"]
                && s.kind == req.arguments["destination"]["kind"]
                && s.id == req.arguments["destination"]["id"]
        })
        .filter(|s| {
            matches!(s.kind.as_str(), "query" | "collection")
                && s.permission.permits("createDelete")
        })
        .ok_or_else(|| {
            "Destination is not a saved Create-Delete source in this verified project and space."
                .into()
        })
}
fn definition(
    client: &Client,
    source: &SourcePermission,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    let path = source_path(source);
    let doc = get(
        client,
        &format!(
            "{}/objects/{}?ids=full",
            base(&source.space_id),
            encode(&source.id)
        ),
        check,
    )?;
    if doc["id"] != source.id {
        return Err("Destination ID did not match the selected source.".into());
    }
    if source.kind == "collection" {
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
    source: &SourcePermission,
    check: &impl Fn() -> Result<(), String>,
) -> Result<(Vec<Value>, Value), String> {
    check()?;
    let properties = client.pages(
        "GET",
        &format!("{}/properties?ids=full", base(&source.space_id)),
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
                    base(&source.space_id),
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
    source: &SourcePermission,
    def: &Value,
    req: &ContentRequest,
    check: &impl Fn() -> Result<(), String>,
) -> Result<String, String> {
    let (response, canonical_type) = query::assess_with_type(client, source, def, req, check)?;
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
    source: &SourcePermission,
    id: &str,
    check: &impl Fn() -> Result<(), String>,
) -> Result<bool, String> {
    let path = source_path(source);
    let urls = if source.kind == "query" {
        let def = definition(client, source, check)?;
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

fn record_path(project: &str, req: &ContentRequest) -> Result<PathBuf, String> {
    let identity = json!([
        project,
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
    pub(super) fn source(kind: &str, access: &str) -> SourcePermission {
        SourcePermission {
            space_id: "s".into(),
            kind: kind.into(),
            id: "dest".into(),
            name: "Destination".into(),
            space: "Work".into(),
            permission: crate::codex::Permission {
                access: access.into(),
                approval: "fullAccess".into(),
                files_enabled: false,
            },
        }
    }
    pub(super) fn project(source: SourcePermission) -> ProjectConfiguration {
        ProjectConfiguration {
            sources: vec![source],
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
        scope: &crate::anyblob_scope::Scope,
        id: &str,
    ) {
        let config = scope.configuration();
        let read = ContentRequest {
            tool: "read_document".into(),
            arguments: json!({"space_id":"s","object_id":id}),
        };
        let mut read =
            anyblob_content::prepare(client, &read, &config, Some(scope), &|| Ok(())).unwrap();
        assert_eq!(
            anyblob_content::execute(client, &mut read, &|| Ok(())).unwrap()["id"],
            id
        );
        let edit = ContentRequest {
            tool: "patch_object".into(),
            arguments: json!({"space_id":"s","object_id":id,"etag":"object-etag","body":{"ops":[{"op":"set_properties","set":{"name":"Edited"}}]}}),
        };
        let mut edit =
            anyblob_content::prepare(client, &edit, &config, Some(scope), &|| Ok(())).unwrap();
        anyblob_content::inspect(client, &mut edit, &config, Some(scope), &|| Ok(())).unwrap();
        assert_eq!(
            anyblob_content::execute(client, &mut edit, &|| Ok(())).unwrap()["outcome"],
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
    fn destination_grants_are_explicit_and_never_cross_projects_or_spaces() {
        let request = req("collection");
        for access in ["none", "read", "readWrite"] {
            assert!(selected(&project(source("collection", access)), &request).is_err());
        }
        assert!(selected(&project(source("collection", "createDelete")), &request).is_ok());
        assert!(selected(&ProjectConfiguration::default(), &request).is_err());
        let mut foreign = source("collection", "createDelete");
        foreign.space_id = "other".into();
        assert!(selected(&project(foreign), &request).is_err());
        let mut missing = request.clone();
        missing
            .arguments
            .as_object_mut()
            .unwrap()
            .remove("destination");
        assert!(selected(&project(source("collection", "createDelete")), &missing).is_err());
        let config = Configuration::default();
        assert!(anyblob_content::permission(&config, None, &request, false).is_err());
    }
    #[test]
    fn empty_saved_sources_are_discoverable_only_with_create_delete() {
        let p = ProjectConfiguration {
            sources: vec![
                source("query", "createDelete"),
                source("collection", "readWrite"),
            ],
            ..Default::default()
        };
        let (client, capture, server) = fixture(vec![
            doc("query"),
            page(json!([{"id":"all"}])),
            page(json!([])),
        ]);
        let request = ContentRequest {
            tool: "list_creation_targets".into(),
            arguments: json!({"space_id":"s"}),
        };
        let result = dispatch(&client, "project", &p, &request, &|| Ok(()), &|_, _| {
            panic!("full access read")
        })
        .unwrap();
        assert_eq!(result["targets"].as_array().unwrap().len(), 1);
        assert_eq!(
            result["targets"][0]["requirements"]["query_source"]["types"][0],
            "page"
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 3);
    }
    #[test]
    fn query_creation_uses_union_and_subsequent_membership_permissions() {
        let mut responses = query_reads();
        responses.extend(query_reads()); // dry run authorization
        responses.push(json!({"valid":true}));
        responses.extend(query_reads()); // commit authorization
        responses.extend(query_reads()); // final authorization after journaling
        responses.push(json!({"id":"new-object"}));
        responses.extend(vec![
            doc("query"),
            page(json!([{"id":"later"}])),
            page(json!([{"id":"new-object"}])),
        ]);
        responses.extend(vec![
            page(json!([{"id":"later"}])),
            page(json!([{"id":"new-object"}])),
        ]);
        responses.extend(normal_access_responses("new-object"));
        let (client, capture, server) = fixture(responses);
        let p = project(source("query", "createDelete"));
        let result = dispatch(
            &client,
            "project",
            &p,
            &req("query"),
            &|| Ok(()),
            &|_, _| panic!("full access"),
        )
        .unwrap();
        assert_eq!(result["creation"]["outcome"], "confirmed");
        assert_eq!(result["placement"]["outcome"], "confirmed");
        let scope = crate::anyblob_scope::Scope::load(&client, &p, &|| Ok(())).unwrap();
        assert!(scope.readable("s", "new-object"));
        let edit = ContentRequest {
            tool: "patch_object".into(),
            arguments: json!({"space_id":"s","object_id":"new-object"}),
        };
        assert!(scope.content_permission(&edit, false).is_some());
        assert!(!scope.readable("s", "unrelated"));
        normal_read_and_edit(&client, &scope, "new-object");
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
            let response = dispatch(
                &client,
                "project",
                &project(source("query", "createDelete")),
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
            json!({"id":"created"}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"applied":true}),
            page(json!([{"id":"created"}])),
            page(json!([{"id":"created"}])),
        ];
        responses.extend(normal_access_responses("created"));
        let (client, capture, server) = fixture(responses);
        let p = project(source("collection", "createDelete"));
        let result = dispatch(
            &client,
            "project",
            &p,
            &req("collection"),
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(result["object_id"], "created");
        assert_eq!(result["placement"]["outcome"], "confirmed");
        let scope = crate::anyblob_scope::Scope::load(&client, &p, &|| Ok(())).unwrap();
        assert!(scope.readable("s", "created"));
        assert!(scope
            .content_permission(
                &ContentRequest {
                    tool: "patch_object".into(),
                    arguments: json!({"space_id":"s","object_id":"created"})
                },
                false
            )
            .is_some());
        assert!(scope
            .content_permission(
                &ContentRequest {
                    tool: "create_property".into(),
                    arguments: json!({"space_id":"s"})
                },
                false
            )
            .is_none());
        normal_read_and_edit(&client, &scope, "created");
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
    fn collection_conflict_recovers_without_creating_twice_and_binds_payload() {
        let request = req("collection");
        let p = project(source("collection", "createDelete"));
        let (client, capture, server) = fixture(vec![
            doc("collection"),
            json!({"valid":true}),
            json!({"id":"kept"}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"fixture_status":409,"message":"ETag conflict"}),
        ]);
        let result = dispatch(
            &client,
            "restart-project",
            &p,
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
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"applied":true}),
            page(json!([{"id":"kept"}])),
        ]);
        let result = dispatch(
            &client,
            "restart-project",
            &p,
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
            .all(|r| !String::from_utf8_lossy(r).starts_with("POST ")));
        let mut changed = request.clone();
        changed.arguments["body"]["name"] = json!("Different");
        let (client, _, server) = fixture(vec![doc("collection")]);
        assert!(dispatch(
            &client,
            "restart-project",
            &p,
            &changed,
            &|| Ok(()),
            &|_, _| Ok(())
        )
        .unwrap_err()
        .contains("different payload"));
        server.join().unwrap();
        assert!(selected(&project(source("collection", "read")), &request).is_err());
    }
    #[test]
    fn uncertain_collection_write_is_not_retried_after_restart() {
        for uncertain_create in [true, false] {
            let request = req("collection");
            let p = project(source("collection", "createDelete"));
            let mut responses = vec![doc("collection"), json!({"valid":true})];
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
            let result = dispatch(
                &client,
                "unknown-project",
                &p,
                &request,
                &|| Ok(()),
                &|_, _| Ok(()),
            )
            .unwrap();
            assert!(result["outcome"] == "unknown" || result["placement"]["outcome"] == "unknown");
            server.join().unwrap();
            let (client, capture, server) = fixture(vec![doc("collection")]);
            let result = dispatch(
                &client,
                "unknown-project",
                &p,
                &request,
                &|| Ok(()),
                &|_, _| Ok(()),
            )
            .unwrap();
            assert_eq!(result["placement"]["outcome"], "unknown");
            server.join().unwrap();
            assert_eq!(capture.lock().unwrap().len(), 1);
        }
    }
    #[test]
    fn creation_approval_uses_saved_destination_and_space_names() {
        for kind in ["query", "collection"] {
            let mut p = project(source(kind, "createDelete"));
            p.sources[0].permission.approval = "alwaysAsk".into();
            let responses = if kind == "query" { query_reads() } else { vec![doc(kind)] };
            let (client, capture, server) = fixture(responses);
            let error = dispatch(&client, "project", &p, &req(kind), &|| Ok(()), &|op, _| {
                assert_eq!(blobui_mcp::approval::describe(op), format!(
                    "An agent wants to create object 'Created' in {kind} 'Destination' in Work."));
                Err("denied".into())
            }).unwrap_err();
            assert!(error.contains("denied"));
            server.join().unwrap();
            assert!(capture.lock().unwrap().iter().all(|r| String::from_utf8_lossy(r).starts_with("GET ")));
        }
    }
    #[test]
    fn approval_denial_cancellation_files_and_schema_payloads_do_not_write() {
        let request = req("collection");
        let mut p = project(source("collection", "createDelete"));
        p.sources[0].permission.approval = "approveForMe".into();
        let (client, capture, server) = fixture(vec![doc("collection")]);
        assert!(
            dispatch(&client, "project", &p, &request, &|| Ok(()), &|_, _| Err(
                "denied".into()
            ))
            .unwrap_err()
            .contains("denied")
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 1);
        let client = Client::with_connection("fixture", "127.0.0.1", 1);
        assert!(dispatch(
            &client,
            "project",
            &p,
            &request,
            &|| Err("cancelled".into()),
            &|_, _| Ok(())
        )
        .unwrap_err()
        .contains("cancelled"));
        let mut files = request.clone();
        files.arguments["body"]["markdown"] = json!("![image](x)");
        assert!(
            dispatch(&client, "project", &p, &files, &|| Ok(()), &|_, _| Ok(()))
                .unwrap_err()
                .contains("permissions")
        );
        for kind in ["property", "object_type", "template"] {
            let mut schema = request.clone();
            schema.arguments["body"]["kind"] = json!(kind);
            assert!(
                dispatch(&client, "project", &p, &schema, &|| Ok(()), &|_, _| Ok(()))
                    .unwrap_err()
                    .contains("schemas")
            );
        }
    }
    #[test]
    fn concurrent_query_changes_reject_before_commit_or_return_retained_receipt() {
        let mut responses = query_reads();
        responses.extend(vec![doc("query"), page(json!([{"id":"changed"}]))]);
        let (client, capture, server) = fixture(responses);
        let request = req("query");
        assert!(dispatch(
            &client,
            "query-change",
            &project(source("query", "createDelete")),
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
        assert!(!record_path("query-change", &request).unwrap().exists());

        let mut responses = query_reads();
        responses.extend(query_reads());
        responses.push(json!({"valid":true}));
        responses.extend(query_reads());
        responses.extend(query_reads());
        responses.push(json!({"id":"retained"}));
        responses.extend(vec![
            doc("query"),
            page(json!([{"id":"changed"}])),
            page(json!([])),
        ]);
        let (client, capture, server) = fixture(responses);
        let result = dispatch(
            &client,
            "query-change",
            &project(source("query", "createDelete")),
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
        let p = project(source("collection", "createDelete"));
        let mut request = req("collection");
        request.arguments["dry_run"] = json!(true);
        let (client, capture, server) = fixture(vec![doc("collection"), json!({"valid":true})]);
        assert_eq!(
            dispatch(&client, "dry", &p, &request, &|| Ok(()), &|_, _| Ok(())).unwrap()["outcome"],
            "validated"
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
        assert!(!record_path("dry", &request).unwrap().exists());
        let path = record_path("dry", &request).unwrap();
        let mut payload = request.arguments.clone();
        payload.as_object_mut().unwrap().remove("dry_run");
        save(&path,&json!({"fingerprint":format!("{:x}",Sha256::digest(payload.to_string().as_bytes())),"state":"created","object_id":"earlier","creation":{"outcome":"confirmed"}})).unwrap();
        let (client, capture, server) = fixture(vec![doc("collection")]);
        assert_eq!(
            dispatch(&client, "dry", &p, &request, &|| Ok(()), &|_, _| Ok(())).unwrap()
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
            dispatch(&client, "files", &p, &files, &|| Ok(()), &|_, _| Ok(()))
                .unwrap_err()
                .contains("permissions")
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
    }
    #[test]
    fn completed_duplicate_and_revoked_recovery_never_create_another_object() {
        let request = req("collection");
        let p = project(source("collection", "createDelete"));
        let (client, _, server) = fixture(vec![
            doc("collection"),
            json!({"valid":true}),
            json!({"id":"created"}),
            page(json!([])),
            doc("collection"),
            schema(),
            json!({"valid":true}),
            json!({"applied":true}),
            page(json!([{"id":"created"}])),
        ]);
        dispatch(&client, "duplicate", &p, &request, &|| Ok(()), &|_, _| {
            Ok(())
        })
        .unwrap();
        server.join().unwrap();
        let (client, capture, server) =
            fixture(vec![doc("collection"), page(json!([{"id":"created"}]))]);
        assert_eq!(
            dispatch(&client, "duplicate", &p, &request, &|| Ok(()), &|_, _| Ok(
                ()
            ))
            .unwrap()["placement"]["outcome"],
            "confirmed"
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 2);
        let client = Client::with_connection("fixture", "127.0.0.1", 1);
        for access in ["none", "read", "readWrite"] {
            assert!(dispatch(
                &client,
                "duplicate",
                &project(source("collection", access)),
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
        let path = record_path("torn", &request).unwrap();
        fs::write(&path, "{\"state\":\"placing\"").unwrap();
        assert!(load(&path).unwrap_err().contains("incomplete"));
    }
}

pub fn dispatch(
    client: &Client,
    project_id: &str,
    project: &ProjectConfiguration,
    req: &ContentRequest,
    check: &impl Fn() -> Result<(), String>,
    approve: &impl Fn(&Operation, &dyn Fn() -> Result<(), String>) -> Result<(), String>,
) -> Result<Value, String> {
    req.validate()?;
    check()?;
    if req.tool == "list_creation_targets" {
        let mut targets = Vec::new();
        for s in &project.sources {
            if s.space_id != req.arguments["space_id"]
                || !matches!(s.kind.as_str(), "query" | "collection")
                || !s.permission.permits("createDelete")
            {
                continue;
            }
            if s.permission.approval == "alwaysAsk" {
                approve(
                    &Operation::Content {
                        content: req.clone(),
                    },
                    check,
                )?;
            }
            let def = definition(client, s, check)?;
            let requirements = if s.kind == "query" {
                query::discovery(client, s, &def, check)?
            } else {
                Value::Null
            };
            targets
                .push(json!({"kind":s.kind,"id":s.id,"name":s.name,"requirements":requirements}));
        }
        check()?;
        return Ok(json!({"targets":targets}));
    }
    let source = selected(project, req)?;
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
        return Err("Scoped creation cannot create schemas, source definitions or collection membership payloads.".into());
    }
    let mut config = Configuration::default();
    config
        .spaces
        .insert(source.space_id.clone(), source.permission.clone());
    let mut prepared = anyblob_content::prepare(client, req, &config, None, check)?;
    let definition = definition(client, source, check)?;
    if source.kind == "query" {
        let (response, canonical_type) =
            query::assess_with_type(client, source, &definition, req, check)?;
        prepared.request.arguments["body"]["type"] = json!(canonical_type);
        prepared.files |= anyblob_content::needs_files(&prepared.request.arguments["body"]);
        anyblob_content::permission(&config, None, &prepared.request, prepared.files)?;
        if let Some(response) = response {
            // A follow-up can reuse a key only if no earlier creation bound it.
            let path = record_path(project_id, req)?;
            if let Some(record) = load(&path)? {
                let mut payload = req.arguments.clone();
                payload.as_object_mut().unwrap().remove("dry_run");
                let fingerprint = format!("{:x}", Sha256::digest(payload.to_string().as_bytes()));
                if record["fingerprint"] != fingerprint {
                    return Err("Idempotency key was already used with a different payload.".into());
                }
                return Ok(result(&record, "unknown", Some("An earlier creation attempt exists. Requirements cannot authorize a new payload or an automatic retry.".into())));
            }
            if source.permission.approval == "alwaysAsk" {
                approve(
                    &creation_prompt(req, source),
                    check,
                )?;
            }
            check()?;
            return Ok(response);
        }
    }
    if req.requires_confirmation(&source.permission.approval) {
        approve(
            &creation_prompt(req, source),
            check,
        )?;
    }
    anyblob_content::inspect(client, &mut prepared, &config, None, check)?;
    // Serialize recovery and new operations across threads. The bridge is a single instance.
    let _lock = OPERATIONS
        .lock()
        .map_err(|_| "Creation operation lock unavailable.")?;
    check()?;
    let path = record_path(project_id, req)?;
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
        let dispatch_type = prepared.request.arguments["body"]["type"].clone();
        let before_write = || {
            check()?;
            if source.kind == "query" {
                let current = self::definition(client, source, check)?;
                if current != definition {
                    return Err("Query definition changed before creation. Retry with current requirements.".into());
                }
                if query_proof(client, source, &current, req, check)? != dispatch_type {
                    return Err(
                        "Type identity changed before creation. Retry with current metadata."
                            .into(),
                    );
                }
            }
            Ok(())
        };
        let mut pending = record.clone();
        pending["state"] = json!("creating");
        let creation =
            anyblob_content::execute_committing(client, &mut prepared, &before_write, &|| {
                save(&path, &pending)
            });
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
        if member(client, source, &id, check)? {
            return Ok(());
        }
        if source.kind == "query" {
            return Err("Created object does not belong to a current query view. It was retained; no continuing access was granted.".into());
        }
        let current = self::definition(client, source, check)?;
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
        let api_path = format!("{}/objects/{}", base(&source.space_id), encode(&source.id));
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
            Sha256::digest(format!("placement:{project_id}:{}:{etag}", req.arguments).as_bytes())
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
        if !member(client, source, &id, check)? {
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
