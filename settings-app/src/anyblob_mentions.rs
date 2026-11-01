//! Global metadata discovery. These filters never authorize object reads.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection<T> {
    pub all: bool,
    pub items: Vec<T>,
}
impl<T> Default for Selection<T> {
    fn default() -> Self { Self { all: true, items: Vec::new() } }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectType {
    pub space_id: String,
    pub id: String,
    pub name: String,
    pub space: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filters {
    pub spaces: Selection<Space>,
    pub types: Selection<ObjectType>,
}
fn selection<T: for<'de> Deserialize<'de>>(value: Option<&Value>) -> Selection<T> {
    match value {
        None => Selection::default(),
        Some(value) if value["all"] == true => Selection::default(),
        Some(value) => Selection {
            all: false,
            items: value["items"].as_array().into_iter().flatten()
                .filter_map(|row| serde_json::from_value(row.clone()).ok()).collect(),
        },
    }
}
impl Filters {
    pub fn normalize(value: Option<&Value>) -> Self {
        match value {
            None => Self::default(),
            Some(value) => Self {
                // A corrupt present filter fails closed rather than restoring All.
                spaces: selection(Some(value.get("spaces").unwrap_or(&Value::Null))),
                types: selection(Some(value.get("types").unwrap_or(&Value::Null))),
            },
        }
    }
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key].as_str().filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Anytype mention search returned a missing {key}."))
}
fn valid_id(id: &str) -> Result<(), String> {
    blobui_mcp::Operation::ReadObject { space_id: id.into(), object_id: id.into() }.validate()
}

/// No cache, bodies, project lookup or approval exceptions. Check before each HTTP request
/// and again before publishing the complete, filtered and sorted result.
pub fn search(
    client: &crate::kanban_api::Client,
    filters: &Filters,
    query: &str,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    search_with(filters, query, check, |method, path, body| client.request(method, path, body))
}
fn search_with(
    filters: &Filters,
    query: &str,
    check: &impl Fn() -> Result<(), String>,
    mut request: impl FnMut(&str, &str, Option<&Value>) -> Result<Value, String>,
) -> Result<Value, String> {
    fn pages(
        method: &str, path: &str, body: Option<&Value>,
        check: &impl Fn() -> Result<(), String>,
        request: &mut impl FnMut(&str, &str, Option<&Value>) -> Result<Value, String>,
    ) -> Result<Vec<Value>, String> {
        let mut rows = Vec::new();
        let mut total = None;
        loop {
            check()?;
            let offset = rows.len();
            if offset >= 100_000 { return Err("Mention search exceeds the safe pagination limit. Narrow the saved filters.".into()); }
            let response = request(method, &format!("{path}&limit=100&offset={offset}"), body)
                .map_err(|_| "Anytype mention search failed. Check the saved API connection and retry.".to_string())?;
            check()?;
            let page = response["data"].as_array().ok_or("Anytype returned invalid mention search results.")?;
            let more = response["has_more"].as_bool().ok_or("Anytype omitted mention search pagination.")?;
            if let Some(current) = response["total"].as_u64() {
                if total.is_some_and(|previous| previous != current) {
                    return Err("Objects changed during mention search pagination. Retry the request.".into());
                }
                total = Some(current);
            }
            if page.len() > 100 || (more && page.is_empty()) {
                return Err("Anytype returned invalid mention search pagination.".into());
            }
            rows.extend(page.iter().cloned());
            if !more {
                if total.is_some_and(|total| total != rows.len() as u64) {
                    return Err("Anytype returned incomplete mention search pagination.".into());
                }
                return Ok(rows);
            }
        }
    }
    check()?;
    if (!filters.spaces.all && filters.spaces.items.is_empty())
        || (!filters.types.all && filters.types.items.is_empty()) {
        return Ok(json!({"items":[]}));
    }
    let spaces = pages("GET", "/v2/spaces?ids=full", None, check, &mut request)?;
    let query = query.to_lowercase();
    let mut results = BTreeMap::new();
    for space in spaces {
        let sid = text(&space, "id")?;
        valid_id(sid)?;
        if !filters.spaces.all && !filters.spaces.items.iter().any(|s| s.id == sid) { continue; }
        if !filters.types.all && !filters.types.items.iter().any(|t| t.space_id == sid) { continue; }
        let space_name = text(&space, "name")?;
        let base=format!("/v2/spaces/{}",crate::api::encode_segment(sid));
        let mut type_keys=std::collections::BTreeSet::new();
        if !filters.types.all {
            for mut object_type in pages("GET",&format!("{base}/types?ids=full"),None,check,&mut request)? {
                let key=text(&object_type,"key")?.to_string();
                if object_type["id"].as_str().filter(|s|!s.is_empty()).is_none() {
                    check()?;
                    let document=request("GET",&format!("{base}/types/{}?ids=full",crate::api::encode_segment(&key)),None)
                        .map_err(|_|"Anytype mention type discovery failed. Retry the search.".to_string())?;
                    check()?;
                    object_type["id"]=json!(text(&document,"id")?);
                }
                let id=text(&object_type,"id")?;
                if filters.types.items.iter().any(|t|t.space_id==sid&&t.id==id) {
                    type_keys.insert(key);type_keys.insert(id.to_string());
                }
            }
            if type_keys.is_empty(){continue;}
        }
        // Enumerate once with an empty API query. Full-text API search has a capped snapshot
        // and overlapping pages, so match titles locally for a complete deterministic order.
        let path = format!("/v2/spaces/{}/search?ids=full", crate::api::encode_segment(sid));
        for row in pages("POST", &path, Some(&json!({})), check, &mut request)? {
            let oid = text(&row, "id")?;
            valid_id(oid)?;
            if !filters.types.all {
                // v2 search rows use type keys; stored filters retain full, space-scoped IDs.
                let selected=if let Some(type_key)=row["type"].as_str() {type_keys.contains(type_key)}
                else if let Some(id)=row.pointer("/type/id").and_then(Value::as_str) {
                    filters.types.items.iter().any(|t| t.space_id==sid&&t.id==id)
                } else {return Err("Anytype omitted the object type. Mention search cannot apply saved filters.".into());};
                if !selected {continue;}
            }
            let title = row["name"].as_str().ok_or("Anytype omitted an object title.")?;
            if !title.to_lowercase().contains(&query) { continue; }
            let item = json!({"type":"resource_link","uri":blobui_mcp::object_uri(sid,oid),
                "name":title,"title":title,"description":space_name,
                "_meta":{"anyblob/spaceId":sid,"anyblob/spaceName":space_name,"anyblob/objectId":oid}});
            let key = (sid.to_string(), oid.to_string());
            if let Some(previous) = results.insert(key, item.clone()) {
                if previous != item { return Err("Objects changed during mention search. Retry the request.".into()); }
            }
        }
    }
    let mut items: Vec<_> = results.into_values().collect();
    items.sort_by(|a,b| {
        let key = |v: &Value| (v["title"].as_str().unwrap().to_string(),
            v["_meta"]["anyblob/spaceName"].as_str().unwrap().to_string(),
            v["_meta"]["anyblob/spaceId"].as_str().unwrap().to_string(),
            v["_meta"]["anyblob/objectId"].as_str().unwrap().to_string());
        key(a).cmp(&key(b))
    });
    items.truncate(20);
    check()?;
    Ok(json!({"items":items}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn filters(value: Value) -> Filters { Filters::normalize(Some(&value)) }
    #[test]
    fn migration_and_unavailable_selections_never_broaden_filters() {
        assert!(Filters::normalize(None).spaces.all);
        for corrupt in [Value::Null, json!({}), json!({"spaces":false,"types":"broken"})] {
            let saved = Filters::normalize(Some(&corrupt));
            assert!(!saved.spaces.all && !saved.types.all);
            assert!(saved.spaces.items.is_empty() && saved.types.items.is_empty());
        }
        let saved = filters(json!({"spaces":{"all":false,"items":[{"id":"unavailable","name":"Saved"}]},
            "types":{"all":false,"items":[{"spaceId":"other","id":"missing","name":"Task","space":"Old"}]}}));
        assert_eq!(saved, Filters::normalize(Some(&serde_json::to_value(&saved).unwrap())));
        let all = filters(json!({"spaces":{"all":true,"items":[{"id":"old","name":"Old"}]},"types":{"all":true}}));
        assert!(all.spaces.items.is_empty());
    }
    #[test]
    fn filters_precede_limit_and_results_contain_only_metadata() {
        let saved = filters(json!({"spaces":{"all":true},"types":{"all":false,"items":[
            {"spaceId":"a","id":"task","name":"Task","space":"Same"},
            {"spaceId":"b","id":"task","name":"Task","space":"Same"}]}}));
        let mut calls = Vec::new();
        let result = search_with(&saved, "Ü", &|| Ok(()), |method,path,body| {
            calls.push(path.to_string());
            assert!(!path.contains("/objects/"));
            if path.contains("/types?"){return Ok(json!({"data":[{"id":"task","key":"task-key"}],"has_more":false}));}
            if method == "GET" { return Ok(json!({"data":[{"id":"b","name":"Same"},{"id":"a","name":"Same"},{"id":"excluded","name":"Other"}],"has_more":false})); }
            assert_eq!(body,Some(&json!({})));
            if path.ends_with("offset=0") {
                return Ok(json!({"data":(0..100).map(|i| json!({"id":format!("skip{i}"),"name":"Ü noise","type":{"id":"other"}})).collect::<Vec<_>>(),"has_more":true,"total":125}));
            }
            assert!(path.ends_with("offset=100"));
            Ok(json!({"data":(0..25).rev().map(|i| json!({"id":format!("obj{i:02}"),"name":"Über","type":{"id":"task"},"properties":{"secret":"never"},"markdown":"never","blocks":["never"]})).collect::<Vec<_>>(),"has_more":false,"total":125}))
        }).unwrap();
        let rows = result["items"].as_array().unwrap();
        assert_eq!(rows.len(),20);
        assert_eq!(rows[0]["uri"],"anyblob://objects/a/obj00");
        assert_eq!(rows[19]["uri"],"anyblob://objects/a/obj19");
        assert!(!result.to_string().contains("never"));
        assert_eq!(calls.len(),7);
    }
    #[test]
    fn dynamic_all_empty_selection_unicode_and_deduplication() {
        let run = |saved: &Filters, query: &str| search_with(saved,query,&||Ok(()), |method,_,_| {
            Ok(if method=="GET" {json!({"data":[{"id":"new-space","name":"新"}],"has_more":false})}
            else {json!({"data":[{"id":"one","name":"猫"},{"id":"one","name":"猫"},{"id":"two","name":""}],"has_more":false})})
        }).unwrap();
        assert_eq!(run(&Filters::default(),"")["items"].as_array().unwrap().len(),2);
        assert_eq!(run(&Filters::default(),"猫")["items"].as_array().unwrap().len(),1);
        for saved in [filters(json!({"spaces":{"all":false,"items":[]},"types":{"all":true}})),
            filters(json!({"spaces":{"all":true},"types":{"all":false,"items":[]}}))] {
            let result=search_with(&saved,"",&||Ok(()),|_,_,_|panic!("Empty selection must not make HTTP calls")).unwrap();
            assert_eq!(result,json!({"items":[]}));
        }
    }
    #[test]
    fn full_type_ids_resolve_to_current_api_keys_without_cross_space_collisions() {
        let saved=filters(json!({"spaces":{"all":true},"types":{"all":false,"items":[
            {"spaceId":"a","id":"full-task","name":"Duplicate","space":"Same"},
            {"spaceId":"b","id":"unavailable","name":"Duplicate","space":"Same"}]}}));
        let mut calls=Vec::new();
        let result=search_with(&saved,"",&||Ok(()),|method,path,_|{
            calls.push(path.to_string());
            Ok(if path.starts_with("/v2/spaces?") {json!({"data":[{"id":"a","name":"Same"},{"id":"b","name":"Same"}],"has_more":false})}
            else if path.contains("/types?"){json!({"data":[{"key":"renamed task / key"}],"has_more":false})}
            else if path.contains("/types/renamed%20task%20%2F%20key?"){json!({"id":"full-task"})}
            else {assert_eq!(method,"POST");assert!(path.contains("/spaces/a/"));json!({"data":[{"id":"one","name":"Task","type":"renamed task / key"},{"id":"two","name":"Wrong","type":"other"}],"has_more":false})})
        }).unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(),1);
        assert_eq!(result["items"][0]["uri"],"anyblob://objects/a/one");
        assert!(!calls.iter().any(|p|p.contains("/spaces/b/search")));
    }
    #[test]
    fn concurrent_queries_and_filter_snapshots_do_not_mix() {
        let barrier=std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads:Vec<_>=[("a","猫"),("b","Über")].into_iter().map(|(sid,query)|{
            let barrier=barrier.clone();
            std::thread::spawn(move||{
                let saved=filters(json!({"spaces":{"all":false,"items":[{"id":sid,"name":"Saved"}]},"types":{"all":true}}));
                search_with(&saved,query,&||Ok(()),|method,path,_|{
                    if method=="GET"{barrier.wait();return Ok(json!({"data":[{"id":"a","name":"A"},{"id":"b","name":"B"}],"has_more":false}));}
                    assert!(path.contains(&format!("/spaces/{sid}/")));
                    Ok(json!({"data":[{"id":"cat","name":"猫"},{"id":"word","name":"Über"}],"has_more":false}))
                }).unwrap()
            })
        }).collect();
        let results:Vec<_>=threads.into_iter().map(|t|t.join().unwrap()).collect();
        assert_eq!(results[0]["items"][0]["uri"],"anyblob://objects/a/cat");
        assert_eq!(results[1]["items"][0]["uri"],"anyblob://objects/b/word");
    }
    #[test]
    fn cancellation_failures_and_forged_ids_fail_closed() {
        assert!(search_with(&Filters::default(),"",&||Err("cancelled".into()),|_,_,_|panic!()).is_err());
        assert!(search_with(&Filters::default(),"",&||Ok(()),|_,_,_|Err("secret server error".into())).unwrap_err().contains("search failed"));
        for response in [json!({"data":[],"has_more":true}),json!({"data":[],"has_more":false,"total":1}),
            json!({"data":[{"id":"../escape","name":"bad"}],"has_more":false})] {
            assert!(search_with(&Filters::default(),"",&||Ok(()),|_,_,_|Ok(response.clone())).is_err());
        }
        let changed=std::cell::Cell::new(false);
        let result=search_with(&Filters::default(),"",&||if changed.get(){Err("filters changed".into())}else{Ok(())},|_,_,_| {
            changed.set(true);Ok(json!({"data":[],"has_more":false}))
        });
        assert_eq!(result.unwrap_err(),"filters changed");
    }
}
