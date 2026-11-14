//! Query creation asks for values before writing. No pending object or edit grant exists.
use super::*;

pub(super) fn resolve_type<'a>(input: &str, types: &'a [Value]) -> Result<&'a Value, String> {
    let keys = types
        .iter()
        .filter(|t| t["key"] == input)
        .collect::<Vec<_>>();
    let ids = types
        .iter()
        .filter(|t| t["id"] == input)
        .collect::<Vec<_>>();
    if keys.len() == 1 && (ids.is_empty() || (ids.len() == 1 && keys[0]["id"] == ids[0]["id"])) {
        return Ok(keys[0]);
    }
    if keys.is_empty() && ids.len() == 1 {
        return Ok(ids[0]);
    }
    Err("Requested type must resolve to one live v2 metadata key or object ID.".into())
}

pub(super) fn filter_values(
    node: &Value,
    prop: &Value,
    options: &Value,
    types: &[Value],
) -> Result<Vec<Value>, String> {
    let key = prop["key"].as_str().ok_or("Property has no API key.")?;
    if key == "type" && prop["format"] != "objects" {
        return Err("Object type metadata has an unsupported format.".into());
    }
    if node.as_object().is_none_or(|o| {
        o.keys()
            .any(|k| !["property", "condition", "value", "id"].contains(&k.as_str()))
    }) {
        return Err(format!("Property {key} has unsupported filter fields."));
    }
    let values = match node["condition"].as_str() {
        Some("equal") => vec![node["value"].clone()],
        Some("in") if key == "type" || prop["format"] == "select" => node["value"]
            .as_array()
            .filter(|a| !a.is_empty())
            .cloned()
            .ok_or("An in filter requires a nonempty list.")?,
        _ => return Err(format!("Property {key} uses an unsupported condition.")),
    };
    values
        .iter()
        .map(|value| {
            if value
                .as_str()
                .is_some_and(|s| s.starts_with("_filter_template_"))
            {
                return Err(format!(
                    "Property {key} uses an unsupported dynamic placeholder."
                ));
            }
            if key == "type" && prop["format"] == "objects" {
                let ty = resolve_type(
                    value
                        .as_str()
                        .ok_or("Type filter requires a type ID or API key.")?,
                    types,
                )?;
                return Ok(ty["id"].clone());
            }
            match prop["format"].as_str() {
                Some("text") if value.is_string() => Ok(value.clone()),
                Some("number") if value.is_number() => Ok(value.clone()),
                Some("checkbox") if value.is_boolean() => Ok(value.clone()),
                Some("select") => {
                    let name = value
                        .as_str()
                        .ok_or("Select filter requires existing option names.")?;
                    let matches = options[key]
                        .as_array()
                        .ok_or("Select metadata unavailable.")?
                        .iter()
                        .filter(|o| o["name"] == name || o["id"] == name || o["key"] == name)
                        .collect::<Vec<_>>();
                    if matches.len() != 1 {
                        return Err(format!("Unknown or ambiguous select option {name}."));
                    }
                    Ok(matches[0]["name"].clone())
                }
                _ => Err(format!(
                    "Property {key} has an unsupported value or format."
                )),
            }
        })
        .collect()
}

pub(super) fn property<'a>(key: &str, properties: &'a [Value]) -> Result<&'a Value, String> {
    let exact = properties
        .iter()
        .filter(|p| {
            ["key", "id", "internal_key"]
                .iter()
                .any(|k| p[*k].as_str() == Some(key))
        })
        .collect::<Vec<_>>();
    if exact.len() == 1 {
        return Ok(exact[0]);
    }
    if exact.len() > 1 {
        return Err(format!("Ambiguous property key {key}."));
    }
    let names = properties
        .iter()
        .filter(|p| p["name"].as_str() == Some(key))
        .collect::<Vec<_>>();
    if names.len() == 1 {
        return Ok(names[0]);
    }
    if names.len() > 1 {
        return Err(format!("Ambiguous property name {key}."));
    }
    let folded = properties
        .iter()
        .filter(|p| {
            ["key", "name"].iter().any(|k| {
                p[*k]
                    .as_str()
                    .is_some_and(|s| normalize(s) == normalize(key))
            })
        })
        .collect::<Vec<_>>();
    if folded.len() == 1 {
        Ok(folded[0])
    } else {
        Err(format!("Unknown or ambiguous property key {key}."))
    }
}

pub(super) fn supplied<'a>(
    prop: &Value,
    body: &'a Value,
    properties: &[Value],
) -> Result<Option<&'a Value>, String> {
    let canonical = prop["key"].as_str().ok_or("Property has no API key.")?;
    let mut values = Vec::new();
    if let Some(fields) = body["properties"].as_object() {
        for (key, value) in fields {
            match property(key, properties) {
                Ok(p) if p["key"] == canonical => values.push(value),
                Err(e) => return Err(e),
                _ => (),
            }
        }
    }
    if canonical == "type" {
        if !values.is_empty() {
            return Err(
                "Object type must be supplied in body.type, never in body.properties.".into(),
            );
        }
        return Ok(body.get("type"));
    }
    if canonical == "name" {
        if let Some(name) = body.get("name") {
            values.push(name);
        }
    }
    if values.len() > 1 {
        return Err(format!(
            "Property {canonical} must be explicitly supplied exactly once."
        ));
    }
    Ok(values.first().copied())
}

fn source_entries<'a>(def: &'a Value, field: &str) -> Result<Vec<&'a str>, String> {
    match def["document"]["query_source"].get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| {
                v.as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| format!("Invalid query source {field}."))
            })
            .collect(),
        _ => Err(format!("Invalid query source {field}.")),
    }
}
fn valid_definition(def: &Value) -> Result<(), String> {
    let source = &def["document"]["query_source"];
    if source.as_object().is_none_or(|o| {
        o.keys()
            .any(|k| !["types", "properties"].contains(&k.as_str()))
    }) {
        return Err("Unsupported query source definition.".into());
    }
    if def["document"]["warnings"]
        .as_array()
        .is_some_and(|a| !a.is_empty())
    {
        return Err("Query definition has export warnings; membership cannot be proven.".into());
    }
    if source_entries(def, "types")?.is_empty() && source_entries(def, "properties")?.is_empty() {
        return Err("Query has no source and matches no objects.".into());
    }
    Ok(())
}

fn type_document(
    client: &Client,
    source: &SourcePermission,
    row: &mut Value,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    if row["_document"].is_object() {
        return Ok(row["_document"].clone());
    }
    let key = row["key"]
        .as_str()
        .ok_or("Type metadata omitted its API key.")?;
    let doc = get(
        client,
        &format!("{}/types/{}?ids=full", base(&source.space_id), encode(key)),
        check,
    )?;
    let id = doc["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Type document omitted its object ID.")?;
    if row["id"].as_str().is_some_and(|old| old != id) {
        return Err("Type identity changed during metadata discovery.".into());
    }
    row["id"] = json!(id);
    row["_document"] = doc.clone();
    Ok(doc)
}
fn load_types(
    client: &Client,
    source: &SourcePermission,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Vec<Value>, String> {
    check()?;
    let mut types = client.pages(
        "GET",
        &format!("{}/types?ids=full", base(&source.space_id)),
        None,
    )?;
    normalize_types(client, source, &mut types, check)?;
    Ok(types)
}
fn normalize_types(
    client: &Client,
    source: &SourcePermission,
    types: &mut Vec<Value>,
    check: &impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    let mut keys = std::collections::BTreeSet::new();
    let mut ids = std::collections::BTreeSet::new();
    for row in types.iter_mut() {
        let key = row["key"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Type metadata omitted its API key.")?
            .to_string();
        if !keys.insert(key) {
            return Err("Duplicate type API keys in metadata.".into());
        }
        if row["id"].as_str().filter(|s| !s.is_empty()).is_none() {
            type_document(client, source, row, check)?;
        }
        if !ids.insert(
            row["id"]
                .as_str()
                .ok_or("Type omitted its object ID.")?
                .to_string(),
        ) {
            return Err("Ambiguous type object IDs in metadata.".into());
        }
    }
    Ok(())
}
fn contains_type_filter(value: &Value, properties: &[Value]) -> bool {
    match value {
        Value::Object(o) => {
            o.get("property")
                .and_then(Value::as_str)
                .is_some_and(|k| property(k, properties).is_ok_and(|p| p["key"] == "type"))
                || o.values().any(|v| contains_type_filter(v, properties))
        }
        Value::Array(a) => a.iter().any(|v| contains_type_filter(v, properties)),
        _ => false,
    }
}
fn query_metadata(
    client: &Client,
    source: &SourcePermission,
    def: &Value,
    types: &mut Vec<Value>,
    check: &impl Fn() -> Result<(), String>,
) -> Result<(Vec<Value>, Value), String> {
    let (mut properties, options) = metadata(client, source, check)?;
    // Legacy servers may serve this legend, but current v2 deliberately removes it.
    // All mappings come from server metadata, never a caller-supplied legend.
    if let Some(legend) = def["document"]["property_internal_keys"].as_object() {
        for prop in &mut properties {
            if let Some(internal) = prop["key"]
                .as_str()
                .and_then(|key| legend.get(key))
                .and_then(Value::as_str)
            {
                prop["internal_key"] = json!(internal);
            }
        }
    }
    let unresolved = |props: &[Value]| -> Result<bool, String> {
        Ok(source_entries(def, "properties")?
            .iter()
            .any(|key| property(key, props).is_err()))
    };
    if (unresolved(&properties)? || contains_type_filter(&def["views"], &properties))
        && types.is_empty()
    {
        *types = load_types(client, source, check)?;
    }
    if unresolved(&properties)? {
        for row in types.iter_mut() {
            let doc = type_document(client, source, row, check)?;
            if let Some(definitions) = doc
                .pointer("/type_settings/property_definitions")
                .and_then(Value::as_array)
            {
                for definition in definitions {
                    if definition["uninstalled"] == true {
                        continue;
                    }
                    let (Some(key), Some(internal)) = (
                        definition["property"].as_str(),
                        definition["internal_key"].as_str(),
                    ) else {
                        continue;
                    };
                    if let Some(prop) = properties.iter_mut().find(|p| p["key"] == key) {
                        if prop["internal_key"]
                            .as_str()
                            .is_some_and(|old| old != internal)
                        {
                            return Err(
                                "Conflicting property identity metadata. Retry discovery.".into()
                            );
                        }
                        prop["internal_key"] = json!(internal);
                    }
                }
            }
            if !unresolved(&properties)? {
                break;
            }
        }
    }
    Ok((properties, options))
}

// This describes whether supplying values can satisfy a supported filter, independently
// of whether the current payload already matches. Preserve the AND/OR tree for callers.
fn filter_requirement(
    node: &Value,
    body: &Value,
    properties: &[Value],
    options: &Value,
    depth: usize,
    types: &[Value],
) -> Value {
    if depth > 32 {
        return json!({"supported":false,"reason":"Filter nesting exceeds the supported depth."});
    }
    if let Some(nodes) = node.as_array() {
        let children = nodes
            .iter()
            .map(|n| filter_requirement(n, body, properties, options, depth + 1, types))
            .collect::<Vec<_>>();
        let supported = children.iter().all(|n| n["supported"] == true);
        return json!({"operator":"and","filters":children,"supported":supported});
    }
    if let Some(operator) = node["operator"].as_str() {
        let valid = matches!(operator, "and" | "or")
            && node.as_object().is_some_and(|o| {
                o.keys()
                    .all(|k| ["operator", "filters", "id"].contains(&k.as_str()))
            });
        let children = node["filters"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|n| filter_requirement(n, body, properties, options, depth + 1, types))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let supported = valid
            && !children.is_empty()
            && if operator == "or" {
                children.iter().any(|n| n["supported"] == true)
            } else {
                children.iter().all(|n| n["supported"] == true)
            };
        return json!({"operator":operator,"filters":children,"supported":supported});
    }
    let key = node["property"].as_str().unwrap_or("");
    let prop = match property(key, properties) {
        Ok(p) => p,
        Err(e) => return json!({"property":key,"supported":false,"reason":e}),
    };
    let canonical = prop["key"].as_str().unwrap_or("");
    let expected = &node["value"];
    let allowed = filter_values(node, prop, options, types);
    let supported = allowed.is_ok();
    json!({"property":canonical,"name":prop["name"],"format":prop["format"],"condition":node["condition"],"required_value":expected,
        "allowed_values":allowed.as_ref().ok(),"reason":allowed.as_ref().err(),"value_location":if canonical == "type" {"body.type"} else {"body.properties"},
        "existing_options":options[canonical],"supported":supported,"satisfied":prove_with_types(node, body, properties, options, 0, types).is_ok()})
}

fn requirements(
    def: &Value,
    body: &Value,
    properties: &[Value],
    options: &Value,
    types: &[Value],
) -> Result<Value, String> {
    let source_properties = source_entries(def, "properties")?.into_iter().map(|key| {
        match property(key, properties) {
            Ok(p) => json!({"property":p["key"],"source_key":key,"name":p["name"],"format":p["format"],"condition":"exists","supported":true,
                "supplied":supplied(p,body,properties).ok().flatten().is_some(),"existing_options":options[p["key"].as_str().unwrap_or("")]}),
            Err(e) => json!({"property":key,"condition":"exists","supported":false,"reason":e}),
        }
    }).collect::<Vec<_>>();
    let empty = json!([]);
    let views = def["views"].as_array().ok_or("Query views unavailable.")?.iter().map(|view| {
        let filters = view.get("filters").unwrap_or(&empty);
        let tree = filter_requirement(filters, body, properties, options, 0, types);
        json!({"id":view["id"],"name":view["name"],"supported":tree["supported"],"filters":tree,
            "satisfied":prove_with_types(filters,body,properties,options,0,types).is_ok(),"reason":prove_with_types(filters,body,properties,options,0,types).err()})
    }).collect::<Vec<_>>();
    Ok(
        json!({"template":"none","query_source":def["document"]["query_source"],"source":{"operator":"or","types":source_entries(def,"types")?,"properties":source_properties},"views":views,
        "membership_rule":"Match at least one source entry AND all conditions in at least one supported view. Property sources require explicit presence, including empty, false, zero or null values. No values are filled automatically."}),
    )
}

fn possible_type_ids(node: &Value, all_ids: &std::collections::BTreeSet<String>) -> Option<std::collections::BTreeSet<String>> {
    if node["supported"] != true { return None; }
    if node["property"] == "type" {
        return Some(node["allowed_values"].as_array()?.iter().filter_map(Value::as_str).map(String::from).collect());
    }
    let children = node["filters"].as_array();
    match node["operator"].as_str() {
        Some("and") => {
            let mut result = all_ids.clone();
            for child in children? {
                let possible = possible_type_ids(child, all_ids)?;
                result.retain(|id| possible.contains(id));
            }
            Some(result)
        }
        Some("or") => {
            let mut result = std::collections::BTreeSet::new();
            let mut supported = false;
            for child in children? {
                if let Some(possible) = possible_type_ids(child, all_ids) {
                    supported = true;
                    result.extend(possible);
                }
            }
            supported.then_some(result)
        }
        _ => Some(all_ids.clone()),
    }
}

pub(super) fn type_candidates(
    client: &Client,
    source: &SourcePermission,
    def: &Value,
    types: &mut Vec<Value>,
    check: &impl Fn() -> Result<(), String>,
) -> Result<std::collections::BTreeSet<String>, String> {
    if valid_definition(def).is_err() { return Ok(Default::default()); }
    normalize_types(client, source, types, check)?;
    let all_ids = types.iter().filter_map(|row| row["id"].as_str().map(String::from)).collect::<std::collections::BTreeSet<_>>();
    let (properties, options) = query_metadata(client, source, def, types, check)?;
    let mut source_ids = std::collections::BTreeSet::new();
    for key in source_entries(def, "types")? {
        if let Ok(ty) = resolve_type(key, types) {
            if let Some(id) = ty["id"].as_str() { source_ids.insert(id.into()); }
        }
    }
    for key in source_entries(def, "properties")? {
        let Ok(prop) = property(key, &properties) else { continue; };
        let Some(canonical) = prop["key"].as_str() else { continue; };
        if matches!(canonical, "name" | "type") {
            source_ids.extend(all_ids.iter().cloned());
            continue;
        }
        for row in types.iter_mut() {
            let doc = type_document(client, source, row, check)?;
            let id = row["id"].as_str().ok_or("Type omitted its object ID.")?;
            let internal = prop["internal_key"].as_str();
            if doc.pointer("/type_settings/property_definitions").and_then(Value::as_array).is_some_and(|definitions| definitions.iter().any(|definition| {
                definition["uninstalled"] != true
                    && (definition["property"] == canonical
                        || prop["id"].as_str().is_some_and(|property_id| definition["property"] == property_id)
                        || internal.is_some_and(|key| definition["internal_key"] == key))
            })) {
                source_ids.insert(id.into());
            }
        }
    }
    let empty = json!([]);
    let mut view_ids = std::collections::BTreeSet::new();
    for view in def["views"].as_array().ok_or("Query views unavailable.")? {
        let filters = view.get("filters").unwrap_or(&empty);
        let tree = filter_requirement(filters, &json!({}), &properties, &options, 0, types);
        if let Some(ids) = possible_type_ids(&tree, &all_ids) { view_ids.extend(ids); }
    }
    Ok(source_ids.intersection(&view_ids).cloned().collect())
}

pub(super) fn discovery(
    client: &Client,
    source: &SourcePermission,
    def: &Value,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    if let Err(e) = valid_definition(def) {
        return Ok(json!({"status":"unsupported","reason":e}));
    }
    let mut types = Vec::new();
    let (properties, options) = query_metadata(client, source, def, &mut types, check)?;
    let mut requirements = requirements(def, &json!({}), &properties, &options, &types)?;
    let source_supported = !source_entries(def, "types")?.is_empty()
        || requirements["source"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["supported"] == true);
    let views_supported = requirements["views"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["supported"] == true);
    requirements["status"] = json!(if source_supported && views_supported {
        "supported"
    } else {
        "unsupported"
    });
    check()?;
    Ok(requirements)
}

pub(super) fn assess_with_type(
    client: &Client,
    source: &SourcePermission,
    def: &Value,
    req: &ContentRequest,
    check: &impl Fn() -> Result<(), String>,
) -> Result<(Option<Value>, String), String> {
    let body = &req.arguments["body"];
    if body["template"] != "none" {
        return Err("Query creation requires template: \"none\".".into());
    }
    if body.as_object().is_none_or(|o| {
        o.keys()
            .any(|k| !["type", "name", "properties", "markdown", "template"].contains(&k.as_str()))
    }) {
        return Err(
            "Scoped query creation initially requires the shortcut creation payload.".into(),
        );
    }
    valid_definition(def)?;
    check()?;
    let mut types = load_types(client, source, check)?;
    let ty = resolve_type(body["type"].as_str().ok_or("Missing body.type.")?, &types)?.clone();
    let canonical_type = ty["key"]
        .as_str()
        .ok_or("Missing type API key.")?
        .to_string();
    if [
        "objectType",
        "object_type",
        "property",
        "relation",
        "set",
        "query",
        "collection",
        "template",
    ]
    .contains(&canonical_type.as_str())
    {
        return Err("Scoped creation cannot create source or schema definitions.".into());
    }
    let source_types = source_entries(def, "types")?;
    let type_matches = source_types
        .iter()
        .any(|key| ty["key"] == *key || ty["id"] == *key);
    let (properties, options) = query_metadata(client, source, def, &mut types, check)?;
    // Resolve every authored property, including aliases, before returning guidance or
    // proving presence. An unknown/ambiguous spelling cannot stand in for a real property.
    if let Some(fields) = body["properties"].as_object() {
        for key in fields.keys() {
            let p = property(key, &properties)?;
            supplied(p, body, &properties)?;
            if matches!(p["format"].as_str(), Some("file" | "files"))
                && !source.permission.files_enabled
            {
                return Err(
                    "File-valued properties require the saved file permission switch.".into(),
                );
            }
        }
    }
    let property_matches = source_entries(def, "properties")?.iter().any(|key| {
        property(key, &properties)
            .is_ok_and(|p| supplied(p, body, &properties).ok().flatten().is_some())
    });
    let mut requirements = requirements(def, body, &properties, &options, &types)?;
    requirements["requested_type"] = body["type"].clone();
    requirements["source"]["satisfied"] = json!(type_matches || property_matches);
    let view_matches = requirements["views"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["satisfied"] == true);
    if (type_matches || property_matches) && view_matches {
        check()?;
        return Ok((None, canonical_type));
    }
    let source_supported = !source_types.is_empty()
        || requirements["source"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["supported"] == true);
    let actionable = source_supported
        && requirements["views"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["supported"] == true);
    check()?;
    Ok((
        Some(
            json!({"outcome":if actionable {"needs_input"} else {"unsupported_query"},
        "message":if actionable {"No object was created. Supply property values or a matching type that satisfy the source and at least one supported view, then resubmit create_object. Ask the user when choices are ambiguous; do not silently replace supplied values."} else {"No object was created. This query has no currently supported, provable creation path; changing values alone may not resolve it."},
        "creation":{"outcome":"not_attempted"},"placement":{"outcome":"not_attempted"},"destination":req.arguments["destination"],
        "idempotency_key":req.arguments["idempotency_key"],"resubmit_tool":"create_object","requirements":requirements}),
        ),
        canonical_type,
    ))
}

#[cfg(test)]
pub(super) fn assess(
    client: &Client,
    source: &SourcePermission,
    def: &Value,
    req: &ContentRequest,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Option<Value>, String> {
    assess_with_type(client, source, def, req, check).map(|r| r.0)
}
