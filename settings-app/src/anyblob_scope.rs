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
impl Scope {
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
        let grants=if request.space_wide() { &space.all_grants } else {
            request.target().and_then(|id|space.object_grants.get(id)).unwrap_or(&space.all_grants)
        };
        let contributors=grants.iter().filter(|p|p.permits(request.capability()) && (!files || p.files_enabled)).collect::<Vec<_>>();
        let first=contributors.first()?;
        let mut effective=(*first).clone();
        effective.approval=if contributors.iter().any(|p|p.approval=="alwaysAsk") {"alwaysAsk"} else {"approveForMe"}.into();
        Some(effective)
    }
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
    fn grant(access:&str,approval:&str,files:bool)->Permission {Permission{access:access.into(),approval:approval.into(),files_enabled:files,}}
    fn req(name:&str,args:Value)->blobui_mcp::content::ContentRequest {let blobui_mcp::Operation::Content{content}=blobui_mcp::operation(name,args).unwrap()else{panic!()};content}
    #[test] fn capabilities_and_approvals_are_combined_independently() {
        let mut space=Space::default();space.all_grants=vec![grant("createDelete","fullAccess",true)];
        space.object_grants.insert("o".into(),vec![grant("read","alwaysAsk",true),grant("readWrite","fullAccess",false)]);
        let scope=Scope{spaces:BTreeMap::from([("s".into(),space)])};
        let read=req("read_document",json!({"space_id":"s","object_id":"o"}));assert_eq!(scope.content_permission(&read,false).unwrap().approval,"alwaysAsk");
        let edit=req("patch_object",json!({"space_id":"s","object_id":"o","etag":"e","body":{"ops":[{"op":"set_properties","set":{"name":"new"}}]}}));
        assert_eq!(scope.content_permission(&edit,false).unwrap().approval,"approveForMe");assert!(scope.content_permission(&edit,true).is_none());
        let delete=req("delete_object",json!({"space_id":"s","object_id":"o"}));assert!(scope.content_permission(&delete,false).is_none());
        let create=req("create_object",json!({"space_id":"s","body":{"type":"page"}}));assert!(scope.content_permission(&create,false).is_some());
    }
    #[test] fn member_grants_cannot_create_objects_or_mutate_schema() {
        let mut space=Space::default();space.object_grants.insert("o".into(),vec![grant("createDelete","fullAccess",true)]);
        let scope=Scope{spaces:BTreeMap::from([("s".into(),space)])};
        for (name,args) in [("create_object",json!({"space_id":"s","body":{"type":"page"}})),("update_property",json!({"space_id":"s","key":"property","body":{"name":"new"}})),("create_widget",json!({"space_id":"s","body":{"target":"o","scope":"personal"}}))] {
            let req=req(name,args);assert!(scope.content_permission(&req,false).is_none());
        }
    }
}
