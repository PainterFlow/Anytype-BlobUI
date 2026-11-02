//! Project source membership and read permissions. Nothing here accepts API paths.
use crate::{
    codex::{Configuration, Permission, ProjectConfiguration},
    kanban_api::Client,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    spaces: BTreeMap<String, Space>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Space {
    all: Option<Permission>,
    all_grants: Vec<Permission>,
    schema_grants: Vec<Permission>,
    object_grants: BTreeMap<String, Vec<Permission>>,
    objects: BTreeMap<String, Permission>,
}
fn reads(permission: &Permission) -> bool {
    permission.permits("read")
}
fn merge(existing: &mut Permission, incoming: &Permission) {
    // Only read grants contribute to read approval. A denial cannot add approval requirements.
    if reads(incoming) {
        let ask = (reads(existing) && existing.approval == "alwaysAsk")
            || incoming.approval == "alwaysAsk";
        existing.access = "read".into();
        existing.approval = if ask { "alwaysAsk" } else { "approveForMe" }.into();
    }
}
fn encoded(value: &str) -> String {
    crate::api::encode_segment(value)
}
// Read only structural references. Property values and names never grant metadata access.
fn property_references(doc: &Value, keys: &mut std::collections::BTreeSet<String>) {
    if let Some(properties) = doc["properties"].as_object() {
        keys.extend(properties.keys().cloned());
    }
    if let Some(mapping) = doc["property_internal_keys"].as_object() {
        keys.extend(mapping.keys().cloned());
        keys.extend(mapping.values().filter_map(Value::as_str).map(String::from));
    }
    fn references(value: &Value, keys: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Array(rows) => for row in rows { references(row, keys); },
            Value::Object(fields) => for (key, value) in fields {
                if matches!(key.as_str(), "property" | "internal_key" | "relation_key" | "relationKey" | "property_key" | "propertyKey") {
                    if let Some(key) = value.as_str().or_else(||value["key"].as_str()) { keys.insert(key.into()); }
                } else if matches!(key.as_str(), "columns" | "relations" | "properties") && value.is_array() {
                    for item in value.as_array().unwrap() {
                        if let Some(key) = item.as_str().or_else(||item["key"].as_str()) { keys.insert(key.into()); }
                        references(item, keys);
                    }
                } else if !matches!(key.as_str(), "value" | "values" | "text" | "content") { references(value, keys); }
            },
            _ => (),
        }
    }
    for field in ["views", "blocks", "type_settings"] { references(&doc[field], keys); }
    if let Some(properties) = doc.pointer("/query_source/properties").and_then(Value::as_array) {
        keys.extend(properties.iter().filter_map(Value::as_str).map(String::from));
    }
}
fn property_page(rows: Vec<Value>, keys: &std::collections::BTreeSet<String>, offset: usize, limit: usize) -> Value {
    let rows = rows.into_iter().filter(|row| ["key", "id", "internal_key"].iter().any(|field| row[*field].as_str().is_some_and(|key| keys.contains(key)))).collect::<Vec<_>>();
    let total = rows.len();
    json!({"data":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),"total":total,"has_more":offset.saturating_add(limit)<total})
}
impl Scope {
    pub(crate) fn list_properties(
        &self, client: &Client, project: &ProjectConfiguration,
        req: &blobui_mcp::content::ContentRequest, check: &impl Fn() -> Result<(), String>,
    ) -> Result<Value, String> {
        let space = req.arguments["space_id"].as_str().ok_or("Missing space ID.")?;
        let base = format!("/v2/spaces/{}", encoded(space));
        let get = |path: String| -> Result<Value, String> {
            check()?;
            let doc = client.content_request("GET", &path, None, None, None, None)?;
            if doc.get("_http_error").is_some() { return Err("Could not read scoped property metadata.".into()); }
            Ok(doc)
        };
        let mut keys = std::collections::BTreeSet::new();
        let mut types = std::collections::BTreeSet::new();
        for source in project.sources.iter().filter(|s| s.space_id == space && reads(&s.permission)) {
            if source.kind == "type" { types.insert(source.id.clone()); continue; }
            if !matches!(source.kind.as_str(), "query" | "collection") { continue; }
            let doc = get(format!("{base}/objects/{}?ids=full", encoded(&source.id)))?;
            if doc["id"] != source.id { return Err("Source identity changed during property discovery.".into()); }
            property_references(&doc, &mut keys);
            if let Some(rows) = doc.pointer("/query_source/types").and_then(Value::as_array) {
                types.extend(rows.iter().filter_map(Value::as_str).map(String::from));
            }
            if source.kind == "query" {
                check()?;
                let views = client.pages("GET", &format!("{base}/queries/{}/views?ids=full", encoded(&source.id)), None)?;
                property_references(&json!({"views":views}), &mut keys);
            }
        }
        for id in self.object_ids(space) {
            let doc = get(format!("{base}/objects/{}?ids=full", encoded(&id)))?;
            if doc["id"] != id { return Err("Member identity changed during property discovery.".into()); }
            property_references(&doc, &mut keys);
            if let Some(key) = doc["type"].as_str().or_else(|| doc["type"]["key"].as_str()) { types.insert(key.into()); }
        }
        if !types.is_empty() {
            check()?;
            let rows = crate::codex::source_types(client, &base)?;
            for row in rows.iter().filter(|row| ["id", "key"].iter().any(|field| row[*field].as_str().is_some_and(|id| types.contains(id)))) {
                let key = row["key"].as_str().ok_or("Type metadata omitted its API key.")?;
                let doc = get(format!("{base}/types/{}?ids=full", encoded(key)))?;
                property_references(&doc, &mut keys);
            }
        }
        check()?;
        let rows = client.pages("GET", &format!("{base}/properties?ids=full"), None)?;
        check()?;
        Ok(property_page(rows, &keys, req.arguments["offset"].as_u64().unwrap_or(0) as usize, req.arguments["limit"].as_u64().unwrap_or(25) as usize))
    }
    pub fn load(
        client: &Client,
        project: &ProjectConfiguration,
        check: &impl Fn() -> Result<(), String>,
    ) -> Result<Self, String> {
        let mut scope = Self::default();
        for source in &project.sources {
            check()?;
            blobui_mcp::Operation::ReadObject {
                space_id: source.space_id.clone(),
                object_id: source.id.clone(),
            }
            .validate()?;
            let base = format!("/v2/spaces/{}", encoded(&source.space_id));
            let space = scope.spaces.entry(source.space_id.clone()).or_default();
            // API schemas contain no source members. Keep grants for empty sources too.
            space.schema_grants.push(source.permission.clone());
            if source.kind == "space" {
                space.all_grants.push(source.permission.clone());
                if source.id != source.space_id {
                    return Err("Invalid saved project space selection.".into());
                }
                if let Some(permission) = &mut space.all {
                    merge(permission, &source.permission);
                } else {
                    space.all = Some(source.permission.clone());
                }
                continue;
            }
            let rows = match source.kind.as_str() {
                "query" => {
                    let path = format!("{base}/queries/{}", encoded(&source.id));
                    let views = client.pages("GET", &format!("{path}/views?ids=full"), None)?;
                    let mut objects = BTreeMap::new();
                    let mut view_ids = std::collections::BTreeSet::new();
                    for view in views {
                        check()?;
                        let id = view["id"].as_str().filter(|id| !id.is_empty())
                            .ok_or("Anytype returned a query view without an ID.")?;
                        if !view_ids.insert(id.to_string()) {
                            return Err("Query views changed during pagination. Retry the request.".into());
                        }
                        let mut seen = std::collections::BTreeSet::new();
                        for row in client.pages("GET", &format!("{path}/objects?ids=full&view={}", encoded(id)), None)? {
                            let id = row["id"].as_str().filter(|id| !id.is_empty())
                                .ok_or("Anytype returned project membership without object IDs.")?;
                            if !seen.insert(id.to_string()) {
                                return Err("Project membership changed during pagination. Retry the request.".into());
                            }
                            objects.insert(id.to_string(), row);
                        }
                    }
                    objects.into_values().collect()
                }
                "collection" => {
                    client.pages(
                        "GET",
                        &format!("{base}/collections/{}/objects?ids=full", encoded(&source.id)),
                        None,
                    )?
                }
                "type" => {
                    let types = crate::codex::source_types(client, &base)?;
                    let row = types.iter().find(|row| row["id"].as_str() == Some(&source.id))
                        .ok_or("A selected project object type is unavailable. Refresh project sources.")?;
                    let key = row["key"]
                        .as_str()
                        .filter(|key| !key.is_empty())
                        .ok_or("Anytype omitted the selected object type's API key.")?;
                    client.pages(
                        "POST",
                        &format!("{base}/search?ids=full"),
                        Some(&json!({"type":key})),
                    )?
                }
                _ => return Err("Invalid saved project source. Refresh project sources.".into()),
            };
            let mut seen = std::collections::BTreeSet::new();
            for row in rows {
                let id = row["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .ok_or("Anytype returned project membership without object IDs.")?;
                if !seen.insert(id.to_string()) {
                    return Err(
                        "Project membership changed during pagination. Retry the request.".into(),
                    );
                }
                space.object_grants.entry(id.into()).or_default().push(source.permission.clone());
                if let Some(permission) = space.objects.get_mut(id) {
                    merge(permission, &source.permission);
                } else {
                    space.objects.insert(id.into(), source.permission.clone());
                }
            }
        }
        check()?;
        Ok(scope)
    }
    pub fn permission(&self, space: &str, object: &str) -> Option<&Permission> {
        let space = self.spaces.get(space)?;
        space.objects.get(object).or(space.all.as_ref())
    }
    pub fn content_permission(&self, request: &blobui_mcp::content::ContentRequest, files: bool) -> Option<Permission> {
        let space=self.spaces.get(request.arguments["space_id"].as_str()?)?;
        let grants=if matches!(request.tool.as_str(), "list_schemas" | "get_schema" | "get_op_schema" | "list_properties") {
            &space.schema_grants
        } else if request.space_wide() { &space.all_grants } else {
            request.target().and_then(|id|space.object_grants.get(id)).unwrap_or(&space.all_grants)
        };
        let contributors=grants.iter().filter(|p|p.permits(request.capability()) && (!files || p.files_enabled)).collect::<Vec<_>>();
        let first=contributors.first()?;
        let mut effective=(*first).clone();
        effective.approval=if contributors.iter().any(|p|p.approval=="alwaysAsk") {"alwaysAsk"} else if contributors.iter().any(|p|p.approval=="approveForMe") {"approveForMe"} else {"fullAccess"}.into();
        Some(effective)
    }
    pub(crate) fn whole_space_permission(&self, space: &str, write: bool) -> Option<Permission> {
        let grants=self.spaces.get(space)?.all_grants.iter().filter(|p|p.permits(if write {"write"}else{"read"})).collect::<Vec<_>>();
        let mut result=(*grants.first()?).clone();
        if grants.iter().any(|p|p.approval=="alwaysAsk"){result.approval="alwaysAsk".into();}Some(result)
    }
    pub(crate) fn object_ids(&self, space: &str) -> Vec<String> {self.spaces.get(space).map(|s|s.objects.iter().filter(|(_,p)|p.permits("read")).map(|(id,_)|id.clone()).collect()).unwrap_or_default()}
    pub(crate) fn has_whole_space(&self, space: &str) -> bool {self.spaces.get(space).is_some_and(|s|s.all_grants.iter().any(|p|p.permits("read")))}
    pub fn readable(&self, space: &str, object: &str) -> bool {
        self.permission(space, object).is_some_and(reads)
    }
    pub fn configuration(&self) -> Configuration {
        let mut config = Configuration::default();
        for (id, space) in &self.spaces {
            let permissions = space
                .all
                .iter()
                .chain(space.objects.values())
                .filter(|permission| reads(permission))
                .collect::<Vec<_>>();
            if !permissions.is_empty() {
                config.spaces.insert(
                    id.clone(),
                    Permission {
                        access: "read".into(),

                        files_enabled: false,
                        approval: if permissions.iter().any(|p| p.approval == "alwaysAsk") {
                            "alwaysAsk"
                        } else {
                            "approveForMe"
                        }
                        .into(),
                    },
                );
            }
        }
        config
    }
    pub fn filter_search(
        &self,
        space: &str,
        rows: Vec<Value>,
        offset: u32,
        limit: u32,
    ) -> Result<Value, String> {
        let mut visible = Vec::new();
        for row in rows {
            let id = row["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or("Anytype returned a search result without an object ID.")?;
            if self.readable(space, id) {
                visible.push(row);
            }
        }
        let total = visible.len();
        let selected = visible
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect::<Vec<_>>();
        Ok(
            json!({"data":selected,"total":total,"has_more":(offset as usize).saturating_add(limit as usize)<total}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(name: &str, arguments: Value) -> blobui_mcp::content::ContentRequest {
        let blobui_mcp::Operation::Content {content} = blobui_mcp::operation(name, arguments).unwrap() else {panic!()};
        content
    }
    #[test]
    fn scoped_property_catalog_filters_before_pagination_and_preserves_mutation_denials() {
        for kind in ["query", "collection", "type"] {
            let permission = p("createDelete", "alwaysAsk");
            let mut space = Space::default();
            space.schema_grants.push(permission.clone());
            let scope = Scope { spaces: BTreeMap::from([("s".into(), space)]) };
            let project = ProjectConfiguration { sources: vec![crate::codex::SourcePermission {
                space_id:"s".into(), id:"selected".into(), kind:kind.into(), name:String::new(), space:String::new(), permission,
            }], ..Default::default() };
            let page = |rows: Value| json!({"data":rows,"has_more":false});
            let properties = page(json!([{"key":"private"},{"key":"status"},{"key":"due"}]));
            let responses = match kind {
                "query" => vec![json!({"id":"selected","query_source":{"properties":["status"]}}), page(json!([{"id":"view","filters":[{"property":"due","value":"private"}]}])), properties],
                "collection" => vec![json!({"id":"selected","properties":{"status":"private","due":null}}), properties],
                _ => vec![page(json!([{"id":"selected","key":"task"}])), json!({"id":"selected","type_settings":{"property_definitions":[{"property":"status"},{"property":"due"}]}}), properties],
            };
            let (client, requests, server) = crate::anyblob_content::tests::fixture(responses);
            let request = req("list_properties", json!({"space_id":"s","offset":1,"limit":1}));
            assert_eq!(scope.content_permission(&request,false).unwrap().approval,"alwaysAsk");
            assert_eq!(scope.list_properties(&client,&project,&request,&||Ok(())).unwrap(), json!({"data":[{"key":"due"}],"total":2,"has_more":false}), "{kind}");
            server.join().unwrap();
            assert!(requests.lock().unwrap().iter().all(|bytes| bytes.starts_with(b"GET ")));
            for (name,args) in [
                ("create_property",json!({"space_id":"s","body":{"name":"New","format":"text"}})),
                ("update_property",json!({"space_id":"s","key":"status","body":{"name":"New"}})),
                ("delete_property",json!({"space_id":"s","key":"status"})),
            ] { assert!(scope.content_permission(&req(name,args),false).is_none()); }
        }
    }
    #[test]
    fn property_references_exclude_values_and_paginate_empty_results() {
        let mut keys = std::collections::BTreeSet::new();
        property_references(&json!({"properties":{"name":"secret"},"type_settings":{"property_definitions":[{"property":"status","internal_key":"stored-status"}]},"views":[{"filters":[{"property":"due","value":{"property":"secret"}}]}]}), &mut keys);
        assert!(keys.contains("name") && keys.contains("status") && keys.contains("stored-status") && keys.contains("due"));
        assert!(!keys.contains("secret"));
        assert_eq!(property_page(vec![json!({"key":"secret"}),json!({"key":"name"}),json!({"key":"due"})], &keys, 0, 1),json!({"data":[{"key":"name"}],"total":2,"has_more":true}));
        assert_eq!(property_page(vec![], &keys, 0, 25),json!({"data":[],"total":0,"has_more":false}));
        let scope = Scope {spaces:BTreeMap::from([("s".into(),Space {schema_grants:vec![p("none","fullAccess")],..Default::default()})])};
        assert!(scope.content_permission(&req("list_properties",json!({"space_id":"s"})),false).is_none());
        assert!(scope.content_permission(&req("list_properties",json!({"space_id":"other"})),false).is_none());
    }
    #[test]
    fn member_properties_and_type_definitions_are_visible_without_exposing_other_metadata() {
        let mut space = Space::default();
        space.objects.insert("member".into(),p("read","fullAccess"));
        let scope = Scope {spaces:BTreeMap::from([("s".into(),space)])};
        let page = |rows: Value| json!({"data":rows,"has_more":false});
        let (client, _, server) = crate::anyblob_content::tests::fixture(vec![
            json!({"id":"member","type":"task","properties":{"status":"private"}}),
            page(json!([{"id":"task-id","key":"task"},{"id":"private-id","key":"private-type"}])),
            json!({"id":"task-id","type_settings":{"property_definitions":[{"property":"due"}]}}),
            page(json!([{"key":"private"},{"key":"status"},{"key":"due"}])),
        ]);
        assert_eq!(scope.list_properties(&client,&ProjectConfiguration::default(),&req("list_properties",json!({"space_id":"s"})),&||Ok(())).unwrap(),json!({"data":[{"key":"status"},{"key":"due"}],"total":2,"has_more":false}));
        server.join().unwrap();
    }
    fn p(access: &str, approval: &str) -> Permission {
        Permission {
            access: access.into(),

            approval: approval.into(),
            files_enabled: false,
        }
    }
    #[test]
    fn specific_rules_override_spaces_and_only_read_grants_contribute_approval() {
        let mut space = Space {
            all: Some(p("read", "fullAccess")),
            objects: BTreeMap::new(),
            ..Space::default()
        };
        space
            .objects
            .insert("denied".into(), p("none", "alwaysAsk"));
        space
            .objects
            .insert("allowed".into(), p("none", "alwaysAsk"));
        merge(
            space.objects.get_mut("allowed").unwrap(),
            &p("read", "fullAccess"),
        );
        assert_eq!(space.objects["allowed"].approval, "approveForMe");
        merge(
            space.objects.get_mut("allowed").unwrap(),
            &p("read", "alwaysAsk"),
        );
        let scope = Scope {
            spaces: BTreeMap::from([("s".into(), space)]),
        };
        assert!(!scope.readable("s", "denied"));
        assert!(scope.readable("s", "allowed"));
        assert!(scope.readable("s", "other"));
        assert!(!scope.readable("outside", "allowed"));
        assert_eq!(
            scope.permission("s", "allowed").unwrap().approval,
            "alwaysAsk"
        );
        let page = scope
            .filter_search(
                "s",
                vec![
                    json!({"id":"denied"}),
                    json!({"id":"allowed"}),
                    json!({"id":"other"}),
                ],
                1,
                1,
            )
            .unwrap();
        assert_eq!(
            page,
            json!({"data":[{"id":"other"}],"total":2,"has_more":false})
        );
    }
}

#[cfg(test)]
mod mutation_permission_tests {
    use super::*;
    #[test]
    fn empty_sources_allow_schema_discovery_without_widening_content_access() {
        for access in ["none", "read", "readWrite", "createDelete"] {
            let space = Space { schema_grants: vec![grant(access,"alwaysAsk",false)], ..Space::default() };
            let scope = Scope { spaces: BTreeMap::from([("s".into(),space)]) };
            for (name,args) in [
                ("list_schemas",json!({"space_id":"s"})),
                ("get_schema",json!({"space_id":"s","kind":"object"})),
                ("get_op_schema",json!({"space_id":"s","op":"insert_blocks"})),
            ] {
                let request = req(name,args);
                let permission = scope.content_permission(&request,false);
                assert_eq!(permission.is_some(),access != "none");
                if let Some(permission) = permission { assert_eq!(permission.approval,"alwaysAsk"); }
                let mut other = request.clone(); other.arguments["space_id"] = json!("other");
                assert!(scope.content_permission(&other,false).is_none());
            }
            for (name,args) in [
                ("read_document",json!({"space_id":"s","object_id":"outside"})),
                ("list_types",json!({"space_id":"s"})),
                ("create_object",json!({"space_id":"s","body":{"type":"page"}})),
                ("update_property",json!({"space_id":"s","key":"p","body":{"name":"new"}})),
            ] {
                assert!(scope.content_permission(&req(name,args),false).is_none());
            }
        }
    }
    fn grant(access:&str,approval:&str,files:bool)->Permission {Permission{access:access.into(),approval:approval.into(),files_enabled:files,}}
    fn req(name:&str,args:Value)->blobui_mcp::content::ContentRequest {let blobui_mcp::Operation::Content{content}=blobui_mcp::operation(name,args).unwrap()else{panic!()};content}
    #[test] fn capabilities_and_approvals_are_combined_independently() {
        let mut space=Space::default();space.all_grants=vec![grant("createDelete","fullAccess",true)];
        space.object_grants.insert("o".into(),vec![grant("read","alwaysAsk",true),grant("readWrite","fullAccess",false)]);
        let scope=Scope{spaces:BTreeMap::from([("s".into(),space)])};
        let read=req("read_document",json!({"space_id":"s","object_id":"o"}));assert_eq!(scope.content_permission(&read,false).unwrap().approval,"alwaysAsk");
        let edit=req("patch_object",json!({"space_id":"s","object_id":"o","etag":"e","body":{"ops":[{"op":"set_properties","set":{"name":"new"}}]}}));
        assert_eq!(scope.content_permission(&edit,false).unwrap().approval,"fullAccess");assert!(scope.content_permission(&edit,true).is_none());
        let delete=req("delete_object",json!({"space_id":"s","object_id":"o"}));assert!(scope.content_permission(&delete,false).is_none());
        let create=req("create_object",json!({"space_id":"s","body":{"type":"page"}}));assert!(scope.content_permission(&create,false).is_some());
    }
    #[test] fn creation_preserves_never_ask_and_stricter_contributing_approvals() {
        for (approvals, expected) in [
            (vec!["fullAccess"], "fullAccess"),
            (vec!["fullAccess", "fullAccess"], "fullAccess"),
            (vec!["fullAccess", "approveForMe"], "approveForMe"),
            (vec!["approveForMe", "fullAccess"], "approveForMe"),
            (vec!["fullAccess", "alwaysAsk"], "alwaysAsk"),
        ] {
            let mut space = Space::default();
            space.all_grants = approvals.iter().map(|approval| grant("createDelete", approval, false)).collect();
            // A read-only grant must not add confirmation to creation.
            space.all_grants.push(grant("read", "alwaysAsk", false));
            let scope = Scope { spaces: BTreeMap::from([("s".into(), space)]) };
            let create = req("create_object", json!({"space_id":"s","body":{"type":"page"}}));
            let permission = scope.content_permission(&create, false).unwrap();
            assert_eq!(permission.approval, expected, "{approvals:?}");
            assert_eq!(create.requires_confirmation(&permission.approval), expected != "fullAccess", "{approvals:?}");
            let irreversible = req("delete_chat_message", json!({"space_id":"s","chat_id":"chat","message_id":"message"}));
            assert!(irreversible.requires_confirmation(&permission.approval));
        }
    }
    #[test] fn member_grants_cannot_create_objects_or_mutate_schema() {
        let mut space=Space::default();space.object_grants.insert("o".into(),vec![grant("createDelete","fullAccess",true)]);
        let scope=Scope{spaces:BTreeMap::from([("s".into(),space)])};
        for (name,args) in [("create_object",json!({"space_id":"s","body":{"type":"page"}})),("update_property",json!({"space_id":"s","key":"property","body":{"name":"new"}})),("create_widget",json!({"space_id":"s","body":{"target":"o","scope":"personal"}}))] {
            let req=req(name,args);assert!(scope.content_permission(&req,false).is_none());
        }
    }
}
