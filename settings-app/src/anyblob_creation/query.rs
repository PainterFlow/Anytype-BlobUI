//! Query creation asks for values before writing. No pending object or edit grant exists.
use super::*;

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

fn query_metadata(
    client: &Client,
    source: &SourcePermission,
    def: &Value,
    check: &impl Fn() -> Result<(), String>,
) -> Result<(Vec<Value>, Value), String> {
    let (mut properties, options) = metadata(client, source, check)?;
    // The server's AnyBlock legend maps served API spellings to stored keys.
    // Never trust a caller-supplied legend for membership or file authorization.
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
) -> Value {
    if depth > 32 {
        return json!({"supported":false,"reason":"Filter nesting exceeds the supported depth."});
    }
    if let Some(nodes) = node.as_array() {
        let children = nodes
            .iter()
            .map(|n| filter_requirement(n, body, properties, options, depth + 1))
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
                    .map(|n| filter_requirement(n, body, properties, options, depth + 1))
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
    let mut supported = node["condition"] == "equal"
        && node.as_object().is_some_and(|o| {
            o.keys()
                .all(|k| ["property", "condition", "value", "id"].contains(&k.as_str()))
        });
    supported &= match prop["format"].as_str() {
        Some("text") => expected
            .as_str()
            .is_some_and(|s| !s.starts_with("_filter_template_")),
        Some("number") => expected.is_number(),
        Some("checkbox") => expected.is_boolean(),
        Some("select") => {
            expected.is_string()
                && options[canonical].as_array().is_some_and(|a| {
                    a.iter().any(|o| {
                        o["name"] == *expected || o["id"] == *expected || o["key"] == *expected
                    })
                })
        }
        _ => false,
    };
    json!({"property":canonical,"name":prop["name"],"format":prop["format"],"condition":node["condition"],"required_value":expected,
        "existing_options":options[canonical],"supported":supported,"satisfied":prove(node, body, properties, options, 0).is_ok()})
}

fn requirements(
    def: &Value,
    body: &Value,
    properties: &[Value],
    options: &Value,
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
        let tree = filter_requirement(filters, body, properties, options, 0);
        json!({"id":view["id"],"name":view["name"],"supported":tree["supported"],"filters":tree,
            "satisfied":prove(filters,body,properties,options,0).is_ok(),"reason":prove(filters,body,properties,options,0).err()})
    }).collect::<Vec<_>>();
    Ok(
        json!({"template":"none","query_source":def["document"]["query_source"],"source":{"operator":"or","types":source_entries(def,"types")?,"properties":source_properties},"views":views,
        "membership_rule":"Match at least one source entry AND all conditions in at least one supported view. Property sources require explicit presence, including empty, false, zero or null values. No values are filled automatically."}),
    )
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
    let (properties, options) = query_metadata(client, source, def, check)?;
    let mut requirements = requirements(def, &json!({}), &properties, &options)?;
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

pub(super) fn assess(
    client: &Client,
    source: &SourcePermission,
    def: &Value,
    req: &ContentRequest,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Option<Value>, String> {
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
    let types = crate::codex::source_types(client, &base(&source.space_id))?;
    let ty = types
        .iter()
        .find(|t| t["key"] == body["type"])
        .ok_or("Requested type cannot be resolved against v2 metadata.")?;
    let source_types = source_entries(def, "types")?;
    let mut type_matches = source_types
        .iter()
        .any(|key| ty["key"] == *key || ty["id"] == *key);
    let (properties, options) = query_metadata(client, source, def, check)?;
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
    // Query sources can use derived internal type references while creates use API keys.
    if !type_matches && !property_matches && source_types.iter().any(|s| s.starts_with("type-")) {
        let key = ty["key"].as_str().ok_or("Type omitted its API key.")?;
        let doc = get(
            client,
            &format!("{}/types/{}?ids=full", base(&source.space_id), encode(key)),
            check,
        )?;
        if let Some(internal) = doc["type_internal_key"].as_str() {
            type_matches = source_types
                .iter()
                .any(|key| *key == internal || *key == format!("type-{internal}"));
        }
    }
    let mut requirements = requirements(def, body, &properties, &options)?;
    requirements["requested_type"] = body["type"].clone();
    requirements["source"]["satisfied"] = json!(type_matches || property_matches);
    let view_matches = requirements["views"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["satisfied"] == true);
    if (type_matches || property_matches) && view_matches {
        check()?;
        return Ok(None);
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
    Ok(Some(
        json!({"outcome":if actionable {"needs_input"} else {"unsupported_query"},
        "message":if actionable {"No object was created. Supply property values or a matching type that satisfy the source and at least one supported view, then resubmit create_object. Ask the user when choices are ambiguous; do not silently replace supplied values."} else {"No object was created. This query has no currently supported, provable creation path; changing values alone may not resolve it."},
        "creation":{"outcome":"not_attempted"},"placement":{"outcome":"not_attempted"},"destination":req.arguments["destination"],
        "idempotency_key":req.arguments["idempotency_key"],"resubmit_tool":"create_object","requirements":requirements}),
    ))
}
