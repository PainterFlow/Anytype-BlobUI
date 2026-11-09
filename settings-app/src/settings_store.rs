//! Split, lossless storage for the legacy BlobUI settings aggregate.
//!
//! `root` is the app installation root. The aggregate is stored under
//! `root/Settings`; its original object shape is described by `data.json` so
//! old and future fields can pass through without normalization.

use serde_json::{json, Map, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SETTINGS_DIR: &str = "Settings";
const FORMAT_VERSION: u64 = 1;
const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;
const DATA_FILE: &str = "data.json";
const META_FILE: &str = "blob.meta.json";
const BASIC_FILE: &str = "basic-settings.json";
const ADVANCED_FILE: &str = "advanced-settings.json";
const CALENDAR_FILE: &str = "calendar-events.json";
const KANBAN_FILE: &str = "kanban-calc.json";
const KEYBINDS_FILE: &str = "keybinds.json";
const ANYBLOB_SETTINGS_FILE: &str = "anyblob/settings.json";
const ANYBLOB_BRIDGE_FILE: &str = "anyblob/bridge.json";
const ANYBLOB_PROJECTS_FILE: &str = "anyblob/projects.json";
const ANYBLOB_PERMISSIONS_FILE: &str = "anyblob/permissions.json";
const PRIVATE_CONNECTION_FILE: &str = "DO-NOT-SHARE/connection.json";
const PRIVATE_CREDENTIAL_FILE: &str = "DO-NOT-SHARE/anytype-api-key.dpapi";

static UNIQUE_ID: AtomicU64 = AtomicU64::new(0);

/// Load the split settings files and reconstruct the legacy aggregate value.
///
/// A missing or malformed component is an error. Returning a partial aggregate
/// could make a later save erase settings that were absent from an import.
pub fn load(root: &Path) -> Result<Value, String> {
    let directory = root.join(SETTINGS_DIR);
    reject_directory_link(&directory)?;

    let metadata = read_json(&directory.join(META_FILE), "settings metadata")?;
    require_format(&metadata, "settings metadata")?;
    let data = read_json(&directory.join(DATA_FILE), "settings data")?;
    require_format(&data, "settings data")?;
    let basic = read_json(&directory.join(BASIC_FILE), "basic settings")?;
    require_format(&basic, "basic settings")?;
    let advanced = read_json(&directory.join(ADVANCED_FILE), "advanced settings")?;
    require_format(&advanced, "advanced settings")?;
    let calendar = read_json(&directory.join(CALENDAR_FILE), "calendar settings")?;
    require_format(&calendar, "calendar settings")?;
    let kanban = read_json(&directory.join(KANBAN_FILE), "Kanban settings")?;
    require_format(&kanban, "Kanban settings")?;
    let keybinds = read_json(&directory.join(KEYBINDS_FILE), "keyboard shortcuts")?;
    require_format(&keybinds, "keyboard shortcuts")?;
    let anyblob_settings = read_json(&directory.join(ANYBLOB_SETTINGS_FILE), "Anyblob settings")?;
    require_format(&anyblob_settings, "Anyblob settings")?;
    let anyblob_projects = read_json(&directory.join(ANYBLOB_PROJECTS_FILE), "Anyblob projects")?;
    require_format(&anyblob_projects, "Anyblob projects")?;
    let anyblob_permissions = read_json(&directory.join(ANYBLOB_PERMISSIONS_FILE), "Anyblob permissions")?;
    require_format(&anyblob_permissions, "Anyblob permissions")?;

    let mut aggregate = object_field(&data, "fields", "settings data")?.clone();
    merge_object_fields(&mut aggregate, object_field(&calendar, "fields", "calendar settings")?);
    merge_object_fields(&mut aggregate, object_field(&kanban, "aggregateFields", "Kanban settings")?);
    if let Some(preferences) = advanced.get("updaterPreferences") {
        aggregate.insert("updaterPreferences".into(), preferences.clone());
    }

    let private_state = object_field(&data, "privateFields", "settings data")?;
    if private_state.get("apiConnectionPresent").and_then(Value::as_bool) == Some(true) {
        aggregate.insert(
            "apiConnection".into(),
            read_json(&directory.join(PRIVATE_CONNECTION_FILE), "private connection settings")?,
        );
    }

    let shape = object_field(&data, "shape", "settings data")?;
    let basic_locations = object_field(&basic, "locations", "basic settings")?;
    let advanced_locations = object_field(&advanced, "locations", "advanced settings")?;
    let projects_by_location = anyblob_projects
        .pointer("/aggregate/locations")
        .and_then(Value::as_object)
        .ok_or("The Anyblob projects file is malformed.")?;
    let permissions_by_location = anyblob_permissions
        .pointer("/aggregate/locations")
        .and_then(Value::as_object)
        .ok_or("The Anyblob permissions file is malformed.")?;
    let anyblob_by_location = anyblob_settings
        .pointer("/aggregate/locations")
        .and_then(Value::as_object)
        .ok_or("The Anyblob settings file is malformed.")?;

    if let Some(settings) = restore_setting_value(
        shape.get("settings").ok_or("The settings shape is missing.")?,
        basic_locations.get("settings"),
        advanced_locations.get("settings"),
        anyblob_by_location.get("settings"),
        projects_by_location.get("settings"),
        permissions_by_location.get("settings"),
    )? {
        aggregate.insert("settings".into(), settings);
    }

    let settings_v2_shape = shape
        .get("settingsV2")
        .ok_or("The settingsV2 shape is missing.")?;
    if let Some(settings_v2) = restore_settings_v2(
        settings_v2_shape,
        basic_locations.get("settingsV2.settings"),
        advanced_locations.get("settingsV2.settings"),
        anyblob_by_location.get("settingsV2.settings"),
        projects_by_location.get("settingsV2.settings"),
        permissions_by_location.get("settingsV2.settings"),
    )? {
        aggregate.insert("settingsV2".into(), settings_v2);
    }

    if keybinds.get("present").and_then(Value::as_bool) == Some(true) {
        let value = keybinds
            .get("value")
            .ok_or("The keyboard shortcut value is missing.")?;
        aggregate.insert("keybinds".into(), value.clone());
    }

    // `blob.meta.json` is intentionally not copied into the returned legacy
    // aggregate. It describes the on-disk split format rather than user data.
    Ok(Value::Object(aggregate))
}

/// Save a legacy aggregate into `root/Settings` with a staged directory swap.
///
/// Non-JSON files already in Settings are copied into the staged directory and
/// remain untouched. The `apiConnection` object is written under
/// `DO-NOT-SHARE`; credential bytes are handled only by the migration helper.
pub fn save(root: &Path, data: &Value) -> Result<(), String> {
    save_internal(root, data, MigrationSeed::default())
}

/// Save the aggregate and the current Kanban formula configuration together.
#[cfg(not(test))]
pub fn save_with_formula(root: &Path, data: &Value, configuration: &Value) -> Result<(), String> {
    save_internal(
        root,
        data,
        MigrationSeed {
            formula_configuration: Some(configuration.clone()),
            ..MigrationSeed::default()
        },
    )
}

/// Save the aggregate and Anyblob's opaque verifier state together.
#[cfg(not(test))]
pub fn save_with_anyblob_state(root: &Path, data: &Value, state: &Value) -> Result<(), String> {
    save_internal(
        root,
        data,
        MigrationSeed {
            anyblob_state: Some(state.clone()),
            ..MigrationSeed::default()
        },
    )
}

/// Save updater choices in `advanced-settings.json` while keeping other settings intact.
#[cfg(not(test))]
pub fn save_updater_preferences(root: &Path, preferences: &Value) -> Result<(), String> {
    let mut data = load(root)?;
    data["updaterPreferences"] = Value::Object(
        preferences
            .as_object()
            .cloned()
            .ok_or("Updater preferences must be a JSON object.")?,
    );
    save(root, &data)
}

/// Migrate a legacy Anytype `blob` directory into `root/Settings`.
///
/// This reads `blob/data.json`, copies `assets`, the existing Anyblob state,
/// Kanban formula configuration, metadata, and the encrypted DPAPI credential.
/// The source directory is never changed. Credential bytes are copied without
/// parsing or decrypting them.
pub fn migrate_legacy_blob(source_blob: &Path, root: &Path) -> Result<(), String> {
    reject_directory_link(source_blob)?;
    if !source_blob.is_dir() {
        return Err("The legacy BlobUI folder does not exist.".into());
    }
    let source = source_blob
        .canonicalize()
        .map_err(|error| format!("Could not locate the legacy BlobUI folder: {error}"))?;
    let target = root
        .join(SETTINGS_DIR)
        .canonicalize()
        .ok();
    if target.as_ref().is_some_and(|target| target.starts_with(&source) || source.starts_with(target)) {
        return Err("The legacy BlobUI folder and Settings folder must be separate.".into());
    }

    let data = read_json(&source.join(DATA_FILE), "legacy BlobUI settings")?;
    if !data.is_object() {
        return Err("Legacy BlobUI settings must contain a JSON object.".into());
    }
    let mut seed = MigrationSeed::default();
    seed.assets = Some(source.join("assets"));
    seed.metadata = read_optional_json(&source.join(META_FILE), "legacy BlobUI metadata")?;
    seed.anyblob_state = read_optional_json(&source.join("anyblob.json"), "legacy Anyblob settings")?;
    seed.formula_configuration = read_optional_json(
        &source.join(".blobui-kanban-formulas.json"),
        "legacy Kanban configuration",
    )?;
    seed.credential = read_optional_bytes(&source.join("credentials/anytype-api-key.dpapi"))?;
    save_internal(root, &data, seed)
}

#[derive(Default)]
struct MigrationSeed {
    assets: Option<PathBuf>,
    metadata: Option<Value>,
    anyblob_state: Option<Value>,
    formula_configuration: Option<Value>,
    credential: Option<Vec<u8>>,
}

fn save_internal(root: &Path, data: &Value, seed: MigrationSeed) -> Result<(), String> {
    let original = data
        .as_object()
        .ok_or("BlobUI settings must contain a JSON object.")?;
    fs::create_dir_all(root).map_err(|error| format!("Could not prepare the app folder: {error}"))?;
    reject_directory_link(root)?;

    let destination = root.join(SETTINGS_DIR);
    reject_directory_link(&destination)?;
    let staged = unique_sibling(&destination, "staged");
    let rollback = unique_sibling(&destination, "rollback");
    if destination.exists() {
        if let Err(error) = copy_tree(&destination, &staged) {
            let _ = fs::remove_dir_all(&staged);
            return Err(error);
        }
    } else {
        fs::create_dir(&staged).map_err(|error| format!("Could not stage Settings: {error}"))?;
    }

    let result = prepare_staged(&staged, original, seed);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staged);
        return Err(error);
    }

    let had_destination = destination.exists();
    if had_destination {
        fs::rename(&destination, &rollback)
            .map_err(|error| {
                let _ = fs::remove_dir_all(&staged);
                format!("Could not prepare to replace Settings: {error}")
            })?;
    }
    if let Err(error) = fs::rename(&staged, &destination) {
        if had_destination {
            if let Err(rollback_error) = fs::rename(&rollback, &destination) {
                return Err(format!(
                    "Could not install staged Settings ({error}); the previous Settings folder remains at {}: {rollback_error}",
                    rollback.display()
                ));
            }
        }
        let _ = fs::remove_dir_all(&staged);
        return Err(format!("Could not install staged Settings: {error}"));
    }

    // A cleanup failure does not roll back a fully committed Settings folder.
    // Leaving a sibling recovery copy is safer than reporting a failed save.
    if had_destination {
        let _ = fs::remove_dir_all(&rollback);
    }
    Ok(())
}

fn prepare_staged(
    staged: &Path,
    original: &Map<String, Value>,
    seed: MigrationSeed,
) -> Result<(), String> {
    let mut residual = Map::new();
    let mut calendar_fields = Map::new();
    let mut kanban_fields = Map::new();
    let mut basic_settings = Map::new();
    let mut advanced_settings = Map::new();
    let mut updater_preferences = None;
    let mut anyblob_descriptors = Map::new();
    let mut anyblob_projects = Map::new();
    let mut anyblob_permissions = Map::new();
    let mut connection_present = false;
    let mut connection_value = None;
    let mut keybinds_present = false;
    let mut keybinds_value = Value::Null;

    for (key, value) in original {
        match key.as_str() {
            "settings" => {
                let _ = split_settings(
                    Some(value),
                    "settings",
                    &mut basic_settings,
                    &mut advanced_settings,
                    &mut anyblob_descriptors,
                    &mut anyblob_projects,
                    &mut anyblob_permissions,
                );
            }
            "settingsV2" => {
                let _ = split_settings_v2(
                    Some(value),
                    &mut basic_settings,
                    &mut advanced_settings,
                    &mut anyblob_descriptors,
                    &mut anyblob_projects,
                    &mut anyblob_permissions,
                );
            }
            "apiConnection" => {
                connection_present = true;
                connection_value = Some(value.clone());
            }
            "keybinds" => {
                keybinds_present = true;
                keybinds_value = value.clone();
            }
            "updaterPreferences" => {
                updater_preferences = Some(value.clone());
            }
            _ if key.starts_with("calendar") => {
                calendar_fields.insert(key.clone(), value.clone());
            }
            _ if key.starts_with("kanban") => {
                kanban_fields.insert(key.clone(), value.clone());
            }
            _ => {
                residual.insert(key.clone(), value.clone());
            }
        }
    }

    // Preserve missing versus explicit-null section shapes. The partitioning
    // pass above only distributes object members to their destination files.
    let settings_shape = original
        .get("settings")
        .map(shape_for_settings)
        .unwrap_or_else(shape_missing);
    let settings_v2_shape = original
        .get("settingsV2")
        .map(shape_for_settings_v2)
        .unwrap_or_else(shape_missing);

    let mut data_file = json!({
        "formatVersion": FORMAT_VERSION,
        "fields": residual,
        "shape": { "settings": settings_shape, "settingsV2": settings_v2_shape },
        "privateFields": { "apiConnectionPresent": connection_present },
    });

    let mut meta = existing_metadata(staged, seed.metadata.as_ref())?;
    set_field(&mut meta, "formatVersion", json!(FORMAT_VERSION));
    set_field(&mut meta, "blobuiVersion", json!(env!("CARGO_PKG_VERSION")));
    if meta.get("anyblobVersion").is_none() {
        set_field(&mut meta, "anyblobVersion", Value::Null);
    }
    if meta.get("latestPublicVersion").is_none() {
        set_field(&mut meta, "latestPublicVersion", Value::Null);
    }

    let bridge_state = seed.anyblob_state.clone();
    let prior_anyblob = read_optional_json(&staged.join(ANYBLOB_SETTINGS_FILE), "Anyblob settings")?;
    let prior_kanban = read_optional_json(&staged.join(KANBAN_FILE), "Kanban settings")?;
    let mut anyblob_settings = merge_managed_file(
        prior_anyblob,
        "aggregate",
        json!({ "locations": anyblob_descriptors }),
    );
    if let Some(state) = seed.anyblob_state {
        set_field(&mut anyblob_settings, "legacyState", state);
    }
    set_field(&mut anyblob_settings, "formatVersion", json!(FORMAT_VERSION));

    let mut projects_file = merged_locations_file(
        read_optional_json(&staged.join(ANYBLOB_PROJECTS_FILE), "Anyblob projects")?,
        anyblob_projects,
    );
    set_field(&mut projects_file, "formatVersion", json!(FORMAT_VERSION));
    let mut permissions_file = merged_locations_file(
        read_optional_json(&staged.join(ANYBLOB_PERMISSIONS_FILE), "Anyblob permissions")?,
        anyblob_permissions,
    );
    set_field(&mut permissions_file, "formatVersion", json!(FORMAT_VERSION));

    let mut advanced_file = merge_managed_file(
        read_optional_json(&staged.join(ADVANCED_FILE), "advanced settings")?,
        "locations",
        json!({
            "settings": advanced_settings.get("settings").cloned().unwrap_or_else(empty_object),
            "settingsV2.settings": advanced_settings.get("settingsV2.settings").cloned().unwrap_or_else(empty_object),
        }),
    );
    if let Some(preferences) = updater_preferences {
        set_field(&mut advanced_file, "updaterPreferences", preferences);
    }
    set_field(&mut advanced_file, "formatVersion", json!(FORMAT_VERSION));

    let mut kanban_file = merge_managed_file(
        prior_kanban,
        "aggregateFields",
        json!(kanban_fields),
    );
    if let Some(configuration) = seed.formula_configuration {
        set_field(&mut kanban_file, "legacyConfiguration", configuration);
    }
    set_field(&mut kanban_file, "formatVersion", json!(FORMAT_VERSION));

    ensure_directory(staged, "assets")?;
    ensure_directory(staged, "anyblob")?;
    ensure_directory(staged, "DO-NOT-SHARE")?;
    if let Some(source_assets) = seed.assets {
        if source_assets.exists() {
            copy_tree_contents(&source_assets, &staged.join("assets"))?;
        }
    }
    if let Some(credential) = seed.credential {
        write_bytes(&staged.join(PRIVATE_CREDENTIAL_FILE), &credential)?;
    }
    let private_credential = staged.join(PRIVATE_CREDENTIAL_FILE);
    if private_credential.is_file() {
        crate::storage::restrict_credential_acl(&private_credential)?;
    }
    if let Some(state) = bridge_state {
        write_json(&staged.join(ANYBLOB_BRIDGE_FILE), &state)?;
    }
    if let Some(connection) = connection_value {
        write_json(&staged.join(PRIVATE_CONNECTION_FILE), &connection)?;
    } else if !connection_present {
        remove_managed_file(&staged.join(PRIVATE_CONNECTION_FILE))?;
    }

    set_field(&mut data_file, "formatVersion", json!(FORMAT_VERSION));
    write_json(&staged.join(META_FILE), &meta)?;
    write_json(&staged.join(DATA_FILE), &data_file)?;
    write_json(
        &staged.join(BASIC_FILE),
        &json!({ "formatVersion": FORMAT_VERSION, "locations": {
            "settings": basic_settings.get("settings").cloned().unwrap_or_else(empty_object),
            "settingsV2.settings": basic_settings.get("settingsV2.settings").cloned().unwrap_or_else(empty_object),
        }}),
    )?;
    write_json(&staged.join(ADVANCED_FILE), &advanced_file)?;
    write_json(
        &staged.join(CALENDAR_FILE),
        &json!({ "formatVersion": FORMAT_VERSION, "fields": calendar_fields }),
    )?;
    write_json(&staged.join(KANBAN_FILE), &kanban_file)?;
    write_json(
        &staged.join(KEYBINDS_FILE),
        &json!({ "formatVersion": FORMAT_VERSION, "present": keybinds_present, "value": keybinds_value }),
    )?;
    write_json(&staged.join(ANYBLOB_SETTINGS_FILE), &anyblob_settings)?;
    write_json(&staged.join(ANYBLOB_PROJECTS_FILE), &projects_file)?;
    write_json(&staged.join(ANYBLOB_PERMISSIONS_FILE), &permissions_file)?;
    Ok(())
}

fn split_settings(
    value: Option<&Value>,
    location: &str,
    basic: &mut Map<String, Value>,
    advanced: &mut Map<String, Value>,
    anyblob: &mut Map<String, Value>,
    projects: &mut Map<String, Value>,
    permissions: &mut Map<String, Value>,
) -> Value {
    let Some(value) = value else { return shape_missing() };
    let Some(fields) = value.as_object() else {
        return json!({ "kind": "value", "value": value });
    };
    for (key, setting) in fields {
        if key == "codexIntegration" {
            split_codex_integration(
                setting,
                location,
                anyblob,
                projects,
                permissions,
            );
        } else if is_advanced_setting(key) {
            let section = merge_member(advanced.get(location), key, setting);
            advanced.insert(location.into(), section);
        } else {
            let section = merge_member(basic.get(location), key, setting);
            basic.insert(location.into(), section);
        }
    }
    json!({ "kind": "object" })
}

fn split_settings_v2(
    value: Option<&Value>,
    basic: &mut Map<String, Value>,
    advanced: &mut Map<String, Value>,
    anyblob: &mut Map<String, Value>,
    projects: &mut Map<String, Value>,
    permissions: &mut Map<String, Value>,
) -> Value {
    let Some(value) = value else { return shape_missing() };
    let Some(container) = value.as_object() else {
        return json!({ "kind": "value", "value": value });
    };
    let Some(settings) = container.get("settings") else {
        return json!({ "kind": "object", "rest": value, "settings": shape_missing() });
    };
    let settings_shape = split_settings(
        Some(settings),
        "settingsV2.settings",
        basic,
        advanced,
        anyblob,
        projects,
        permissions,
    );
    let mut rest = container.clone();
    rest.remove("settings");
    json!({ "kind": "object", "rest": rest, "settings": settings_shape })
}

fn shape_for_settings(value: &Value) -> Value {
    if value.is_object() { json!({ "kind": "object" }) }
    else { json!({ "kind": "value", "value": value }) }
}

fn shape_for_settings_v2(value: &Value) -> Value {
    let Some(container) = value.as_object() else {
        return json!({ "kind": "value", "value": value });
    };
    let mut rest = container.clone();
    let setting = rest.remove("settings");
    let settings_shape = match setting {
        Some(settings) => shape_for_settings(&settings),
        None => shape_missing(),
    };
    json!({ "kind": "object", "rest": rest, "settings": settings_shape })
}

fn split_codex_integration(
    value: &Value,
    location: &str,
    anyblob: &mut Map<String, Value>,
    projects: &mut Map<String, Value>,
    permissions: &mut Map<String, Value>,
) {
    let Some(integration) = value.as_object() else {
        anyblob.insert(location.into(), json!({ "kind": "value", "value": value }));
        return;
    };
    let mut base = integration.clone();
    if let Some(value) = base.remove("projects") {
        projects.insert(location.into(), value);
    }
    if let Some(value) = base.remove("spaces") {
        permissions.insert(location.into(), value);
    }
    anyblob.insert(location.into(), json!({ "kind": "object", "base": base }));
}

fn restore_setting_value(
    shape: &Value,
    basic: Option<&Value>,
    advanced: Option<&Value>,
    anyblob: Option<&Value>,
    projects: Option<&Value>,
    permissions: Option<&Value>,
) -> Result<Option<Value>, String> {
    match shape.get("kind").and_then(Value::as_str) {
        Some("missing") => Ok(None),
        Some("value") => Ok(Some(
            shape.get("value").ok_or("A saved settings value is missing.")?.clone(),
        )),
        Some("object") => {
            let mut fields = Map::new();
            merge_object_fields(&mut fields, as_object_or_empty(basic, "basic settings location")?);
            merge_object_fields(&mut fields, as_object_or_empty(advanced, "advanced settings location")?);
            if let Some(integration) = restore_codex_integration(anyblob, projects, permissions)? {
                fields.insert("codexIntegration".into(), integration);
            }
            Ok(Some(Value::Object(fields)))
        }
        _ => Err("Unsupported saved settings shape.".into()),
    }
}

fn restore_settings_v2(
    shape: &Value,
    basic: Option<&Value>,
    advanced: Option<&Value>,
    anyblob: Option<&Value>,
    projects: Option<&Value>,
    permissions: Option<&Value>,
) -> Result<Option<Value>, String> {
    match shape.get("kind").and_then(Value::as_str) {
        Some("missing") => Ok(None),
        Some("value") => Ok(Some(
            shape.get("value").ok_or("A saved settingsV2 value is missing.")?.clone(),
        )),
        Some("object") => {
            let mut fields = object_field(shape, "rest", "settingsV2 shape")?.clone();
            let settings_shape = shape
                .get("settings")
                .ok_or("The nested settings shape is missing.")?;
            if let Some(settings) = restore_setting_value(
                settings_shape,
                basic,
                advanced,
                anyblob,
                projects,
                permissions,
            )? {
                fields.insert("settings".into(), settings);
            }
            Ok(Some(Value::Object(fields)))
        }
        _ => Err("Unsupported saved settingsV2 shape.".into()),
    }
}

fn restore_codex_integration(
    descriptor: Option<&Value>,
    projects: Option<&Value>,
    permissions: Option<&Value>,
) -> Result<Option<Value>, String> {
    let Some(descriptor) = descriptor else { return Ok(None) };
    match descriptor.get("kind").and_then(Value::as_str) {
        Some("value") => Ok(Some(
            descriptor
                .get("value")
                .ok_or("An Anyblob integration value is missing.")?
                .clone(),
        )),
        Some("object") => {
            let mut integration = object_field(descriptor, "base", "Anyblob settings location")?.clone();
            if let Some(projects) = projects {
                integration.insert("projects".into(), projects.clone());
            }
            if let Some(permissions) = permissions {
                integration.insert("spaces".into(), permissions.clone());
            }
            Ok(Some(Value::Object(integration)))
        }
        _ => Err("Unsupported Anyblob integration shape.".into()),
    }
}

fn is_advanced_setting(key: &str) -> bool {
    matches!(key, "backgroundExperiment" | "autoUpdate" | "updateChannel")
        || key.starts_with("experiment")
}

fn shape_missing() -> Value {
    json!({ "kind": "missing" })
}

fn merge_member(existing: Option<&Value>, key: &str, value: &Value) -> Value {
    let mut map = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    map.insert(key.into(), value.clone());
    Value::Object(map)
}

fn merged_locations_file(existing: Option<Value>, locations: Map<String, Value>) -> Value {
    let mut result = existing
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    result.insert("formatVersion".into(), json!(FORMAT_VERSION));
    result.insert("aggregate".into(), json!({ "locations": locations }));
    Value::Object(result)
}

fn merge_managed_file(existing: Option<Value>, managed_key: &str, managed_value: Value) -> Value {
    let mut result = match existing {
        Some(Value::Object(mut object)) => {
            object.remove("formatVersion");
            object.remove(managed_key);
            object
        }
        Some(value) => Map::from_iter([("preservedLegacyValue".into(), value)]),
        None => Map::new(),
    };
    result.insert("formatVersion".into(), json!(FORMAT_VERSION));
    result.insert(managed_key.into(), managed_value);
    Value::Object(result)
}

fn existing_metadata(staged: &Path, migration_metadata: Option<&Value>) -> Result<Value, String> {
    let existing = read_optional_json(&staged.join(META_FILE), "settings metadata")?;
    let mut metadata = match existing.or_else(|| migration_metadata.cloned()) {
        Some(Value::Object(object)) => object,
        Some(value) => Map::from_iter([("preservedLegacyValue".into(), value)]),
        None => Map::new(),
    };
    metadata.remove("aggregateShape");
    Ok(Value::Object(metadata))
}

fn object_field<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("The {label} file is malformed."))
}

fn as_object_or_empty<'a>(value: Option<&'a Value>, label: &str) -> Result<&'a Map<String, Value>, String> {
    match value {
        Some(Value::Object(value)) => Ok(value),
        Some(_) => Err(format!("The {label} file is malformed.")),
        None => Ok(empty_map_ref()),
    }
}

fn empty_map_ref() -> &'static Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(Map::new)
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn merge_object_fields(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (key, value) in source {
        target.insert(key.clone(), value.clone());
    }
}

fn set_field(value: &mut Value, key: &str, field: Value) {
    if !value.is_object() {
        *value = empty_object();
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(key.into(), field);
    }
}

fn require_format(value: &Value, label: &str) -> Result<(), String> {
    if value.get("formatVersion").and_then(Value::as_u64) == Some(FORMAT_VERSION) {
        Ok(())
    } else {
        Err(format!("The {label} file has an unsupported format version."))
    }
}

fn read_json(path: &Path, label: &str) -> Result<Value, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| format!("The Settings folder is incomplete. {label} is missing."))?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() || metadata.len() > MAX_JSON_BYTES {
        return Err(format!("The Settings folder contains unsupported {label}."));
    }
    let bytes = fs::read(path).map_err(|_| format!("Could not read {label}."))?;
    serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
        .map_err(|_| format!("The Settings folder contains invalid {label}."))
}

fn read_optional_json(path: &Path, label: &str) -> Result<Option<Value>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) || !metadata.is_file() || metadata.len() > MAX_JSON_BYTES => {
            Err(format!("The existing {label} file is unsupported."))
        }
        Ok(_) => read_json(path, label).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Could not inspect {label}: {error}")),
    }
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) || !metadata.is_file() => {
            Err("The legacy API credential is not a regular file.".into())
        }
        Ok(_) => fs::read(path)
            .map(Some)
            .map_err(|error| format!("Could not copy the legacy API credential: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Could not inspect the legacy API credential: {error}")),
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Could not encode {}: {error}", path.display()))?;
    bytes.push(b'\n');
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("The Settings path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("Could not flush {}: {error}", path.display()))
}

fn ensure_directory(root: &Path, relative: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(format!("The Settings path is not a regular folder: {}", path.display()));
        }
    }
    fs::create_dir_all(&path).map_err(|error| format!("Could not create {}: {error}", path.display()))
}

fn remove_managed_file(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) || !metadata.is_file() => {
            Err(format!("The Settings path is not a regular file: {}", path.display()))
        }
        Ok(_) => fs::remove_file(path)
            .map_err(|error| format!("Could not remove {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not inspect {}: {error}", path.display())),
    }
}

fn reject_directory_link(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) => {
            Err(format!("A Settings folder is a link and cannot be changed safely: {}", path.display()))
        }
        Ok(metadata) if !metadata.is_dir() => {
            Err(format!("A Settings path is not a folder: {}", path.display()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not inspect {}: {error}", path.display())),
    }
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    reject_directory_link(source)?;
    fs::create_dir(destination)
        .map_err(|error| format!("Could not stage Settings: {error}"))?;
    copy_tree_contents(source, destination)
}

fn copy_tree_contents(source: &Path, destination: &Path) -> Result<(), String> {
    reject_directory_link(source)?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| format!("Could not read {}: {error}", source.display()))?
        .map(|entry| entry.map(|entry| entry.path()).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
        if is_link_or_reparse(&metadata) {
            return Err("A Settings folder contains a link and cannot be copied safely.".into());
        }
        let target = destination.join(
            path.file_name()
                .ok_or("A Settings path has no file name.")?,
        );
        if metadata.is_dir() {
            if target.exists() {
                reject_directory_link(&target)?;
            } else {
                fs::create_dir(&target)
                    .map_err(|error| format!("Could not create {}: {error}", target.display()))?;
            }
            copy_tree_contents(&path, &target)?;
            fs::set_permissions(&target, metadata.permissions())
                .map_err(|error| format!("Could not preserve folder permissions: {error}"))?;
        } else if metadata.is_file() {
            if target.is_dir() {
                return Err(format!("A Settings file conflicts with a folder: {}", target.display()));
            }
            fs::copy(&path, &target)
                .map_err(|error| format!("Could not preserve {}: {error}", path.display()))?;
        } else {
            return Err("A Settings folder contains an unsupported file type.".into());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn unique_sibling(path: &Path, suffix: &str) -> PathBuf {
    let id = UNIQUE_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{name}.{suffix}-{}-{nanos}-{id}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../.test-run/settings-store-fixtures")
                .join(format!("{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn split_round_trip_preserves_unknown_fields_and_nested_anyblob_values() {
        let temp = TempDir::new();
        let root = &temp.0;
        let original = json!({
            "installDirectory": "C:/Anytype",
            "futureTopLevel": {"unknown": [1, {"keep": true}]},
            "apiConnection": {"enabled": true, "ip": "127.0.0.1", "port": 31010, "future": "private"},
            "settings": {
                "fontFamily": "Inter",
                "backgroundExperiment": "normal",
                "futureSetting": {"nested": ["kept"]},
                "codexIntegration": {
                    "objectMentions": {"types": ["task"], "futureMentionField": 7},
                    "spaces": {"space-id": {"access": "read", "futurePermission": {"enabled": true}}},
                    "projects": {"stable-project": {"name": "Work", "rootPaths": ["C:/work"], "futureProjectField": [1, 2]}},
                    "futureAnyblobField": {"value": 42}
                }
            },
            "settingsV2": {
                "version": 2,
                "futureEnvelope": {"keep": "me"},
                "settings": {
                    "accentMode": "custom",
                    "backgroundExperiment": "autostart-background",
                    "futureV2Setting": {"key": "value"},
                    "codexIntegration": {
                        "objectMentions": {"objects": ["page"], "future": false},
                        "spaces": {"full-space-id": {"access": "createDelete", "future": 1}},
                        "projects": {"stable-id": {"name": "Project", "future": true}},
                        "futureNestedAnyblob": {"deep": {"value": "preserve"}}
                    }
                }
            },
            "calendarColorRules": {"version": 1, "types": [{"spaceId": "space", "unknown": true}]},
            "calendarObjectColors": {"type": "Blue"},
            "kanbanFormulaResults": {"view": {"result": 4}},
            "keybinds": [{"key": "F10", "futureBindingField": true}]
        });

        save(root, &original).unwrap();
        assert_eq!(load(root).unwrap(), original);

        let directory = root.join(SETTINGS_DIR);
        let basic = read_json(&directory.join(BASIC_FILE), "basic").unwrap();
        let advanced = read_json(&directory.join(ADVANCED_FILE), "advanced").unwrap();
        let projects = read_json(&directory.join(ANYBLOB_PROJECTS_FILE), "projects").unwrap();
        let permissions = read_json(&directory.join(ANYBLOB_PERMISSIONS_FILE), "permissions").unwrap();
        assert_eq!(basic["locations"]["settings"]["fontFamily"], "Inter");
        assert_eq!(advanced["locations"]["settings"]["backgroundExperiment"], "normal");
        assert_eq!(projects["aggregate"]["locations"]["settings"]["stable-project"]["futureProjectField"], json!([1, 2]));
        assert_eq!(permissions["aggregate"]["locations"]["settingsV2.settings"]["full-space-id"]["future"], 1);
        assert!(directory.join(PRIVATE_CONNECTION_FILE).is_file());
        assert!(directory.join(PRIVATE_CREDENTIAL_FILE).parent().unwrap().is_dir());
    }

    #[test]
    fn legacy_migration_copies_assets_and_credentials_without_changing_source() {
        let temp = TempDir::new();
        let source = temp.0.join("legacy-blob");
        let root = temp.0.join("install");
        fs::create_dir_all(source.join("assets")).unwrap();
        fs::create_dir_all(source.join("credentials")).unwrap();
        fs::write(source.join("data.json"), br#"{"settings":{"future":"kept"},"apiConnection":{"ip":"127.0.0.1","port":31010}}"#).unwrap();
        fs::write(source.join("blob.meta.json"), br#"{"formatVersion":1,"latestPublicVersion":"9.9.9","futureMeta":"kept"}"#).unwrap();
        fs::write(source.join("anyblob.json"), br#"{"enabled":true,"verifier":"opaque-hash"}"#).unwrap();
        fs::write(source.join(".blobui-kanban-formulas.json"), br#"{"version":2,"display":"balanced","definitions":[]}"#).unwrap();
        fs::write(source.join("credentials/anytype-api-key.dpapi"), [0x01, 0xA2, 0xFE, 0x00]).unwrap();
        fs::write(source.join("assets/banner.png"), [0x89, b'P', b'N', b'G']).unwrap();
        let legacy_data = read_json(&source.join("data.json"), "legacy").unwrap();

        migrate_legacy_blob(&source, &root).unwrap();

        let settings = root.join(SETTINGS_DIR);
        assert_eq!(load(&root).unwrap(), legacy_data);
        assert_eq!(fs::read(settings.join(PRIVATE_CREDENTIAL_FILE)).unwrap(), vec![0x01, 0xA2, 0xFE, 0x00]);
        assert_eq!(fs::read(settings.join("assets/banner.png")).unwrap(), vec![0x89, b'P', b'N', b'G']);
        assert_eq!(read_json(&settings.join(META_FILE), "meta").unwrap()["futureMeta"], "kept");
        assert_eq!(read_json(&settings.join(META_FILE), "meta").unwrap()["latestPublicVersion"], "9.9.9");
        assert_eq!(read_json(&settings.join(ANYBLOB_SETTINGS_FILE), "anyblob").unwrap()["legacyState"]["verifier"], "opaque-hash");
        assert_eq!(read_json(&settings.join(KANBAN_FILE), "kanban").unwrap()["legacyConfiguration"]["version"], 2);
        assert!(source.join("credentials/anytype-api-key.dpapi").is_file());
    }

    #[test]
    fn staging_failure_keeps_the_existing_settings_folder_unchanged() {
        let temp = TempDir::new();
        let root = &temp.0;
        let directory = root.join(SETTINGS_DIR);
        fs::create_dir_all(directory.join("anyblob")).unwrap();
        fs::write(directory.join("sentinel.txt"), b"old settings stay").unwrap();
        // A file where the split writer needs an Anyblob directory fails while
        // preparing the stage, before the current Settings folder is renamed.
        fs::remove_dir(directory.join("anyblob")).unwrap();
        fs::write(directory.join("anyblob"), b"old file stays").unwrap();
        let original = json!({"settings":{"fontFamily":"Inter"}});

        assert!(save(root, &original).is_err());
        assert_eq!(fs::read(directory.join("sentinel.txt")).unwrap(), b"old settings stay");
        assert_eq!(fs::read(directory.join("anyblob")).unwrap(), b"old file stays");
    }

    #[test]
    fn non_json_files_survive_successful_saves() {
        let temp = TempDir::new();
        let root = &temp.0;
        let settings = root.join(SETTINGS_DIR);
        fs::create_dir_all(settings.join("assets")).unwrap();
        fs::write(settings.join("assets/banner.png"), [0x89, b'P', b'N', b'G']).unwrap();
        fs::write(settings.join("custom.bin"), [0x01, 0x02, 0x03]).unwrap();

        save(root, &json!({"settings":{"fontFamily":"Inter"}})).unwrap();

        assert_eq!(fs::read(settings.join("assets/banner.png")).unwrap(), vec![0x89, b'P', b'N', b'G']);
        assert_eq!(fs::read(settings.join("custom.bin")).unwrap(), vec![0x01, 0x02, 0x03]);
    }
}
