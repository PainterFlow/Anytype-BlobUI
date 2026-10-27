//! Read-only Anytype adapter. Full IDs are retained; incomplete pages fail closed.
use crate::{
    api,
    calculator::Calculator,
    kanban::{CategoryResult, Property, Target, View},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

const DISCOVERY_CONCURRENCY: usize = 4;

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
    fn document_blocks(&self, space: &str, object: &str) -> Result<Value, String> {
        self.request(
            "GET",
            &format!(
                "/v2/spaces/{}/objects/{}?ids=full&include=blocks",
                enc(space),
                enc(object)
            ),
            None,
        )
    }
    fn type_document(&self, space: &str, key: &str) -> Result<Value, String> {
        self.request(
            "GET",
            &format!(
                "/v2/spaces/{}/types/{}?ids=full",
                enc(space),
                enc(key)
            ),
            None,
        )
    }
    fn type_document_by_id(&self, space: &str, object: &str) -> Result<Value, String> {
        let rows = self.pages(
            "GET",
            &format!("/v2/spaces/{}/types", enc(space)),
            None,
        )?;
        for row in rows {
            let Ok(key) = required(&row, "key") else {
                continue;
            };
            let Ok(document) = self.type_document(space, &key) else {
                continue;
            };
            if document.get("id").and_then(Value::as_str) == Some(object)
                && document.get("kind").and_then(Value::as_str) == Some("object_type")
            {
                return Ok(document);
            }
        }
        Err("This Object Type is unavailable. Reload the view list.".into())
    }
    fn exposed_views(
        &self,
        space: &str,
        object: &str,
        kind: &str,
    ) -> Result<Vec<Value>, String> {
        let path = if kind == "objectTypes" {
            // Anytype's v1 lists route explicitly supports objectType layouts.
            format!(
                "/v1/spaces/{}/lists/{}/views",
                enc(space),
                enc(object)
            )
        } else {
            format!(
                "/v2/spaces/{}/{}/{}/views?ids=full",
                enc(space),
                kind,
                enc(object)
            )
        };
        self.pages("GET", &path, None)
    }
    fn document_for_target(&self, target: &Target) -> Result<(Value, Option<&'static str>), String> {
        let object_document = self.document_blocks(&target.space_id, &target.object_id);
        if let Ok(document) = &object_document {
            if let Some(kind) = list_kind(document) {
                return Ok((document.clone(), Some(kind)));
            }
        }
        if let Ok(document) = self.type_document_by_id(&target.space_id, &target.object_id) {
            return Ok((document, Some("objectTypes")));
        }
        object_document.map(|document| (document, None))
    }
    fn document_properties(&self, space: &str, object: &str) -> Result<Value, String> {
        self.request(
            "GET",
            &format!(
                "/v2/spaces/{}/objects/{}?ids=full&include=properties",
                enc(space),
                enc(object)
            ),
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

    fn discover_rows(
        &self,
        space_id: &str,
        space_name: &str,
        rows: &[Value],
    ) -> Result<SpaceDiscovery, String> {
        if rows.is_empty() {
            return Ok(SpaceDiscovery::default());
        }
        if rows.len() == 1 {
            return self.discover_row(space_id, space_name, &rows[0]).map(|object| {
                SpaceDiscovery {
                    views: object.views,
                    unreadable: object.unreadable,
                }
            });
        }

        let next = AtomicUsize::new(0);
        let discovery = Mutex::new(SpaceDiscovery::default());
        let errors = Mutex::new(Vec::new());
        let worker_count = rows.len().min(DISCOVERY_CONCURRENCY);
        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let discovery = &discovery;
                let errors = &errors;
                let next = &next;
                scope.spawn(move || loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= rows.len() {
                        break;
                    }
                    match self.discover_row(space_id, space_name, &rows[index]) {
                        Ok(object) => {
                            if let Ok(mut aggregate) = discovery.lock() {
                                aggregate.unreadable += object.unreadable;
                                aggregate.views.extend(object.views);
                            } else {
                                break;
                            }
                        }
                        Err(error) => {
                            if let Ok(mut failures) = errors.lock() {
                                failures.push((index, error));
                            } else {
                                break;
                            }
                        }
                    }
                });
            }
        });

        let mut failures = errors
            .into_inner()
            .map_err(|_| "The Kanban discovery task stopped. Try loading views again.".to_string())?;
        if !failures.is_empty() {
            failures.sort_by_key(|(index, _)| *index);
            return Err(failures.remove(0).1);
        }
        discovery
            .into_inner()
            .map_err(|_| "The Kanban discovery task stopped. Try loading views again.".to_string())
    }

    fn discover_row(
        &self,
        space_id: &str,
        space_name: &str,
        row: &Value,
    ) -> Result<ObjectDiscovery, String> {
        let object_id = required(row, "id")?;
        let document = match self.document_blocks(space_id, &object_id) {
            Ok(document) => document,
            Err(_) => {
                return Ok(ObjectDiscovery {
                    unreadable: 1,
                    ..ObjectDiscovery::default()
                });
            }
        };
        let kind = list_kind(&document).or_else(|| list_kind(row));
        // Object Type documents are enumerated through the dedicated types
        // route below, where their canonical IDs and keys are both available.
        if kind == Some("objectTypes") {
            return Ok(ObjectDiscovery::default());
        }
        let source = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Untitled")
            .to_string();
        self.discover_document(space_id, space_name, object_id, source, &document, kind)
    }

    fn discover_type_row(
        &self,
        space_id: &str,
        space_name: &str,
        row: &Value,
    ) -> Result<ObjectDiscovery, String> {
        let key = required(row, "key")?;
        let document = self.type_document(space_id, &key)?;
        if document.get("kind").and_then(Value::as_str) != Some("object_type") {
            return Err("Anytype returned an invalid Object Type document.".into());
        }
        let object_id = required(&document, "id")?;
        let source = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Untitled")
            .to_string();
        self.discover_document(
            space_id,
            space_name,
            object_id,
            source,
            &document,
            Some("objectTypes"),
        )
    }

    fn discover_document(
        &self,
        space_id: &str,
        space_name: &str,
        object_id: String,
        source: String,
        document: &Value,
        kind: Option<&'static str>,
    ) -> Result<ObjectDiscovery, String> {
        let mut candidates = Vec::new();
        let mut unreadable = 0;

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
                    space_id: space_id.into(),
                    object_id: object_id.clone(),
                    block_id,
                    view_id,
                };
                let view_source_signature = source_signature(&target, kind, block, raw);
                let (available_keys, metadata_error) =
                    match configured_view_keys(raw, Some(block)) {
                    Ok(keys) => (keys, None),
                    Err(error) => (None, Some(error)),
                };
                let mut candidate = DiscoveredView {
                    target,
                    name: raw
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("Kanban")
                        .into(),
                    space: space_name.into(),
                    source: source.clone(),
                    kind: if kind == Some("collections") {
                        "Collection"
                    } else if kind == Some("objectTypes") {
                        "Object Type"
                    } else if kind.is_some() {
                        "Query"
                    } else {
                        "Inline / type view"
                    }
                    .into(),
                    raw: raw.clone(),
                    source_signature: view_source_signature,
                    available_keys,
                    unsupported: metadata_error,
                };
                if kind.is_none() && candidate.unsupported.is_none() {
                    candidate.unsupported = Some(
                        "The API does not expose a filtered membership endpoint for this view."
                            .into(),
                    );
                }
                candidates.push(candidate);
            }
        }

        if candidates.is_empty() {
            return Ok(ObjectDiscovery {
                views: candidates,
                unreadable,
            });
        }

        let exposed = if let Some(kind) = kind.filter(|_| {
            candidates.iter().any(|candidate| candidate.unsupported.is_none())
        }) {
            match self.exposed_views(space_id, &object_id, kind) {
                Ok(value) => Some(value),
                Err(_) => {
                    unreadable += 1;
                    None
                }
            }
        } else {
            None
        };

        for candidate in &mut candidates {
            if candidate.unsupported.is_some() {
                continue;
            }
            let Some(exposed) = exposed.as_deref() else {
                candidate.unsupported = Some(
                    "The API does not expose a filtered membership endpoint for this view.".into(),
                );
                continue;
            };
            match exposed_view(&candidate.target, exposed) {
                // The v1 Object Type list route exposes view IDs and layout,
                // while the full type document supplies its property schema.
                Ok(api_view) if kind == Some("objectTypes") => {
                    if api_view
                        .get("layout")
                        .or_else(|| api_view.get("type"))
                        .and_then(Value::as_str)
                        != Some("kanban")
                    {
                        candidate.unsupported =
                            Some("This view is no longer a Kanban view.".into());
                    }
                }
                Ok(api_view) => match configured_view_keys(api_view, None) {
                    Ok(Some(api_keys)) => match &candidate.available_keys {
                        Some(document_keys)
                            if api_keys.iter().any(|key| !document_keys.contains(key)) =>
                        {
                            candidate.unsupported = Some(
                                "The API returned inconsistent properties for this view.".into(),
                            );
                        }
                        Some(_) => {}
                        None => candidate.available_keys = Some(api_keys),
                    },
                    Ok(None) => {}
                    Err(error) => candidate.unsupported = Some(error),
                },
                Err(error) => candidate.unsupported = Some(error),
            }
        }

        Ok(ObjectDiscovery {
            views: candidates,
            unreadable,
        })
    }

    pub fn discover(&self) -> Result<crate::kanban::Catalog, String> {
        let mut views = Vec::new();
        let mut unreadable = 0usize;
        for space in self.pages("GET", "/v2/spaces?ids=full", None)? {
            let sid = required(&space, "id")?;
            let name = space.get("name").and_then(Value::as_str).unwrap_or("Space");
            let rows = self.pages(
                "POST",
                &format!("/v2/spaces/{}/search", enc(&sid)),
                Some(&json!({})),
            )?;
            let mut discovered = self.discover_rows(&sid, name, &rows)?;
            unreadable += discovered.unreadable;
            // Type documents do not appear in the space search results. The
            // v2 types list supplies their keys; full documents carry board
            // views and the IDs used by Anytype's v1 filtered list route.
            match self.pages(
                "GET",
                &format!("/v2/spaces/{}/types", enc(&sid)),
                None,
            ) {
                Ok(type_rows) => {
                    for row in type_rows {
                        match self.discover_type_row(&sid, name, &row) {
                            Ok(object) => {
                                unreadable += object.unreadable;
                                discovered.views.extend(object.views);
                            }
                            Err(_) => unreadable += 1,
                        }
                    }
                }
                Err(_) => unreadable += 1,
            }
            if discovered.views.is_empty() {
                continue;
            }

            // Space property definitions are needed for names and formats, but
            // only keys explicitly configured on each view are exposed below.
            let properties = self.properties(&sid)?;
            for candidate in discovered.views.drain(..) {
                views.push(candidate.into_view(&properties));
            }
        }
        views.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.space.cmp(&b.space))
                .then(a.source.cmp(&b.source))
                .then(a.target.cmp(&b.target))
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
        let (document, kind) = self.document_for_target(target)?;
        let kind = kind
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
        let exposed = self.exposed_views(&target.space_id, &target.object_id, kind)?;
        let api_view = exposed_view(target, &exposed)?;
        if kind == "objectTypes"
            && api_view
                .get("layout")
                .or_else(|| api_view.get("type"))
                .and_then(Value::as_str)
                != Some("kanban")
        {
            return Err("This view is no longer a Kanban view.".into());
        }
        let document_keys = configured_view_keys(raw, Some(block))?;
        let endpoint_keys = if kind == "objectTypes" {
            None
        } else {
            configured_view_keys(api_view, None)?
        };
        let available_keys = match (document_keys, endpoint_keys) {
            (Some(document), Some(endpoint))
                if endpoint.iter().any(|key| !document.contains(key)) =>
            {
                return Err("The API returned inconsistent properties for this view.".into());
            }
            (Some(document), _) => document,
            (_, Some(endpoint)) => endpoint,
            (None, None) => {
                return Err("The API does not expose which properties are available to this view.".into());
            }
        };
        let properties = self.properties(&target.space_id)?;
        let keys = calculator.properties();
        let mut boolean_keys = BTreeSet::new();
        if keys.iter().any(|key| {
            let format = properties
                .iter()
                .find(|property| property.key == *key)
                .map(|property| property.format.as_str());
            match format {
                Some("number") => !available_keys.contains(key),
                Some(format) if is_boolean_format(format) => {
                    boolean_keys.insert(key.clone());
                    !available_keys.contains(key)
                }
                _ => true,
            }
        }) {
            return Err("A calculation property is no longer numeric or boolean, or is unavailable to this view. Choose a replacement property.".into());
        }
        let mut grouping = self.grouping(target, raw, &properties)?;
        let mut fields = keys;
        fields.push(grouping.property.clone());
        fields.sort();
        fields.dedup();
        let rows = if kind == "objectTypes" {
            let rows = self.pages(
                "GET",
                &format!(
                    "/v1/spaces/{}/lists/{}/views/{}/objects",
                    enc(&target.space_id),
                    enc(&target.object_id),
                    enc(&target.view_id)
                ),
                None,
            )?;
            rows.iter()
                .map(|row| normalize_v1_list_object(row, &fields, &properties))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            self.pages(
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
            )?
        };
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
                let value = calculator.evaluate_with_boolean_properties(&cards, &boolean_keys)?;
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
        let (property, format) = grouping_property(view, properties)?;
        let mut grouping = Grouping {
            property,
            format: format.clone(),
            options: BTreeMap::new(),
            categories: BTreeMap::new(),
        };
        if is_boolean_format(&format) {
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
                insert_option_id(&mut resolved, name, id)?;
            }
        }

        let options_need_ids = options.iter().any(|option| {
            option["name"]
                .as_str()
                .is_none_or(|name| !resolved.contains_key(name))
        });
        let select_group_ids = if format == "select" {
            view.get("groups")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|group| required(group, "id"))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let group_needs_ids = select_group_ids.iter().any(|id| {
            id.as_str() != "empty" && !resolved.values().any(|known| known == id)
        });

        // A single legacy tags read can resolve the whole option set. This
        // avoids one document request per select group on API versions that
        // omit IDs from v2 option rows.
        if options_need_ids || group_needs_ids {
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
                        insert_option_id(&mut resolved, name, id)?;
                    }
                }
            }
        }

        // When neither v2 nor the legacy endpoint exposes stable IDs, resolve
        // only groups whose exact IDs are absent from the option map.
        let unresolved_group_ids = select_group_ids
            .iter()
            .filter(|id| {
                id.as_str() != "empty" && !resolved.values().any(|known| known == *id)
            })
            .cloned()
            .collect::<Vec<_>>();
        for id in &unresolved_group_ids {
            let object = self.document_properties(&target.space_id, id).map_err(|_| {
                "The API could not resolve a category's exact identity.".to_string()
            })?;
            let name = object
                .pointer("/properties/name")
                .and_then(Value::as_str)
                .ok_or("The API did not return a category option name.")?;
            insert_option_id(&mut resolved, name, id)?;
        }

        if options.iter().any(|option| {
            option["name"]
                .as_str()
                .is_none_or(|name| !resolved.contains_key(name))
        }) {
            return Err("The API does not expose every option's stable ID. This view cannot publish results safely.".into());
        }
        for option in options {
            let name = required(&option, "name")?;
            let id = resolved
                .get(&name)
                .ok_or("The API does not expose every option's stable ID. This view cannot publish results safely.")?;
            crate::kanban::validate_id(id)?;
            if grouping.options.insert(name.clone(), id.clone()).is_some() {
                return Err("Duplicate option names prevent an exact category match.".into());
            }
            let category_id = if format == "multi_select" {
                md5(id)
            } else {
                id.clone()
            };
            if category_id == "empty"
                || grouping
                    .categories
                    .insert(category_id, name)
                    .is_some()
            {
                return Err("The API returned duplicate category identities.".into());
            }
        }
        grouping.categories.insert("empty".into(), String::new());
        Ok(grouping)
    }
}

#[derive(Default)]
struct ObjectDiscovery {
    views: Vec<DiscoveredView>,
    unreadable: usize,
}

#[derive(Default)]
struct SpaceDiscovery {
    views: Vec<DiscoveredView>,
    unreadable: usize,
}

struct DiscoveredView {
    target: Target,
    name: String,
    space: String,
    source: String,
    kind: String,
    raw: Value,
    source_signature: Option<String>,
    available_keys: Option<BTreeSet<String>>,
    unsupported: Option<String>,
}

impl DiscoveredView {
    fn into_view(mut self, space_properties: &[Property]) -> View {
        let properties = self.available_keys.as_ref().map(|keys| {
            space_properties
                .iter()
                .filter(|property| {
                    keys.contains(&property.key)
                        && (property.format == "number" || is_boolean_format(&property.format))
                })
                .cloned()
                .collect()
        }).unwrap_or_else(|| {
            if self.unsupported.is_none() {
                self.unsupported = Some(
                    "The API does not expose which properties are available to this view.".into(),
                );
            }
            Vec::new()
        });
        if self.unsupported.is_none() {
            if let Err(error) = grouping_property(&self.raw, space_properties) {
                self.unsupported = Some(error);
            }
        }
        View {
            target: self.target,
            name: self.name,
            space: self.space,
            source: self.source,
            kind: self.kind,
            source_signature: self.source_signature,
            properties,
            unsupported: self.unsupported,
        }
    }
}

fn exposed_view<'a>(target: &Target, exposed: &'a [Value]) -> Result<&'a Value, String> {
    // API routes accept a unique suffix, but storage/CSS always use the full ID
    // from the document. Ambiguous suffixes are refused, not guessed.
    let matches = exposed
        .iter()
        .filter(|view| {
            view["id"].as_str().is_some_and(|id| {
                id == target.view_id || (id.len() >= 6 && target.view_id.ends_with(id))
            })
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("The API does not expose an unambiguous filtered endpoint for this view.".into());
    }
    Ok(matches[0])
}

fn configured_view_keys(
    view: &Value,
    dataview_block: Option<&Value>,
) -> Result<Option<BTreeSet<String>>, String> {
    let mut configured = BTreeSet::new();
    let mut found = false;
    for field in ["relations", "columns"] {
        let Some(raw) = view.get(field).filter(|value| !value.is_null()) else {
            continue;
        };
        let items = raw
            .as_array()
            .ok_or("The API returned malformed properties for this view.")?;
        for item in items {
            let key = item
                .as_str()
                .or_else(|| item.get("key").and_then(Value::as_str))
                .or_else(|| item.get("property_key").and_then(Value::as_str))
                .or_else(|| item.get("propertyKey").and_then(Value::as_str))
                .or_else(|| item.get("relation_key").and_then(Value::as_str))
                .or_else(|| item.get("relationKey").and_then(Value::as_str))
                .or_else(|| {
                    item.get("property").and_then(|property| {
                        property.as_str().or_else(|| {
                            property
                                .get("key")
                                .or_else(|| property.get("property_key"))
                                .or_else(|| property.get("propertyKey"))
                                .or_else(|| property.get("relation_key"))
                                .and_then(Value::as_str)
                        })
                    })
                })
                .filter(|key| !key.is_empty())
                .ok_or("The API returned a view property without a key.")?;
            // Anytype keeps configured but hidden columns in the dataview.
            // They remain properties of the view and can be used in formulas.
            configured.insert(key.to_string());
        }
        found = true;
    }

    // The dataview block's `properties` list is the source type's relation
    // schema: Anytype serializes entries as { property, format }. It includes
    // properties the board does not currently display as columns.
    if let Some(raw) = dataview_block
        .and_then(|block| block.get("properties"))
        .filter(|value| !value.is_null())
    {
        let items = raw
            .as_array()
            .ok_or("The API returned malformed source properties for this view.")?;
        for item in items {
            if item
                .get("format")
                .and_then(Value::as_str)
                .filter(|format| !format.is_empty())
                .is_none()
            {
                return Err("The API returned a source property without a format.".into());
            }
            let format = item.get("format").and_then(Value::as_str).unwrap_or_default();
            if format != "number" && !is_boolean_format(format) {
                continue;
            }
            let property = item
                .get("property")
                .and_then(Value::as_str)
                .or_else(|| {
                    item.get("property").and_then(|property| {
                        property
                            .get("key")
                            .or_else(|| property.get("property_key"))
                            .or_else(|| property.get("propertyKey"))
                            .and_then(Value::as_str)
                    })
                })
                .filter(|property| !property.is_empty())
                .ok_or("The API returned a source property without a key.")?;
            configured.insert(property.to_string());
        }
        found = true;
    }

    Ok(found.then_some(configured))
}

fn source_signature(
    target: &Target,
    kind: Option<&str>,
    block: &Value,
    view: &Value,
) -> Option<String> {
    let source_id = match kind {
        Some("queries" | "collections" | "objectTypes") => target.object_id.as_str(),
        _ => block.get("object_id").and_then(Value::as_str)?,
    };
    let view_id = view.get("id").and_then(Value::as_str)?;
    let layout = view
        .get("type")
        .or_else(|| view.get("layout"))
        .and_then(Value::as_str)?;
    let group_by = view.get("group_by").and_then(Value::as_str)?;
    // Empty filter/sort lists are omitted by some serialized document blocks,
    // while the canonical views endpoint emits them as arrays. Missing and
    // null therefore mean the same as no configured filters or sorts.
    let empty_list = Value::Array(Vec::new());
    let filters = view
        .get("filters")
        .filter(|value| !value.is_null())
        .unwrap_or(&empty_list);
    let sorts = view
        .get("sorts")
        .filter(|value| !value.is_null())
        .unwrap_or(&empty_list);
    if !filters.is_array() || !sorts.is_array() {
        return None;
    }
    let filters = strip_view_editor_ids(filters);
    let sorts = strip_view_editor_ids(sorts);
    let signature = serde_json::to_vec(&json!([
        target.space_id.as_str(),
        source_id,
        view_id,
        layout,
        group_by,
        filters,
        sorts
    ]))
    .ok()?;
    let digest = Sha256::digest(signature);
    Some(
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

fn strip_view_editor_ids(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(strip_view_editor_ids).collect()),
        Value::Object(fields) => {
            let is_filter_or_sort = [
                "property",
                "property_key",
                "propertyKey",
                "relation_key",
                "relationKey",
            ]
            .iter()
            .any(|key| fields.contains_key(*key))
                && (fields.contains_key("condition")
                    || fields.contains_key("direction")
                    || fields.contains_key("sort_type")
                    || fields.contains_key("sortType"));
            let is_filter_group = fields.contains_key("operator") && fields.contains_key("filters");
            Value::Object(
                fields
                    .iter()
                    .filter(|(key, _)| {
                        !(key.as_str() == "id" && (is_filter_or_sort || is_filter_group))
                    })
                    .map(|(key, value)| {
                        let value = if key == "filters" && (is_filter_group || is_filter_or_sort) {
                            strip_view_editor_ids(value)
                        } else {
                            value.clone()
                        };
                        (key.clone(), value)
                    })
                    .collect(),
            )
        }
        value => value.clone(),
    }
}

fn grouping_property(view: &Value, properties: &[Property]) -> Result<(String, String), String> {
    let property = required(view, "group_by")
        .map_err(|_| "The API does not expose this view's grouping property.".to_string())?;
    let format = properties
        .iter()
        .find(|candidate| candidate.key == property)
        .ok_or("The grouping property is unavailable.")?
        .format
        .clone();
    if !(is_boolean_format(&format) || matches!(format.as_str(), "select" | "multi_select")) {
        return Err("This grouping format cannot be matched to Anytype headers safely.".into());
    }
    Ok((property, format))
}

fn is_boolean_format(format: &str) -> bool {
    matches!(format, "checkbox" | "boolean")
}

fn insert_option_id(
    resolved: &mut BTreeMap<String, String>,
    name: &str,
    id: &str,
) -> Result<(), String> {
    if resolved.get(name).is_some_and(|old| old.as_str() != id) {
        return Err("Duplicate option names prevent an exact category match.".into());
    }
    if resolved
        .iter()
        .any(|(previous_name, previous_id)| {
            previous_name.as_str() != name && previous_id.as_str() == id
        })
    {
        return Err("The API returned duplicate category identities.".into());
    }
    resolved.insert(name.to_string(), id.to_string());
    Ok(())
}

fn list_kind(document: &Value) -> Option<&'static str> {
    if document.get("kind").and_then(Value::as_str) == Some("object_type") {
        return Some("objectTypes");
    }
    match document["type"].as_str() {
        Some("collection") => Some("collections"),
        Some("query" | "set") => Some("queries"),
        _ => None,
    }
}

fn normalize_v1_list_object(
    row: &Value,
    fields: &[String],
    definitions: &[Property],
) -> Result<Value, String> {
    let id = required(row, "id")?;
    let raw_properties = row
        .get("properties")
        .and_then(Value::as_array)
        .ok_or("Anytype returned malformed Object Type membership properties.")?;
    let requested = fields.iter().cloned().collect::<BTreeSet<_>>();
    let mut normalized = serde_json::Map::new();
    for raw in raw_properties {
        let key = required(raw, "key")?;
        if !requested.contains(&key) {
            continue;
        }
        if normalized.contains_key(&key) {
            return Err("Anytype returned duplicate Object Type membership properties.".into());
        }
        let format = required(raw, "format")?;
        let expected = definitions
            .iter()
            .find(|property| property.key == key)
            .map(|property| property.format.as_str())
            .ok_or("An Object Type membership property is unavailable.")?;
        if expected != format && !(is_boolean_format(expected) && is_boolean_format(&format)) {
            return Err("Anytype returned an incompatible Object Type membership property.".into());
        }
        let value = match format.as_str() {
            "number" => raw
                .get("number")
                .filter(|value| value.as_f64().is_some())
                .cloned()
                .ok_or("Anytype returned an invalid numeric Object Type property.")?,
            "checkbox" | "boolean" => raw
                .get("checkbox")
                .filter(|value| value.is_boolean())
                .cloned()
                .ok_or("Anytype returned an invalid checkbox Object Type property.")?,
            "select" => match raw.get("select") {
                Some(Value::Null) => Value::Null,
                Some(value) => Value::String(required(value, "name")?),
                None => return Err("Anytype omitted a select Object Type property value.".into()),
            },
            "multi_select" => match raw.get("multi_select") {
                Some(Value::Null) => Value::Array(Vec::new()),
                Some(Value::Array(values)) => Value::Array(
                    values
                        .iter()
                        .map(|value| required(value, "name").map(Value::String))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                _ => return Err("Anytype returned an invalid multi-select Object Type property.".into()),
            },
            _ => return Err("Anytype returned an unsupported Object Type property format.".into()),
        };
        normalized.insert(key, value);
    }
    for field in fields {
        normalized.entry(field.clone()).or_insert(Value::Null);
    }
    Ok(json!({"id": id, "properties": normalized}))
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
        if is_boolean_format(&self.format) {
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
                let view = json!({"id":"view-a","type":"kanban","name":"Board","group_by":"status","filters":[],"sorts":[],"groups":[{"id":"option-a"},{"id":"empty"}],"columns":[{"property":"title","width":180},{"property":"status","width":140}]});
                let missing_type_api = path.starts_with("/v2/spaces/space/types?");
                let response = if missing_type_api {
                    json!({"error":"type endpoint unavailable in this fixture"})
                } else if path.starts_with("/v2/spaces?") {
                    page(json!([{"id":"space","name":"Work"}]))
                } else if path.starts_with("/v2/spaces/space/properties/status/options") {
                    page(json!([{"name":"A"}]))
                } else if path.starts_with("/v1/spaces/space/properties/status/tags") {
                    page(json!([{"id":"option-a","name":"A"}]))
                } else if path.starts_with("/v2/spaces/space/properties") {
                    page(
                        json!([{"key":"estimate","name":"Estimate","format":"number"},{"key":"hidden-metric","name":"Hidden metric","format":"number"},{"key":"unused-metric","name":"Unused metric","format":"number"},{"key":"done","name":"Done","format":"checkbox"},{"key":"off-view-bool","name":"Off view boolean","format":"checkbox"},{"key":"status","name":"Status","format":"select"}]),
                    )
                } else if path.starts_with("/v2/spaces/space/search") {
                    page(json!([{"id":"object-a","name":"Tasks","type":"query"}]))
                } else if path.starts_with("/v2/spaces/space/objects/object-a") {
                    json!({"id":"object-a","type":"query","blocks":[{"id":"board","object_id":"object-a","properties":[{"property":"estimate","format":"number"},{"property":"hidden-metric","format":"number"},{"property":"done","format":"checkbox"},{"property":"status","format":"select"}],"views":[view]}]})
                } else if path.starts_with("/v2/spaces/space/objects/option-a") {
                    json!({"id":"option-a","kind":"relation_option","properties":{"name":"A"}})
                } else if path.starts_with("/v2/spaces/space/queries/object-a/views") {
                    page(json!([view]))
                } else if path.starts_with("/v2/spaces/space/queries/object-a/objects") {
                    assert!(path.contains("view=view-a"));
                    assert!(path.contains("fields=estimate%2Cstatus") || path.contains("fields=done%2Cstatus"));
                    if path.contains("offset=0") {
                        json!({"data":[{"id":"card-a","properties":{"estimate":3,"done":true,"status":"A"}}],"has_more":true,"total":2})
                    } else {
                        json!({"data":[{"id":"card-b","properties":{"estimate":7,"done":false,"status":null}}],"has_more":false,"total":2})
                    }
                } else {
                    panic!("Unexpected fixture route {path}")
                };
                let body = serde_json::to_vec(&response).unwrap();
                let status = if missing_type_api {
                    "404 Not Found"
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
        let client = Client {
            key: crate::SecretString::new("fixture-key".into()),
            ip: "127.0.0.1".into(),
            port,
        };
        let catalog = client.discover().unwrap();
        assert_eq!(catalog.views.len(), 1);
        assert!(catalog
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("view list may be incomplete")));
        assert!(catalog.views[0].unsupported.is_none());
        assert_eq!(
            catalog.views[0]
                .properties
                .iter()
                .map(|property| property.key.as_str())
                .collect::<Vec<_>>(),
            vec!["estimate", "hidden-metric", "done"]
        );
        assert!(catalog.views[0].source_signature.is_some());
        let calculator = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "estimate".into(),
            operation: "total".into(),
            calculation: None,
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
            Some((3.0).into())
        );
        assert_eq!(
            results
                .iter()
                .find(|r| r.category_id == "empty")
                .unwrap()
                .value,
            Some((7.0).into())
        );
        let boolean_calculator = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "done".into(),
            operation: "total".into(),
            calculation: None,
        }])
        .unwrap();
        let boolean_results = client
            .calculate(&catalog.views[0].target, &boolean_calculator)
            .unwrap();
        assert_eq!(
            boolean_results
                .iter()
                .find(|result| result.category_id == "option-a")
                .unwrap()
                .value,
            Some((1.0).into())
        );
        assert_eq!(
            boolean_results
                .iter()
                .find(|result| result.category_id == "empty")
                .unwrap()
                .value,
            Some((0.0).into())
        );
        let comparison = Calculator::parse(&[
            crate::calculator::Token::Aggregate { property: "done".into(), operation: "sum".into(), calculation: None },
            crate::calculator::Token::Operator { value: ">".into() },
            crate::calculator::Token::Number { value: "0".into() },
        ]).unwrap();
        assert!(client.calculate(&catalog.views[0].target, &comparison).unwrap().iter().all(|result| matches!(result.value, Some(crate::calculator::CalculationValue::Boolean(_)))));
        let skipped_deleted = Calculator::parse(&[
            crate::calculator::Token::Boolean { value: true },
            crate::calculator::Token::Operator { value: "?".into() },
            crate::calculator::Token::Number { value: "1".into() },
            crate::calculator::Token::Operator { value: ":".into() },
            crate::calculator::Token::Aggregate { property: "deleted".into(), operation: "sum".into(), calculation: None },
        ]).unwrap();
        assert!(client.calculate(&catalog.views[0].target, &skipped_deleted).is_err());
        let deleted = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "deleted".into(),
            operation: "total".into(),
            calculation: None,
        }])
        .unwrap();
        assert!(client
            .calculate(&catalog.views[0].target, &deleted)
            .is_err());
        let unavailable = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "unused-metric".into(),
            operation: "total".into(),
            calculation: None,
        }])
        .unwrap();
        assert!(client
            .calculate(&catalog.views[0].target, &unavailable)
            .is_err());
        let unavailable_boolean = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "off-view-bool".into(),
            operation: "total".into(),
            calculation: None,
        }])
        .unwrap();
        assert!(client
            .calculate(&catalog.views[0].target, &unavailable_boolean)
            .is_err());
        stop.store(true, Ordering::Relaxed);
        worker.join().unwrap();
    }

    #[test]
    fn object_type_views_use_filtered_v1_membership_and_typed_properties() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc, Mutex,
            },
            time::{Duration, Instant},
        };
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let paths = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_paths = paths.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
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
                seen_paths.lock().unwrap().push(path.to_string());
                let is_missing_object_route =
                    path.starts_with("/v2/spaces/space/objects/type-id?");
                let is_v1 = path.starts_with("/v1/");
                if is_v1 {
                    assert!(request.contains("Anytype-Version: 2025-11-08"));
                }
                let v2_page = |data: Value| {
                    let total = data.as_array().map_or(0, |rows| rows.len());
                    json!({"data":data,"has_more":false,"total":total})
                };
                let v1_page = |data: Value, has_more: bool, total: usize| {
                    json!({"data":data,"pagination":{"has_more":has_more,"total":total}})
                };
                let view = json!({
                    "id":"type-view",
                    "type":"kanban",
                    "name":"Tasks by status",
                    "group_by":"status",
                    "filters":[{"property_key":"estimate","condition":"gt","value":0}],
                    "sorts":[{"property_key":"estimate","sort_type":"desc"}],
                    "groups":[{"id":"option-a"},{"id":"empty"}],
                    "columns":[{"property":"estimate"},{"property":"done"},{"property":"status"}]
                });
                let rows_first = json!([
                    {"id":"card-a","properties":[
                        {"key":"estimate","format":"number","number":3},
                        {"key":"done","format":"checkbox","checkbox":true},
                        {"key":"status","format":"select","select":{"id":"option-a","name":"A"}}
                    ]},
                    {"id":"card-b","properties":[
                        {"key":"status","format":"select","select":null}
                    ]}
                ]);
                let rows_last = json!([
                    {"id":"card-c","properties":[
                        {"key":"estimate","format":"number","number":7},
                        {"key":"done","format":"checkbox","checkbox":false},
                        {"key":"status","format":"select","select":{"id":"option-a","name":"A"}}
                    ]}
                ]);
                let response = if is_missing_object_route {
                    json!({"error":"Object Type documents use the types route"})
                } else if path.starts_with("/v2/spaces?") {
                    v2_page(json!([{"id":"space","name":"Work"}]))
                } else if path.starts_with("/v2/spaces/space/search") {
                    v2_page(json!([]))
                } else if path.starts_with("/v2/spaces/space/types/task?") {
                    json!({
                        "kind":"object_type",
                        "id":"type-id",
                        "blocks":[{
                            "id":"dataview",
                            "properties":[
                                {"property":"estimate","format":"number"},
                                {"property":"done","format":"checkbox"},
                                {"property":"status","format":"select"}
                            ],
                            "views":[view]
                        }]
                    })
                } else if path.starts_with("/v2/spaces/space/types?") {
                    v2_page(json!([{"key":"task","name":"Task"}]))
                } else if path.starts_with("/v1/spaces/space/lists/type-id/views/type-view/objects") {
                    if path.contains("offset=0") {
                        v1_page(rows_first, true, 3)
                    } else {
                        assert!(path.contains("offset=2"));
                        v1_page(rows_last, false, 3)
                    }
                } else if path.starts_with("/v1/spaces/space/lists/type-id/views") {
                    v1_page(
                        json!([{"id":"type-view","name":"Tasks by status","layout":"kanban"}]),
                        false,
                        1,
                    )
                } else if path.starts_with("/v2/spaces/space/properties/status/options") {
                    v2_page(json!([{"id":"option-a","name":"A"}]))
                } else if path.starts_with("/v2/spaces/space/properties") {
                    v2_page(json!([
                        {"key":"estimate","name":"Estimate","format":"number"},
                        {"key":"done","name":"Done","format":"checkbox"},
                        {"key":"status","name":"Status","format":"select"}
                    ]))
                } else {
                    panic!("Unexpected Object Type fixture route {path}")
                };
                let body = serde_json::to_vec(&response).unwrap();
                let status = if is_missing_object_route {
                    "404 Not Found"
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
        let client = Client {
            key: crate::SecretString::new("fixture-key".into()),
            ip: "127.0.0.1".into(),
            port,
        };
        let catalog = client.discover().unwrap();
        assert_eq!(catalog.views.len(), 1);
        let view = &catalog.views[0];
        assert_eq!(view.kind, "Object Type");
        assert_eq!(view.source, "Task");
        assert!(view.unsupported.is_none());
        assert!(view.source_signature.is_some());

        let total = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "estimate".into(),
            operation: "total".into(),
            calculation: None,
        }])
        .unwrap();
        let results = client.calculate(&view.target, &total).unwrap();
        assert_eq!(
            results
                .iter()
                .find(|result| result.category_id == "option-a")
                .unwrap()
                .value,
            Some((10.0).into())
        );
        assert_eq!(
            results
                .iter()
                .find(|result| result.category_id == "empty")
                .unwrap()
                .value,
            Some((0.0).into()),
            "a missing numeric property stays null and is skipped like v2 nulls"
        );
        assert!(results.iter().all(|result| result.target == view.target));

        let done = Calculator::parse(&[crate::calculator::Token::Aggregate {
            property: "done".into(),
            operation: "total".into(),
            calculation: None,
        }])
        .unwrap();
        let boolean_results = client.calculate(&view.target, &done).unwrap();
        assert_eq!(
            boolean_results
                .iter()
                .find(|result| result.category_id == "option-a")
                .unwrap()
                .value,
            Some((1.0).into())
        );
        assert_eq!(
            boolean_results
                .iter()
                .find(|result| result.category_id == "empty")
                .unwrap()
                .value,
            Some((0.0).into())
        );
        let paths = paths.lock().unwrap();
        assert!(paths.iter().any(|path| path.contains("/types/task?ids=full")));
        assert!(paths.iter().any(|path| path.contains("/lists/type-id/views/type-view/objects?limit=100&offset=0")));
        assert!(paths.iter().any(|path| path.contains("/lists/type-id/views/type-view/objects?limit=100&offset=2")));
        stop.store(true, Ordering::Relaxed);
        drop(paths);
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
    fn view_properties_are_scoped_to_source_schema_and_configured_columns() {
        let relations = json!({
            "relations": [
                {"key":"estimate","isVisible":true},
                {"key":"hidden-metric","isVisible":false}
            ]
        });
        assert_eq!(
            configured_view_keys(&relations, None).unwrap().unwrap(),
            BTreeSet::from(["estimate".to_string(), "hidden-metric".to_string()])
        );

        let columns = json!({
            "columns": [
                {"property":"title"},
                {"property":"hidden-metric","hidden":true}
            ]
        });
        let source_schema = json!({
            "properties": [
                {"property":"estimate","format":"number"},
                {"property":"done","format":"checkbox"},
                {"property":"status","format":"select"}
            ]
        });
        assert_eq!(
            configured_view_keys(&columns, Some(&source_schema)).unwrap().unwrap(),
            BTreeSet::from([
                "estimate".to_string(),
                "done".to_string(),
                "hidden-metric".to_string(),
                "title".to_string()
            ])
        );
        assert!(configured_view_keys(&json!({"group_by":"estimate"}), None)
            .unwrap()
            .is_none());
    }

    #[test]
    fn v1_object_type_property_rows_normalize_null_and_typed_values() {
        let definitions = vec![
            Property {
                key: "estimate".into(),
                name: "Estimate".into(),
                format: "number".into(),
            },
            Property {
                key: "done".into(),
                name: "Done".into(),
                format: "checkbox".into(),
            },
            Property {
                key: "status".into(),
                name: "Status".into(),
                format: "select".into(),
            },
            Property {
                key: "tags".into(),
                name: "Tags".into(),
                format: "multi_select".into(),
            },
        ];
        let fields = vec![
            "estimate".into(),
            "done".into(),
            "status".into(),
            "tags".into(),
        ];
        let row = json!({
            "id":"card-a",
            "properties":[
                {"key":"status","format":"select","select":null},
                {"key":"tags","format":"multi_select","multi_select":[{"name":"A"},{"name":"B"}]}
            ]
        });
        let normalized = normalize_v1_list_object(&row, &fields, &definitions).unwrap();
        assert!(normalized.pointer("/properties/estimate").unwrap().is_null());
        assert!(normalized.pointer("/properties/done").unwrap().is_null());
        assert!(normalized.pointer("/properties/status").unwrap().is_null());
        assert_eq!(normalized["properties"]["tags"], json!(["A", "B"]));

        // v1's typed NumberPropertyValue always carries a number (unset values
        // are omitted before marshaling), so a present null number is malformed.
        let malformed = json!({
            "id":"card-b",
            "properties":[{"key":"estimate","format":"number","number":null}]
        });
        assert!(normalize_v1_list_object(&malformed, &fields, &definitions).is_err());
    }

    #[test]
    fn source_signature_matches_inline_and_canonical_query_definitions_safely() {
        let canonical_target = Target {
            space_id: "space-a".into(),
            object_id: "query-a".into(),
            block_id: "query-block".into(),
            view_id: "view-a".into(),
        };
        let inline_target = Target {
            object_id: "parent-document".into(),
            block_id: "embed-block".into(),
            ..canonical_target.clone()
        };
        let canonical_block = json!({"id":"query-block"});
        let inline_block = json!({"id":"embed-block","object_id":"query-a"});
        let mut definition = json!({
            "id":"view-a",
            "type":"kanban",
            "group_by":"status",
            "filters":[{"id":"editor-filter-a","property_key":"status","condition":"eq","value":{"id":"option-a"}}],
            "sorts":[{"id":"editor-sort-a","property_key":"created","sort_type":"desc"}]
        });

        let canonical_signature = source_signature(&canonical_target, Some("queries"), &canonical_block, &definition);
        let inline_signature = source_signature(&inline_target, None, &inline_block, &definition);
        assert_eq!(canonical_signature, inline_signature);

        let type_target = Target {
            object_id: "type-a".into(),
            block_id: "dataview".into(),
            ..canonical_target.clone()
        };
        let inline_type_target = Target {
            object_id: "parent-document".into(),
            block_id: "type-embed".into(),
            ..type_target.clone()
        };
        let type_block = json!({"id":"dataview"});
        let inline_type_block = json!({"id":"type-embed","object_id":"type-a"});
        assert_eq!(
            source_signature(&type_target, Some("objectTypes"), &type_block, &definition),
            source_signature(&inline_type_target, None, &inline_type_block, &definition),
            "Object Type boards and their embedded copies share their semantic source signature"
        );

        let mut empty_lists = definition.clone();
        empty_lists["filters"] = json!([]);
        empty_lists["sorts"] = json!([]);
        let mut omitted_empty_lists = empty_lists.clone();
        omitted_empty_lists.as_object_mut().unwrap().remove("filters");
        omitted_empty_lists.as_object_mut().unwrap().remove("sorts");
        assert_eq!(
            source_signature(&canonical_target, Some("queries"), &canonical_block, &empty_lists),
            source_signature(&canonical_target, Some("queries"), &canonical_block, &omitted_empty_lists),
            "omitted empty filter/sort lists are equivalent to explicit empty lists"
        );

        definition["filters"][0]["id"] = json!("editor-filter-b");
        definition["sorts"][0]["id"] = json!("editor-sort-b");
        assert_eq!(
            source_signature(&canonical_target, Some("queries"), &canonical_block, &definition),
            canonical_signature,
            "generated filter/sort row ids do not change semantic identity"
        );
        definition["filters"][0]["value"]["id"] = json!("option-b");
        assert_ne!(
            source_signature(&canonical_target, Some("queries"), &canonical_block, &definition),
            canonical_signature,
            "nested option ids are filter meaning and must remain significant"
        );
        let mut different_group = definition.clone();
        different_group["group_by"] = json!("assignee");
        assert_ne!(
            source_signature(&canonical_target, Some("queries"), &canonical_block, &different_group),
            source_signature(&canonical_target, Some("queries"), &canonical_block, &definition)
        );
        let mut different_space = canonical_target.clone();
        different_space.space_id = "space-b".into();
        assert_ne!(
            source_signature(&canonical_target, Some("queries"), &canonical_block, &definition),
            source_signature(&different_space, Some("queries"), &canonical_block, &definition)
        );
        assert!(source_signature(&inline_target, None, &canonical_block, &definition).is_none());
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
