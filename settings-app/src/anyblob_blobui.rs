//! Targeted configuration publication. This module never writes Anytype content.
use crate::{
    anyblob_scope::Scope,
    codex::{Configuration as Permissions, Permission, ProjectConfiguration},
    kanban_api::Client,
    kanban_snapshot::Target,
};
use blobui_mcp::{blobui::BlobuiRequest, Operation};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(crate) fn revision(value: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("JSON serializes"))
    )
}
pub(crate) fn kanban_configuration(install: &Path) -> Result<crate::kanban::Configuration, String> {
    crate::kanban::read_at(
        &install.join("blob/.blobui-kanban-formulas.json"),
        &install.join(".blobui-kanban-formulas.json"),
        &install.join(".blobui-formula-prototype.json"),
    )
}

pub(crate) fn calendar_revision(data: &Value) -> Result<String, String> {
    Ok(revision(&json!(crate::calendar::configuration(data)?)))
}
fn content_request(
    space: &str,
    object: Option<&str>,
    write: bool,
) -> blobui_mcp::content::ContentRequest {
    blobui_mcp::content::ContentRequest {
        tool: if object.is_some() {
            if write {
                "patch_object"
            } else {
                "read_document"
            }
        } else {
            if write {
                "update_property"
            } else {
                "get_schema"
            }
        }
        .into(),
        arguments: json!({"space_id":space,"object_id":object}),
    }
}
fn permission(
    config: &Permissions,
    scope: Option<&Scope>,
    project: Option<&ProjectConfiguration>,
    space: &str,
    object: Option<&str>,
    write: bool,
) -> Result<Permission, String> {
    let capability = if write { "write" } else { "read" };
    let mut grants = Vec::new();
    if let Some(scope) = scope {
        if let Some(p) = if object.is_none() {
            scope.whole_space_permission(space, write)
        } else {
            scope.content_permission(&content_request(space, object, write), false)
        } {
            grants.push(p);
        }
        // Source objects are configuration targets even when they are not members of themselves.
        if let (Some(project), Some(object)) = (project, object) {
            grants.extend(
                project
                    .sources
                    .iter()
                    .filter(|s| {
                        s.space_id == space
                            && s.id == object
                            && matches!(s.kind.as_str(), "query" | "collection" | "type")
                    })
                    .map(|s| s.permission.clone()),
            );
        }
    } else if let Some(p) = config.spaces.get(space) {
        grants.push(p.clone());
    }
    grants.retain(|p| p.permits(capability));
    let mut p = grants
        .first()
        .cloned()
        .ok_or("Access denied by saved BlobUI permissions for this target.")?;
    if grants.iter().any(|p| p.approval == "alwaysAsk") {
        p.approval = "alwaysAsk".into();
    }
    Ok(p)
}
fn objects(
    client: &Client,
    config: &Permissions,
    scope: Option<&Scope>,
    project: Option<&ProjectConfiguration>,
    space: &str,
    selected: Option<&str>,
    check: &impl Fn() -> Result<(), String>,
) -> Result<Vec<String>, String> {
    if let Some(id) = selected {
        permission(config, scope, project, space, Some(id), false)?;
        return Ok(vec![id.into()]);
    }
    if let Some(scope) = scope {
        let mut ids = scope.object_ids(space);
        if let Some(project) = project {
            ids.extend(
                project
                    .sources
                    .iter()
                    .filter(|s| {
                        s.space_id == space && s.kind != "space" && s.permission.permits("read")
                    })
                    .map(|s| s.id.clone()),
            );
        }
        // A whole-space source has no enumerated object list.
        if permission(config, Some(scope), project, space, None, true).is_err()
            && !scope.has_whole_space(space)
        {
            ids.sort();
            ids.dedup();
            return Ok(ids);
        }
    }
    permission(config, scope, project, space, None, false)?;
    check()?;
    let rows = client.pages(
        "POST",
        &format!("/v2/spaces/{}/search?ids=full", enc(space)),
        Some(&json!({})),
    )?;
    rows.iter().map(|r| required(r, "id")).collect()
}
fn enc(value: &str) -> String {
    crate::api::encode_segment(value)
}
fn required(value: &Value, key: &str) -> Result<String, String> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| format!("Anytype omitted {key}."))
}
fn target(request: &BlobuiRequest) -> Result<Option<Target>, String> {
    request
        .arguments
        .get("target")
        .map(|v| serde_json::from_value(v.clone()).map_err(|_| "Invalid view target.".into()))
        .transpose()
}
fn calendar_type(client: &Client, space: &str, key: &str) -> Result<(Value, Vec<Value>), String> {
    let base = format!("/v2/spaces/{}", enc(space));
    let document = client.request("GET", &format!("{base}/types/{}?ids=full", enc(key)), None)?;
    let properties = client.pages("GET", &format!("{base}/properties?ids=full"), None)?;
    let eligible = crate::calendar::type_properties(&document, &properties);
    Ok((document, eligible))
}
fn legacy_color(object: &Value, key: Option<&str>) -> Result<Option<String>, String> {
    let Some(key) = key else {
        return Ok(None);
    };
    let Some(raw) = crate::api::object_property(object, key, "Calendar Color") else {
        return Ok(None);
    };
    let Some(value) = crate::api::property_value(raw)? else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let color = value.as_str().ok_or("Calendar Color must be text.")?;
    if color.trim().is_empty() || color.trim().eq_ignore_ascii_case("default") {
        return Ok(None);
    }
    let color = crate::api::canonical_calendar_color(color)?;
    crate::css::validate_calendar_color(&color)?;
    Ok(Some(color))
}
struct CalendarEvaluation {
    base: Value,
    overrides: Vec<Value>,
    objects: Vec<String>,
    warnings: Vec<String>,
}
fn evaluate_calendar(
    client: &Client,
    space: &str,
    key: &str,
    selected: Option<&Target>,
    config: &crate::calendar::Configuration,
    authorize: &impl Fn(&str) -> Result<(), String>,
) -> Result<CalendarEvaluation, String> {
    let (document, properties) = calendar_type(client, space, key)?;
    let entry = config
        .types
        .iter()
        .find(|t| t.space_id == space && t.type_key == key);
    let inherited = crate::calendar::Rule::Inherit;
    let default = entry.map(|e| &e.default).unwrap_or(&inherited);
    let overrides = entry.map(|e| e.overrides.as_slice()).unwrap_or(&[]);
    let mut options = BTreeMap::new();
    let mut warnings = Vec::new();
    for rule in std::iter::once(default).chain(overrides.iter().map(|o| &o.rule)) {
        if let crate::calendar::Rule::Property { key } = rule {
            let property = properties
                .iter()
                .find(|p| p["key"] == *key)
                .ok_or("The calendar property is missing or has an unsupported format.")?;
            if property["format"] == "select" && !options.contains_key(key) {
                options.insert(
                    key.clone(),
                    client.pages(
                        "GET",
                        &format!(
                            "/v2/spaces/{}/properties/{}/options?ids=full",
                            enc(space),
                            enc(key)
                        ),
                        None,
                    )?,
                );
            }
        }
    }
    let all_properties = client.pages(
        "GET",
        &format!("/v2/spaces/{}/properties?ids=full", enc(space)),
        None,
    )?;
    let legacy = all_properties
        .iter()
        .filter(|p| {
            p["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("Calendar Color"))
        })
        .collect::<Vec<_>>();
    if legacy.len() > 1 {
        return Err(
            "More than one Calendar Color property exists. Rename the duplicate before publishing."
                .into(),
        );
    }
    let legacy_key = legacy.first().and_then(|p| p["key"].as_str());
    let api_key = document
        .pointer("/type_settings/api_key")
        .and_then(Value::as_str)
        .or_else(|| document["internal_key"].as_str())
        .unwrap_or(key);
    let mut fields = properties
        .iter()
        .filter_map(|p| p["key"].as_str().map(String::from))
        .collect::<Vec<_>>();
    if let Some(key) = legacy_key {
        fields.push(key.into());
    }
    fields.sort();
    fields.dedup();
    let rows = if let Some(target) = selected {
        client.calendar_members(target, &fields, authorize)?
    } else {
        client.pages(
            "POST",
            &format!("/v2/spaces/{}/search?ids=full", enc(space)),
            Some(&json!({"type":api_key,"fields":fields})),
        )?
    };
    let mut base = json!({});
    let mut scoped = Vec::new();
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    for object in rows {
        let id = required(&object, "id")?;
        authorize(&id)?;
        if !seen.insert(id.clone()) {
            return Err(
                "Calendar membership changed during pagination. Reread before retrying.".into(),
            );
        }
        // View endpoints can contain other types. Never color those using this type's rule.
        if selected.is_some() {
            let actual = object["type"]
                .as_str()
                .or_else(|| object["type"]["key"].as_str())
                .or_else(|| object["type"]["id"].as_str())
                .ok_or("Calendar member has no type identity.")?;
            if actual != key && actual != api_key && Some(actual) != document["id"].as_str() {
                continue;
            }
        }
        ids.push(id.clone());
        let fallback = legacy_color(&object, legacy_key)?;
        let resolved = crate::calendar::resolve(default, &object, &properties, &options)
            .unwrap_or_else(|| fallback.clone());
        base[&id] = json!(resolved);
        for item in overrides
            .iter()
            .filter(|o| selected.is_none_or(|t| o.target == *t))
        {
            let color = crate::calendar::resolve(&item.rule, &object, &properties, &options)
                .unwrap_or_else(|| resolved.clone());
            if !matches!(item.rule, crate::calendar::Rule::Inherit) {
                scoped.push(json!({"target":item.target,"objectId":id,"color":color}));
            }
        }
        if let Some(target) = selected {
            if !overrides
                .iter()
                .any(|o| o.target == *target && !matches!(o.rule, crate::calendar::Rule::Inherit))
            {
                scoped.push(json!({"target":target,"objectId":id,"color":resolved}));
            }
        }
        for rule in std::iter::once(default).chain(
            overrides
                .iter()
                .filter(|o| selected.is_none_or(|t| o.target == *t))
                .map(|o| &o.rule),
        ) {
            if matches!(rule, crate::calendar::Rule::Property { .. })
                && crate::calendar::resolve(rule, &object, &properties, &options).is_none()
            {
                warnings.push(format!("Object '{id}' uses its fallback color because its property value or option color is unavailable."));
            }
        }
        if ids.len() > 100_000 || scoped.len() > 100_000 {
            return Err("Calendar snapshot exceeds the publication limit.".into());
        }
    }
    warnings.extend(client.take_info());
    Ok(CalendarEvaluation {
        base,
        overrides: scoped,
        objects: ids,
        warnings,
    })
}
fn edit_calendar(
    config: &mut crate::calendar::Configuration,
    space: &str,
    key: &str,
    selected: Option<&Target>,
    rule: Option<crate::calendar::Rule>,
) {
    let index = config
        .types
        .iter()
        .position(|e| e.space_id == space && e.type_key == key);
    if index.is_none() && rule.is_none() {
        return;
    }
    let index = index.unwrap_or_else(|| {
        config.types.push(crate::calendar::TypeRule {
            space_id: space.into(),
            type_key: key.into(),
            default: crate::calendar::Rule::Inherit,
            overrides: Vec::new(),
        });
        config.types.len() - 1
    });
    let entry = &mut config.types[index];
    if let Some(t) = selected {
        entry.overrides.retain(|o| o.target != *t);
        if let Some(rule) = rule {
            entry.overrides.push(crate::calendar::Override {
                target: t.clone(),
                rule,
            });
        }
    } else {
        entry.default = rule.unwrap_or_default();
    }
}
pub(crate) fn execute_at(
    install: &Path,
    client: &Client,
    config: &Permissions,
    scope: Option<&Scope>,
    project: Option<&ProjectConfiguration>,
    request: &BlobuiRequest,
    check: &impl Fn() -> Result<(), String>,
    approve: &impl Fn(&Operation, &dyn Fn() -> Result<(), String>) -> Result<(), String>,
) -> Result<Value, String> {
    request.validate()?;
    let space = request.arguments["space_id"]
        .as_str()
        .ok_or("Missing space ID.")?;
    let selected = target(request)?;
    let object = selected
        .as_ref()
        .map(|t| t.object_id.as_str())
        .or_else(|| request.arguments["object_id"].as_str());
    // Calendar type defaults always need a whole-space grant, including previews.
    let wide_calendar =
        request.calendar() && !request.tool.starts_with("list_") && selected.is_none();
    let p = permission(
        config,
        scope,
        project,
        space,
        if wide_calendar { None } else { object },
        request.mutates() || (wide_calendar && !request.tool.starts_with("get_")),
    )
    .or_else(|error| {
        if request.tool.starts_with("list_") && object.is_none() {
            scope
                .and_then(|s| s.content_permission(&content_request(space, None, false), false))
                .ok_or(error)
        } else {
            Err(error)
        }
    })?;
    let authorize = |id: &str| {
        check()?;
        permission(config, scope, project, space, Some(id), false).map(|_| ())
    };
    // Conservatively honor approvals on every source contributing data.
    let ask = p.approval == "alwaysAsk"
        || project.is_some_and(|p| {
            p.sources.iter().any(|s| {
                s.space_id == space
                    && s.permission.permits("read")
                    && s.permission.approval == "alwaysAsk"
            })
        });
    if ask {
        approve(
            &Operation::Blobui {
                blobui: request.clone(),
            },
            check,
        )?;
    }
    check()?;
    if request.tool.starts_with("list_") {
        let ids = objects(client, config, scope, project, space, object, check)?;
        let result = if request.calendar() {
            let mut views = Vec::new();
            for id in &ids {
                authorize(id)?;
                let doc = client.calendar_document(&format!(
                    "/v2/spaces/{}/objects/{}?ids=full&include=blocks,views",
                    enc(space),
                    enc(id)
                ))?;
                views.extend(crate::calendar::calendar_views(space, space, &doc));
            }
            let types = client.pages(
                "GET",
                &format!("/v2/spaces/{}/types?ids=full", enc(space)),
                None,
            )?;
            let mut targets = Vec::new();
            for row in types {
                check()?;
                let key = required(&row, "key")?;
                let (doc, properties) = calendar_type(client, space, &key)?;
                if let Some(id) = doc["id"].as_str() {
                    if permission(config, scope, project, space, Some(id), false).is_ok()
                        && object.is_none_or(|selected| selected == id)
                    {
                        views.extend(crate::calendar::calendar_views(space, space, &doc));
                    }
                }
                targets.push(json!({"type_key":key,"name":doc["name"],"properties":properties,"default_requires_space_read_write":true}));
            }
            json!({"types":targets,"views":views,"warnings":client.take_info()})
        } else {
            let mut views = client.discover_targeted(space, &ids, &authorize)?;
            if object.is_none() && permission(config, scope, project, space, None, false).is_ok() {
                views.extend(client.discover_type_views_in_space(space, &authorize)?);
            }
            let mut seen = BTreeSet::new();
            views.retain(|v| seen.insert(v.target.clone()));
            json!({"views":views,"supported_tokens":[{"kind":"count"},{"kind":"aggregate","property":"stable-property-key","operation":"total|average|median|minimum|maximum","calculation":null},{"kind":"property","property":"stable-property-key"},{"kind":"number","value":"1"},{"kind":"boolean","value":true},{"kind":"operator","value":"+|-|*|/|^|>|<|>=|<=|==|!=|&&, ||, !|?|:"},{"kind":"function","value":"sqrt|root"},{"kind":"open"},{"kind":"close"},{"kind":"comma"}],"warnings":client.take_info()})
        };
        check()?;
        return Ok(result);
    }
    let _guard = if request.mutates() && request.arguments["dry_run"] != true {
        Some(crate::acquire_save_guard(
            "Wait for the current Settings save to finish.",
        )?)
    } else {
        None
    };
    let data_path = install.join("blob/data.json");
    let mut data = crate::read_json_file(&data_path)?;
    let old_data = data.clone();
    let mut kanban = if request.calendar() {
        crate::kanban::Configuration::default()
    } else {
        kanban_configuration(install)?
    };
    let mut calendar = if request.calendar() {
        crate::calendar::configuration(&data)?
    } else {
        crate::calendar::Configuration::default()
    };
    let before = if request.calendar() {
        json!(calendar)
    } else {
        json!(kanban)
    };
    let current = revision(&before);
    if request.mutates() && request.arguments["revision"] != current {
        return Err(
            "Configuration revision conflict. Reread the saved configuration and preview again."
                .into(),
        );
    }
    let saved = if request.calendar() {
        let key = request.arguments["type_key"]
            .as_str()
            .ok_or("Missing type key.")?;
        calendar
            .types
            .iter()
            .find(|e| e.space_id == space && e.type_key == key)
            .map(|e| {
                if let Some(t) = &selected {
                    e.overrides
                        .iter()
                        .find(|o| o.target == *t)
                        .map(|o| json!(o.rule))
                        .unwrap_or(Value::Null)
                } else {
                    json!(e.default)
                }
            })
            .unwrap_or(Value::Null)
    } else {
        let t = selected.as_ref().ok_or("Missing Kanban target.")?;
        kanban
            .definitions
            .iter()
            .find(|d| d.target == *t)
            .map(|d| json!(d.tokens))
            .unwrap_or(Value::Null)
    };
    if request.tool.starts_with("refresh_") && saved.is_null() {
        return Err(
            "No saved configuration to refresh. Read and configure this target first.".into(),
        );
    }
    let publication_key = revision(&json!([
        space,
        request.calendar(),
        request.arguments.get("type_key"),
        selected
    ]));
    let publication = data
        .pointer("/blobuiPublications")
        .and_then(|v| v.get(&publication_key))
        .cloned()
        .unwrap_or(Value::Null);
    let legacy_stamp = if request.calendar() {
        data["calendarColorsPublishedAt"].clone()
    } else {
        data.pointer("/kanbanFormulaResults/calculatedAt")
            .cloned()
            .unwrap_or(Value::Null)
    };
    let stamp = if publication["revision"] == current {
        publication
            .get("published_at")
            .cloned()
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    if request.tool.starts_with("get_") {
        // Inspect only the selected target's metadata, without evaluating saved results.
        let properties = if request.calendar() {
            calendar_type(
                client,
                space,
                request.arguments["type_key"].as_str().unwrap(),
            )?
            .1
            .into_iter()
            .collect::<Vec<_>>()
        } else {
            json!(client.discover_targeted(
                space,
                &[selected.as_ref().unwrap().object_id.clone()],
                &authorize
            )?)
            .as_array()
            .cloned()
            .unwrap_or_default()
        };
        let mut warnings = publication["warnings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        warnings.extend(client.take_info().into_iter().map(Value::String));
        if stamp.is_null() && !legacy_stamp.is_null() {
            warnings.push(json!("Only a legacy whole-snapshot timestamp is available; this target's publication time is unknown."));
        }
        check()?;
        return Ok(
            json!({"configuration":saved,"revision":current,"published_at":stamp,"legacy_snapshot_published_at":legacy_stamp,"cached":true,"properties":properties,"warnings":warnings}),
        );
    }
    let remove = request.tool.starts_with("remove_");
    let change = request.tool.starts_with("set_") || request.tool.starts_with("preview_") || remove;
    let now = chrono::Utc::now().to_rfc3339();
    let (preview, mut warnings) = if request.calendar() {
        let key = request.arguments["type_key"].as_str().unwrap();
        if change {
            let rule = if remove {
                None
            } else {
                Some(
                    serde_json::from_value(request.arguments["rule"].clone())
                        .map_err(|_| "Invalid calendar rule.")?,
                )
            };
            edit_calendar(&mut calendar, space, key, selected.as_ref(), rule);
        }
        calendar.validate()?;
        let evaluated =
            evaluate_calendar(client, space, key, selected.as_ref(), &calendar, &authorize)?;
        if !data["calendarObjectColors"].is_object() {
            data["calendarObjectColors"] = json!({});
        }
        if selected.is_none() {
            for id in &evaluated.objects {
                if evaluated.base[id].is_null() {
                    data["calendarObjectColors"]
                        .as_object_mut()
                        .unwrap()
                        .remove(id);
                } else {
                    data["calendarObjectColors"][id] = evaluated.base[id].clone();
                }
            }
        }
        let mut colors = data["calendarViewColors"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        colors.retain(|v| {
            if let Some(t) = &selected {
                v["target"] != json!(t) || !evaluated.objects.iter().any(|id| v["objectId"] == *id)
            } else {
                !evaluated.objects.iter().any(|id| v["objectId"] == *id)
            }
        });
        colors.extend(evaluated.overrides.clone());
        data["calendarViewColors"] = json!(colors);
        data["calendarColorRules"] = json!(calendar);
        data["calendarColorsPublishedAt"] = json!(now);
        (
            json!({"base":evaluated.base,"overrides":evaluated.overrides}),
            evaluated.warnings,
        )
    } else {
        let t = selected.as_ref().unwrap();
        let tokens = if change && !remove {
            Some(
                serde_json::from_value::<Vec<crate::calculator::Token>>(
                    request.arguments["tokens"].clone(),
                )
                .map_err(|_| "Invalid calculator tokens.")?,
            )
        } else if remove {
            None
        } else {
            kanban
                .definitions
                .iter()
                .find(|d| d.target == *t)
                .map(|d| d.tokens.clone())
                .ok_or("No saved calculation to refresh.")
                .map(Some)?
        };
        let results = if let Some(tokens) = &tokens {
            client.calculate_authorized(
                t,
                &crate::calculator::Calculator::parse(tokens)?,
                &authorize,
            )?
        } else {
            Vec::new()
        };
        if change {
            kanban.definitions.retain(|d| d.target != *t);
            if let Some(tokens) = tokens {
                kanban.definitions.push(crate::kanban::Definition {
                    target: t.clone(),
                    tokens,
                });
            }
        }
        kanban.validate()?;
        let mut snapshot = if data["kanbanFormulaResults"].is_null() {
            crate::kanban::Snapshot {
                version: 2,
                display: kanban.display.clone(),
                calculated_at: now.clone(),
                results: Vec::new(),
            }
        } else {
            serde_json::from_value(data["kanbanFormulaResults"].clone())
                .map_err(|_| "Saved Kanban snapshot is malformed.")?
        };
        snapshot.results.retain(|r| r.target != *t);
        snapshot.results.extend(results.clone());
        snapshot.calculated_at = now.clone();
        data["kanbanFormulaResults"] = json!(snapshot);
        (json!(results), client.take_info())
    };
    if request.calendar()
        && old_data.pointer("/settings/calendarColorsEnabled") == Some(&json!(false))
    {
        warnings.push(
            "The Calendar Colors switch is off. The snapshot is saved but colors remain disabled."
                .into(),
        );
    }
    let next = revision(&if request.calendar() {
        json!(calendar)
    } else {
        json!(kanban)
    });
    check()?;
    if !request.mutates() || request.arguments["dry_run"] == true {
        return Ok(
            json!({"outcome":"preview","configuration":saved,"revision":current,"published_at":stamp,"results":preview,"warnings":warnings,"publication_status":"unchanged","manual_reload_needed":false}),
        );
    }
    if !data["blobuiPublications"].is_object() {
        data["blobuiPublications"] = json!({});
    }
    data["blobuiPublications"][&publication_key] =
        json!({"published_at":now,"revision":next,"warnings":warnings});
    // Compile before committing anything. API and CSS errors preserve every prior file.
    let (_, _, css_path, _) = crate::save_paths(install)?;
    let runtime = crate::resolve_runtime_directory_for_install(install)?;
    let css = crate::css::compile(&runtime, install, &data)?;
    check()?;
    if crate::read_json_file(&data_path)? != old_data
        || (!request.calendar() && revision(&json!(kanban_configuration(install)?)) != current)
    {
        return Err("Configuration changed during publication. Reread before retrying.".into());
    }
    use crate::storage::transaction::{commit_update, FileChange};
    let mut changes = vec![
        FileChange {
            path: data_path,
            contents: Some(crate::serialize_data(&data)?),
        },
        FileChange {
            path: css_path,
            contents: Some(css.into_bytes()),
        },
    ];
    if !request.calendar() {
        changes.push(FileChange {
            path: install.join("blob/.blobui-kanban-formulas.json"),
            contents: Some(
                serde_json::to_vec_pretty(&kanban)
                    .map_err(|_| "Could not serialize calculations.")?,
            ),
        });
    }
    check()?;
    commit_update(changes)?;
    Ok(
        json!({"outcome":"confirmed","revision":next,"published_at":now,"publication_status":"published","manual_reload_needed":true,"warnings":warnings}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };
    struct Fixture {
        root: std::path::PathBuf,
        client: Client,
        stop: Arc<AtomicBool>,
        fail: Arc<AtomicBool>,
        server: Option<std::thread::JoinHandle<()>>,
        requests: Arc<AtomicUsize>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            self.server.take().unwrap().join().unwrap();
        }
    }
    impl Fixture {
        fn new() -> Self { Self::with_grouping("direct") }
        fn with_grouping(mode: &'static str) -> Self {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(".test-run")
                .join(format!(
                    "blobui-tools-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
            fs::create_dir_all(root.join("blob")).unwrap();
            let mut data = json!({"installDirectory":root,"unrelated":{"preserve":true},"calendarColorRules":{"version":1,"types":[]},"calendarObjectColors":{"unrelated":"Pink"},"calendarViewColors":[{"target":{"spaceId":"other","objectId":"other","blockId":"other","viewId":"other"},"objectId":"other","color":"Blue"}]});
            crate::apply_settings(&mut data, &root, &crate::SettingsDraft::default());
            fs::write(
                root.join("blob/data.json"),
                serde_json::to_vec(&data).unwrap(),
            )
            .unwrap();
            fs::write(root.join("custom.css"), b"previous css").unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let stop = Arc::new(AtomicBool::new(false));
            let fail = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(AtomicUsize::new(0));
            let (st, fl, req) = (stop.clone(), fail.clone(), requests.clone());
            let server = std::thread::spawn(move || {
                while !st.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    let mut buf = [0; 8192];
                    loop {
                        let n = stream.read(&mut buf).unwrap();
                        if n == 0 {
                            break;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let h = String::from_utf8_lossy(&bytes[..end]);
                            let len = h
                                .lines()
                                .find_map(|s| {
                                    s.to_ascii_lowercase()
                                        .strip_prefix("content-length: ")
                                        .and_then(|s| s.parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + len {
                                break;
                            }
                        }
                    }
                    let text = String::from_utf8(bytes).unwrap();
                    let path = text
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    assert!(
                        path.starts_with("/v2/spaces/space/"),
                        "unscoped endpoint {path}"
                    );
                    req.fetch_add(1, Ordering::SeqCst);
                    stream.set_nonblocking(false).unwrap();
                    let mut view = json!({"id":"view","type":"kanban","name":"Board","group_by":"status","filters":[],"sorts":[],"groups":[{"id":"option-a"},{"id":"empty"}],"columns":[{"property":"estimate"},{"property":"status"}]});
                    if mode == "missing" { view["groups"] = json!([]); }
                    if mode == "checkbox" { view["group_by"] = json!("done"); }
                    let cal = json!({"id":"calendar","type":"calendar","name":"Calendar","filters":[],"sorts":[]});
                    let page = |v: Value| json!({"data":v,"has_more":false});
                    let value = if path.contains("/properties/status/options") {
                        page(json!([if mode == "direct" {json!({"id":"option-a","name":"A","color":"red"})} else {json!({"name":"A","color":"red"})}]))
                    } else if path.contains("/objects/option-a") {
                        json!({"id":"option-a","properties":{"name":"A"}})
                    } else if path.contains("/properties") {
                        if mode == "multi" {
                            page(json!([{"key":"status","name":"Status","format":"multi_select"},{"key":"estimate","name":"Estimate","format":"number"}]))
                        } else { page(
                            json!([{"key":"status","name":"Status","format":"select"},{"key":"estimate","name":"Estimate","format":"number"},{"key":"done","name":"Done","format":"checkbox"},{"key":"legacy","name":"Calendar Color","format":"text"}]),
                        )
                        }
                    } else if path.contains("/types/task") {
                        json!({"id":"type-task","name":"Task","kind":"object_type","internal_key":"task","type_settings":{"api_key":"task","property_definitions":[{"property":"status"},{"property":"done"}]},"blocks":[{"id":"board","properties":[{"property":"estimate","format":"number"},{"property":"status","format":"select"}],"views":[view,cal]}]})
                    } else if path.contains("/types?") {
                        page(json!([{"id":"type-task","key":"task","name":"Task"}]))
                    } else if path.contains("/views") {
                        if mode == "document" { view["groups"] = json!([]); }
                        page(json!([view, cal]))
                    } else if path.contains("/objects/query") {
                        if mode == "endpoint" { view["groups"] = json!([]); }
                        json!({"id":"query","name":"Tasks","type":"query","blocks":[{"id":"board","object_id":"query","properties":[{"property":"estimate","format":"number"},{"property":"status","format":"select"}],"views":[view,cal]}]})
                    } else if path.contains("/search") || path.contains("/queries/query/objects") {
                        page(
                            json!([{"id":"card","type":"task","properties":{"estimate":5,"status":"A","done":true,"legacy":{"text":"Blue"}}},{"id":"card-2","type":"task","properties":{"estimate":3,"status":null,"done":false,"legacy":"Pink"}}]),
                        )
                    } else {
                        panic!("Unexpected fixture endpoint {path}")
                    };
                    let body = serde_json::to_vec(&value).unwrap();
                    let status = if fl.load(Ordering::SeqCst) {
                        "500 Internal Server Error"
                    } else {
                        "200 OK"
                    };
                    write!(
                        stream,
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .unwrap();
                    stream.write_all(&body).unwrap();
                }
            });
            Self {
                root,
                client: Client::with_connection("fixture", "127.0.0.1", port),
                stop,
                fail,
                server: Some(server),
                requests,
            }
        }
        fn files(&self) -> Vec<Vec<u8>> {
            [
                "blob/data.json",
                "custom.css",
                "blob/.blobui-kanban-formulas.json",
            ]
            .iter()
            .map(|p| fs::read(self.root.join(p)).unwrap_or_default())
            .collect()
        }
        fn call(
            &self,
            name: &str,
            args: Value,
            access: &str,
            check: &impl Fn() -> Result<(), String>,
        ) -> Result<Value, String> {
            let config = crate::codex::normalize(
                &json!({"spaces":{"space":{"access":access,"approval":"fullAccess"}}}),
            );
            execute_at(
                &self.root,
                &self.client,
                &config,
                None,
                None,
                &BlobuiRequest {
                    tool: name.into(),
                    arguments: args,
                },
                check,
                &|_, check| check(),
            )
        }
        fn kanban_args(&self) -> Value {
            json!({"space_id":"space","target":{"spaceId":"space","objectId":"query","blockId":"board","viewId":"view"},"tokens":[{"kind":"aggregate","property":"estimate","operation":"total"}],"revision":revision(&json!(kanban_configuration(&self.root).unwrap()))})
        }
        fn calendar_args(&self) -> Value {
            let d = crate::read_json_file(&self.root.join("blob/data.json")).unwrap();
            json!({"space_id":"space","type_key":"task","rule":{"mode":"property","key":"done"},"revision":calendar_revision(&d).unwrap()})
        }
    }
    #[test]
    fn kanban_connector_identity_sources_and_failures_preserve_files() {
        for mode in ["direct", "document", "endpoint", "checkbox", "missing", "multi"] {
            let f = Fixture::with_grouping(mode);
            if mode == "multi" {
                let discovery = f.call("list_kanban_views", json!({"space_id":"space","object_id":"query"}), "read", &|| Ok(())).unwrap();
                assert!(discovery.to_string().contains("unsupported_grouping_multi_select"));
            }
            let before = f.files();
            for tool in ["preview_kanban_calculation", "set_kanban_calculation"] {
                let mut args = f.kanban_args();
                if tool == "set_kanban_calculation" { args["dry_run"] = json!(true); }
                else { args.as_object_mut().unwrap().remove("revision"); }
                let result = f.call(tool, args, "readWrite", &|| Ok(()));
                if mode == "missing" || mode == "multi" {
                    let error = result.unwrap_err();
                    assert!(error.contains(if mode == "multi" {"unsupported_grouping_multi_select"} else {"unresolved_category_identity"}), "{error}");
                } else {
                    let preview = result.unwrap_or_else(|error| panic!("{mode}: {error}"));
                    let results = preview["results"].as_array().unwrap();
                    assert!(results.iter().any(|result| result["value"].as_f64() == Some(5.0)));
                    assert!(results.iter().any(|result| result["categoryId"] == if mode == "checkbox" {"false"} else {"empty"}));
                }
                assert_eq!(f.files(), before);
            }
            if !["multi", "missing"].contains(&mode) {
                let mut args = f.kanban_args();
                args.as_object_mut().unwrap().remove("revision");
                args["tokens"] = json!([{"kind":"count"}]);
                let preview = f.call("preview_kanban_calculation", args, "read", &|| Ok(())).unwrap();
                assert_eq!(preview["results"].as_array().unwrap().len(), 2);
                assert!(preview["results"].as_array().unwrap().iter().all(|result| result["value"].as_f64() == Some(1.0)));
                assert_eq!(f.files(), before);
            }
            if mode == "multi" || mode == "missing" {
                let args = f.kanban_args();
                fs::write(f.root.join("blob/.blobui-kanban-formulas.json"), json!({"version":2,"display":"balanced","definitions":[{"target":args["target"],"tokens":args["tokens"]}]}).to_string()).unwrap();
                let before = f.files();
                let mut refresh = f.kanban_args();
                refresh.as_object_mut().unwrap().remove("tokens");
                let error = f.call("refresh_kanban_calculation", refresh, "readWrite", &|| Ok(())).unwrap_err();
                assert!(error.contains(if mode == "multi" {"unsupported_grouping_multi_select"} else {"unresolved_category_identity"}), "{error}");
                assert_eq!(f.files(), before);
            }
        }
    }

    #[test]
    fn isolated_kanban_preview_publish_conflict_and_rollback() {
        let f = Fixture::new();
        let before = f.files();
        let mut args = f.kanban_args();
        args["dry_run"] = json!(true);
        let preview = f
            .call("set_kanban_calculation", args.clone(), "readWrite", &|| {
                Ok(())
            })
            .unwrap();
        assert_eq!(preview["outcome"], "preview");
        assert_eq!(f.files(), before);
        assert!(preview["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["value"].as_f64() == Some(5.0)));
        args["dry_run"] = json!(false);
        let result = f
            .call("set_kanban_calculation", args.clone(), "readWrite", &|| {
                Ok(())
            })
            .unwrap();
        assert_eq!(result["publication_status"], "published");
        assert_eq!(result["manual_reload_needed"], true);
        let saved = f.files();
        assert!(f
            .call("set_kanban_calculation", args, "readWrite", &|| Ok(()))
            .unwrap_err()
            .contains("revision conflict"));
        assert_eq!(saved, f.files());
        let mut args = f.kanban_args();
        args["tokens"] = json!([{"kind":"count"}]);
        crate::storage::FAIL_AFTER_REPLACE
            .with(|fail| *fail.borrow_mut() = Some(f.root.join("custom.css")));
        assert!(f
            .call("set_kanban_calculation", args.clone(), "readWrite", &|| Ok(
                ()
            ))
            .is_err());
        assert_eq!(saved, f.files());
        f.fail.store(true, Ordering::SeqCst);
        assert!(f
            .call("set_kanban_calculation", args, "readWrite", &|| Ok(()))
            .is_err());
        assert_eq!(saved, f.files());
    }
    #[test]
    fn isolated_calendar_inheritance_none_and_unrelated_entries() {
        let f = Fixture::new();
        let before = f.files();
        let mut args = f.calendar_args();
        args["dry_run"] = json!(true);
        let p = f
            .call(
                "set_calendar_color_rule",
                args.clone(),
                "readWrite",
                &|| Ok(()),
            )
            .unwrap();
        assert_eq!(p["results"]["base"]["card"], "Green");
        assert_eq!(p["results"]["base"]["card-2"], "Grey");
        assert_eq!(f.files(), before);
        args["dry_run"] = json!(false);
        f.call("set_calendar_color_rule", args, "readWrite", &|| Ok(()))
            .unwrap();
        let d = crate::read_json_file(&f.root.join("blob/data.json")).unwrap();
        assert_eq!(d["unrelated"]["preserve"], true);
        assert_eq!(d["calendarObjectColors"]["unrelated"], "Pink");
        assert_eq!(d["calendarViewColors"].as_array().unwrap().len(), 1);
        let mut args = f.calendar_args();
        args["rule"] = json!({"mode":"none"});
        f.call("set_calendar_color_rule", args, "readWrite", &|| Ok(()))
            .unwrap();
        let mut args = f.calendar_args();
        args.as_object_mut().unwrap().remove("rule");
        f.call("remove_calendar_color_rule", args, "readWrite", &|| Ok(()))
            .unwrap();
        let d = crate::read_json_file(&f.root.join("blob/data.json")).unwrap();
        assert_eq!(d["calendarObjectColors"]["card"], "Blue");
    }
    #[test]
    fn read_only_revocation_errors_and_dry_runs_never_write() {
        let f = Fixture::new();
        let before = f.files();
        assert!(f
            .call(
                "set_kanban_calculation",
                f.kanban_args(),
                "read",
                &|| Ok(())
            )
            .is_err());
        assert_eq!(f.requests.load(Ordering::SeqCst), 0);
        let mut args = f.kanban_args();
        args.as_object_mut().unwrap().remove("revision");
        args["tokens"] = json!([{"kind":"operator","value":"+"}]);
        assert!(f
            .call("preview_kanban_calculation", args, "read", &|| Ok(()))
            .is_err());
        let mut args = f.calendar_args();
        args["rule"] = json!({"mode":"property","key":"estimate"});
        assert!(f
            .call("set_calendar_color_rule", args, "readWrite", &|| Ok(()))
            .is_err());
        assert!(f
            .call(
                "set_kanban_calculation",
                f.kanban_args(),
                "readWrite",
                &|| Err("permissions revoked".into())
            )
            .is_err());
        assert_eq!(f.files(), before);
    }
    #[test]
    fn project_sources_cannot_publish_defaults_or_partial_aggregates() {
        let f = Fixture::new();
        let project:ProjectConfiguration=serde_json::from_value(json!({"name":"Test","rootPaths":[],"sources":[{"spaceId":"space","kind":"query","id":"query","name":"Tasks","space":"Space","permission":{"access":"readWrite","filesEnabled":false,"approval":"fullAccess"}}]})).unwrap();
        let scope = Scope::load(&f.client, &project, &|| Ok(())).unwrap();
        let config = scope.configuration();
        let before = f.files();
        assert!(execute_at(
            &f.root,
            &f.client,
            &config,
            Some(&scope),
            Some(&project),
            &BlobuiRequest {
                tool: "set_calendar_color_rule".into(),
                arguments: f.calendar_args()
            },
            &|| Ok(()),
            &|_, _| Ok(())
        )
        .is_err());
        assert_eq!(f.files(), before);
        let result = execute_at(
            &f.root,
            &f.client,
            &config,
            Some(&scope),
            Some(&project),
            &BlobuiRequest {
                tool: "set_kanban_calculation".into(),
                arguments: f.kanban_args(),
            },
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(result["outcome"], "confirmed");
        let before = f.files();
        // A grant containing only the board host must not yield aggregates from a subset.
        let project:ProjectConfiguration=serde_json::from_value(json!({"name":"Test","rootPaths":[],"sources":[{"spaceId":"space","kind":"collection","id":"query","name":"Only host","space":"Space","permission":{"access":"readWrite","filesEnabled":false,"approval":"fullAccess"}}]})).unwrap();
        let empty_scope = Scope::default();
        assert!(execute_at(
            &f.root,
            &f.client,
            &config,
            Some(&empty_scope),
            Some(&project),
            &BlobuiRequest {
                tool: "set_kanban_calculation".into(),
                arguments: f.kanban_args()
            },
            &|| Ok(()),
            &|_, _| Ok(())
        )
        .unwrap_err()
        .contains("Access denied"));
        assert_eq!(f.files(), before);
    }
    #[test]
    fn legacy_reads_do_not_persist_migrations() {
        let f = Fixture::new();
        let path = f.root.join("blob/.blobui-kanban-formulas.json");
        let bytes = br#"{"version":1,"display":"compact","formulas":{}}"#;
        fs::write(&path, bytes).unwrap();
        let _ = kanban_configuration(&f.root);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn targeted_kanban_refresh_and_remove_preserve_unrelated_formulas() {
        let f = Fixture::new();
        let unrelated = json!({"target":{"spaceId":"other","objectId":"other","blockId":"other","viewId":"other"},"tokens":[{"kind":"aggregate","property":"missing","operation":"total"}]});
        fs::write(
            f.root.join("blob/.blobui-kanban-formulas.json"),
            serde_json::to_vec(
                &json!({"version":2,"display":"spacious","definitions":[unrelated]}),
            )
            .unwrap(),
        )
        .unwrap();
        let path = f.root.join("blob/data.json");
        let mut data = crate::read_json_file(&path).unwrap();
        data["kanbanFormulaResults"] = json!({"version":2,"display":"spacious","calculatedAt":"old","results":[{"target":unrelated["target"],"categoryId":"empty","value":17}]});
        fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
        f.call(
            "set_kanban_calculation",
            f.kanban_args(),
            "readWrite",
            &|| Ok(()),
        )
        .unwrap();
        let before = kanban_configuration(&f.root).unwrap();
        let mut args = f.kanban_args();
        args.as_object_mut().unwrap().remove("tokens");
        f.call(
            "refresh_kanban_calculation",
            args.clone(),
            "readWrite",
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(json!(before), json!(kanban_configuration(&f.root).unwrap()));
        f.call("remove_kanban_calculation", args, "readWrite", &|| Ok(()))
            .unwrap();
        let config = kanban_configuration(&f.root).unwrap();
        assert_eq!(config.display, "spacious");
        assert_eq!(config.definitions.len(), 1);
        assert_eq!(json!(config.definitions[0]), unrelated);
        let data = crate::read_json_file(&path).unwrap();
        assert_eq!(
            data["kanbanFormulaResults"]["results"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            data["kanbanFormulaResults"]["results"][0]["value"].as_f64(),
            Some(17.0)
        );
    }
    #[test]
    fn selected_calendar_rules_keep_other_types_and_publish_inheritance() {
        let f = Fixture::new();
        f.call(
            "set_calendar_color_rule",
            f.calendar_args(),
            "readWrite",
            &|| Ok(()),
        )
        .unwrap();
        let target =
            json!({"spaceId":"space","objectId":"query","blockId":"board","viewId":"calendar"});
        let path = f.root.join("blob/data.json");
        let mut data = crate::read_json_file(&path).unwrap();
        data["calendarViewColors"]
            .as_array_mut()
            .unwrap()
            .push(json!({"target":target,"objectId":"different-type","color":"Yellow"}));
        fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
        let mut args = f.calendar_args();
        args["target"] = target.clone();
        args["rule"] = json!({"mode":"property","key":"status"});
        f.call("set_calendar_color_rule", args, "readWrite", &|| Ok(()))
            .unwrap();
        let data = crate::read_json_file(&path).unwrap();
        assert_eq!(data["calendarObjectColors"]["card"], "Green");
        assert!(data["calendarViewColors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["objectId"] == "card" && v["color"] == "Red"));
        let before = f.files();
        let mut args = f.calendar_args();
        args["target"] = target.clone();
        args["rule"] = json!({"mode":"none"});
        args["dry_run"] = json!(true);
        f.call(
            "set_calendar_color_rule",
            args.clone(),
            "readWrite",
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(before, f.files());
        args["dry_run"] = json!(false);
        f.call("set_calendar_color_rule", args, "readWrite", &|| Ok(()))
            .unwrap();
        let mut args = f.calendar_args();
        args["target"] = target.clone();
        args.as_object_mut().unwrap().remove("rule");
        f.call("remove_calendar_color_rule", args, "readWrite", &|| Ok(()))
            .unwrap();
        let data = crate::read_json_file(&path).unwrap();
        let values = data["calendarViewColors"].as_array().unwrap();
        assert!(values
            .iter()
            .any(|v| v["objectId"] == "card" && v["color"] == "Green"));
        assert!(values
            .iter()
            .any(|v| v["objectId"] == "different-type" && v["color"] == "Yellow"));
        let read = f
            .call(
                "get_calendar_color_rule",
                json!({"space_id":"space","type_key":"task","target":target}),
                "read",
                &|| Ok(()),
            )
            .unwrap();
        assert!(!read["published_at"].is_null());
        assert_eq!(read["cached"], true);
        assert_eq!(read["configuration"], Value::Null);
    }
    #[test]
    fn approvals_and_mid_request_cancellation_keep_files_unchanged() {
        let f = Fixture::new();
        let config = crate::codex::normalize(
            &json!({"spaces":{"space":{"access":"readWrite","approval":"alwaysAsk"}}}),
        );
        let before = f.files();
        let asked = std::cell::Cell::new(false);
        assert!(execute_at(
            &f.root,
            &f.client,
            &config,
            None,
            None,
            &BlobuiRequest {
                tool: "set_kanban_calculation".into(),
                arguments: f.kanban_args()
            },
            &|| Ok(()),
            &|_, _| {
                asked.set(true);
                Err("denied".into())
            }
        )
        .is_err());
        assert!(asked.get());
        assert_eq!(f.requests.load(Ordering::SeqCst), 0);
        assert!(f
            .call(
                "set_kanban_calculation",
                f.kanban_args(),
                "readWrite",
                &|| if f.requests.load(Ordering::SeqCst) > 2 {
                    Err("cancelled".into())
                } else {
                    Ok(())
                }
            )
            .is_err());
        assert_eq!(f.files(), before);
    }
    #[test]
    fn targeted_discovery_includes_type_views_and_never_lists_other_spaces() {
        let f = Fixture::new();
        let before = f.files();
        let types = f
            .client
            .discover_type_views_in_space("space", &|id| {
                if id == "type-task" {
                    Ok(())
                } else {
                    Err("unexpected target".into())
                }
            })
            .unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].target.object_id, "type-task");
        assert!(types[0].properties.iter().any(|p| p.key == "estimate"));
        let calendar = f
            .call(
                "list_calendar_color_targets",
                json!({"space_id":"space","object_id":"query"}),
                "read",
                &|| Ok(()),
            )
            .unwrap();
        assert_eq!(calendar["types"][0]["type_key"], "task");
        assert_eq!(calendar["views"].as_array().unwrap().len(), 1);
        assert_eq!(before, f.files());
    }
}
