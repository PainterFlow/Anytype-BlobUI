//! App settings stored under the installation root.
//!
//! `root` is the app installation root. The aggregate is stored under
//! `root/Settings`, with one copy of each preference.

use serde_json::{json, Map, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SETTINGS_DIR: &str = "Settings";
const FORMAT_VERSION: u64 = 2;
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
static STORE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Load the split settings files and reconstruct the legacy aggregate value.
///
/// A missing or malformed component is an error. Returning a partial aggregate
/// could make a later save erase settings that were absent from an import.
pub fn load(root: &Path) -> Result<Value, String> {
    let _guard = STORE_LOCK.lock().map_err(|_| "The Settings store lock is unavailable.")?;
    load_unlocked(root)
}

fn load_unlocked(root: &Path) -> Result<Value, String> {
    let directory = root.join(SETTINGS_DIR);
    reject_directory_link(&directory)?;
    reject_directory_link(&directory.join("anyblob"))?;
    reject_directory_link(&directory.join("DO-NOT-SHARE"))?;

    let metadata = read_json(&directory.join(META_FILE), "settings metadata")?;
    let version = metadata.get("formatVersion").and_then(Value::as_u64)
        .filter(|version| matches!(*version, 1 | FORMAT_VERSION))
        .ok_or("The settings metadata file has an unsupported format version.")?;
    let data = read_json(&directory.join(DATA_FILE), "settings data")?;
    require_format(&data, version, "settings data")?;
    let basic = read_json(&directory.join(BASIC_FILE), "basic settings")?;
    require_format(&basic, version, "basic settings")?;
    let advanced = read_json(&directory.join(ADVANCED_FILE), "advanced settings")?;
    require_format(&advanced, version, "advanced settings")?;
    let calendar = read_json(&directory.join(CALENDAR_FILE), "calendar settings")?;
    require_format(&calendar, version, "calendar settings")?;
    let kanban = read_json(&directory.join(KANBAN_FILE), "Kanban settings")?;
    require_format(&kanban, version, "Kanban settings")?;
    let keybinds = read_json(&directory.join(KEYBINDS_FILE), "keyboard shortcuts")?;
    require_format(&keybinds, version, "keyboard shortcuts")?;
    let anyblob_settings = read_json(&directory.join(ANYBLOB_SETTINGS_FILE), "Anyblob settings")?;
    require_format(&anyblob_settings, version, "Anyblob settings")?;
    let anyblob_projects = read_json(&directory.join(ANYBLOB_PROJECTS_FILE), "Anyblob projects")?;
    require_format(&anyblob_projects, version, "Anyblob projects")?;
    let anyblob_permissions = read_json(&directory.join(ANYBLOB_PERMISSIONS_FILE), "Anyblob permissions")?;
    require_format(&anyblob_permissions, version, "Anyblob permissions")?;

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
    if version == 1 {
        restore_legacy_locations(&mut aggregate, shape, &basic, &advanced, &anyblob_settings, &anyblob_projects, &anyblob_permissions)?;
        crate::consolidate_legacy_settings(&mut aggregate);
    } else if let Some(settings) = restore_setting_value(
        shape.get("settings").ok_or("The settings shape is missing.")?,
        Some(&basic["settings"]),
        Some(&advanced["settings"]),
        anyblob_settings.get("integration"),
        anyblob_projects.get("projects"),
        anyblob_permissions.get("spaces"),
    )? {
        aggregate.insert("settings".into(), settings);
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

fn restore_legacy_locations(
    aggregate: &mut Map<String, Value>,
    shape: &Map<String, Value>,
    basic: &Value,
    advanced: &Value,
    anyblob_settings: &Value,
    anyblob_projects: &Value,
    anyblob_permissions: &Value,
) -> Result<(), String> {
    let basic_locations = object_field(basic, "locations", "basic settings")?;
    let advanced_locations = object_field(advanced, "locations", "advanced settings")?;
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

    Ok(())
}

/// Upgrade the split layout or migrate the original Anytype blob on first use.
pub fn ensure(root: &Path, legacy_blob: &Path) -> Result<bool, String> {
    let _guard = STORE_LOCK.lock().map_err(|_| "The Settings store lock is unavailable.")?;
    reject_directory_link(root)?;
    let directory = root.join(SETTINGS_DIR);
    reject_directory_link(&directory)?;
    if directory.is_dir() {
        let metadata = read_json(&directory.join(META_FILE), "settings metadata")?;
        if metadata.get("formatVersion").and_then(Value::as_u64) == Some(1) {
            let data = load_unlocked(root)?;
            save_unlocked(root, &data, MigrationSeed::default())?;
        }
        return Ok(false);
    }
    reject_directory_link(legacy_blob)?;
    if read_optional_json(&legacy_blob.join(DATA_FILE), "legacy BlobUI settings")?.is_none() {
        return Ok(false);
    }
    migrate_legacy_blob_unlocked(legacy_blob, root)?;
    Ok(true)
}

/// Save a settings aggregate into `root/Settings` with a staged directory swap.
///
/// Non-JSON files already in Settings are copied into the staged directory and
/// remain untouched. The `apiConnection` object is written under
/// `DO-NOT-SHARE`; credential bytes are handled only by the migration helper.
pub fn save(root: &Path, data: &Value) -> Result<(), String> {
    save_internal(root, data, MigrationSeed::default())
}

pub fn read_formula(root: &Path) -> Result<Option<Value>, String> {
    let _guard = STORE_LOCK.lock().map_err(|_| "The Settings store lock is unavailable.")?;
    let directory = root.join(SETTINGS_DIR);
    reject_directory_link(&directory)?;
    let document = read_json(&directory.join(KANBAN_FILE), "Kanban settings")?;
    match document.get("formatVersion").and_then(Value::as_u64) {
        Some(1) => Ok(document.get("legacyConfiguration").cloned()),
        Some(FORMAT_VERSION) => Ok(document.get("configuration").cloned()),
        _ => Err("The Kanban settings file has an unsupported format version.".into()),
    }
}

/// Save the aggregate and the current Kanban formula configuration together.
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

/// Store updater choices and the latest public version with the current settings.
pub fn save_updater_preferences(root: &Path, preferences: &Value, latest_version: Option<&str>) -> Result<(), String> {
    let _guard = STORE_LOCK.lock().map_err(|_| "The Settings store lock is unavailable.")?;
    let mut data = load_unlocked(root)?;
    let mut saved = data["updaterPreferences"].as_object().cloned().unwrap_or_default();
    merge_object_fields(&mut saved, preferences.as_object().ok_or("Updater preferences must be a JSON object.")?);
    let saved = Value::Object(saved);
    if data.get("updaterPreferences") != Some(&saved) {
        data["updaterPreferences"] = saved;
        save_unlocked(root, &data, MigrationSeed {
            latest_public_version: latest_version.map(str::to_owned),
            ..MigrationSeed::default()
        })?;
    } else if let Some(version) = latest_version {
        let path = root.join(SETTINGS_DIR).join(META_FILE);
        let mut metadata = read_json(&path, "settings metadata")?;
        if metadata["latestPublicVersion"].as_str() != Some(version) {
            metadata["latestPublicVersion"] = json!(version);
            crate::storage::write_atomic(&path, &crate::serialize_data(&metadata)?)?;
        }
    }
    Ok(())
}

/// Migrate a legacy Anytype `blob` directory into `root/Settings`.
///
/// This reads `blob/data.json`, copies `assets`, the existing Anyblob state,
/// Kanban formula configuration, metadata, and the encrypted DPAPI credential.
/// The source directory is never changed. Credential bytes are copied without
/// parsing or decrypting them.
pub fn migrate_legacy_blob(source_blob: &Path, root: &Path) -> Result<(), String> {
    let _guard = STORE_LOCK.lock().map_err(|_| "The Settings store lock is unavailable.")?;
    migrate_legacy_blob_unlocked(source_blob, root)
}

fn migrate_legacy_blob_unlocked(source_blob: &Path, root: &Path) -> Result<(), String> {
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
    save_unlocked(root, &data, seed)
}

#[derive(Default)]
struct MigrationSeed {
    assets: Option<PathBuf>,
    metadata: Option<Value>,
    anyblob_state: Option<Value>,
    formula_configuration: Option<Value>,
    latest_public_version: Option<String>,
    credential: Option<Vec<u8>>,
}

fn save_internal(root: &Path, data: &Value, seed: MigrationSeed) -> Result<(), String> {
    let _guard = STORE_LOCK.lock().map_err(|_| "The Settings store lock is unavailable.")?;
    save_unlocked(root, data, seed)
}

fn save_unlocked(root: &Path, data: &Value, seed: MigrationSeed) -> Result<(), String> {
    let mut canonical = data.clone();
    if let Some(fields) = canonical.as_object_mut() {
        crate::consolidate_legacy_settings(fields);
    }
    let original = canonical
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
    let mut anyblob_integration = None;
    let mut anyblob_projects = None;
    let mut anyblob_permissions = None;
    let mut connection_present = false;
    let mut connection_value = None;
    let mut keybinds_present = false;
    let mut keybinds_value = Value::Null;

    for (key, value) in original {
        match key.as_str() {
            "settings" => {
                if let Some(settings) = value.as_object() {
                    for (name, setting) in settings {
                        if name == "codexIntegration" {
                            anyblob_integration = Some(match setting.as_object() {
                                Some(integration) => {
                                    let mut fields = integration.clone();
                                    anyblob_projects = fields.remove("projects");
                                    anyblob_permissions = fields.remove("spaces");
                                    json!({ "kind": "object", "base": fields })
                                }
                                None => json!({ "kind": "value", "value": setting }),
                            });
                        } else if is_advanced_setting(name) {
                            advanced_settings.insert(name.clone(), setting.clone());
                        } else {
                            basic_settings.insert(name.clone(), setting.clone());
                        }
                    }
                }
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
    let data_file = json!({
        "formatVersion": FORMAT_VERSION,
        "fields": residual,
        "shape": { "settings": settings_shape },
        "privateFields": { "apiConnectionPresent": connection_present },
    });

    let mut meta = existing_metadata(staged, seed.metadata.as_ref())?;
    set_field(&mut meta, "formatVersion", json!(FORMAT_VERSION));
    set_field(&mut meta, "blobuiVersion", json!(env!("BLOBUI_BUILD_VERSION")));
    let plugin: Value = serde_json::from_str(include_str!("../../codex-plugin/src/.codex-plugin/plugin.json"))
        .map_err(|_| "The bundled Anyblob version could not be read.")?;
    set_field(&mut meta, "anyblobVersion", plugin["version"].clone());
    if let Some(version) = seed.latest_public_version {
        set_field(&mut meta, "latestPublicVersion", json!(version));
    } else if meta.get("latestPublicVersion").is_none() {
        set_field(&mut meta, "latestPublicVersion", Value::Null);
    }

    let mut anyblob_settings = read_optional_json(&staged.join(ANYBLOB_SETTINGS_FILE), "Anyblob settings")?
        .unwrap_or_else(empty_object);
    let legacy_state = anyblob_settings.as_object_mut().and_then(|fields| {
        fields.remove("aggregate");
        fields.remove("integration");
        fields.remove("legacyState")
    });
    let bridge = staged.join(ANYBLOB_BRIDGE_FILE);
    let previous_bridge = read_optional_json(&bridge, "legacy Anyblob bridge")?;
    if let Some(state) = seed.anyblob_state.or(previous_bridge).or(legacy_state) {
        if let Some(fields) = state.as_object() {
            merge_object_fields(anyblob_settings.as_object_mut().ok_or("The Anyblob settings file is malformed.")?, fields);
        } else {
            return Err("The legacy Anyblob bridge file is malformed.".into());
        }
        remove_managed_file(&bridge)?;
    }
    if let Some(integration) = anyblob_integration {
        set_field(&mut anyblob_settings, "integration", integration);
    }
    if anyblob_settings.get("enabled").is_none() {
        set_field(&mut anyblob_settings, "enabled", Value::Bool(false));
    }
    set_field(&mut anyblob_settings, "formatVersion", json!(FORMAT_VERSION));

    let projects_file = section_file("projects", anyblob_projects);
    let permissions_file = section_file("spaces", anyblob_permissions);
    let mut advanced_file = json!({ "formatVersion": FORMAT_VERSION, "settings": advanced_settings });
    if let Some(preferences) = updater_preferences {
        set_field(&mut advanced_file, "updaterPreferences", preferences);
    }

    let mut prior_kanban = read_optional_json(&staged.join(KANBAN_FILE), "Kanban settings")?;
    let legacy_formula = prior_kanban.as_mut().and_then(Value::as_object_mut)
        .and_then(|fields| fields.remove("legacyConfiguration"));
    let mut kanban_file = merge_managed_file(
        prior_kanban,
        "aggregateFields",
        json!(kanban_fields),
    );
    if let Some(configuration) = seed.formula_configuration
        .or_else(|| legacy_formula.filter(|_| kanban_file.get("configuration").is_none()))
    {
        set_field(&mut kanban_file, "configuration", configuration);
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
    let banner = staged.join("assets/banner.png");
    let legacy_banner = staged.join("assets/spacebanner/banner.png");
    if !banner.exists() && legacy_banner.is_file() {
        fs::rename(&legacy_banner, &banner)
            .map_err(|error| format!("Could not migrate the saved space banner: {error}"))?;
    }
    if let Some(credential) = seed.credential {
        write_bytes(&staged.join(PRIVATE_CREDENTIAL_FILE), &credential)?;
    }
    let private_credential = staged.join(PRIVATE_CREDENTIAL_FILE);
    if private_credential.is_file() {
        crate::storage::restrict_credential_acl(&private_credential)?;
    }
    if let Some(connection) = connection_value {
        write_json(&staged.join(PRIVATE_CONNECTION_FILE), &connection)?;
    } else if !connection_present {
        remove_managed_file(&staged.join(PRIVATE_CONNECTION_FILE))?;
    }

    write_json(&staged.join(META_FILE), &meta)?;
    write_json(&staged.join(DATA_FILE), &data_file)?;
    write_json(
        &staged.join(BASIC_FILE),
        &json!({ "formatVersion": FORMAT_VERSION, "settings": basic_settings }),
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

fn shape_for_settings(value: &Value) -> Value {
    if value.is_object() { json!({ "kind": "object" }) }
    else { json!({ "kind": "value", "value": value }) }
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

fn section_file(key: &str, value: Option<Value>) -> Value {
    let mut section = json!({ "formatVersion": FORMAT_VERSION });
    if let Some(value) = value {
        set_field(&mut section, key, value);
    }
    section
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

fn require_format(value: &Value, version: u64, label: &str) -> Result<(), String> {
    if value.get("formatVersion").and_then(Value::as_u64) == Some(version) {
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
        let mut expected = original.clone();
        crate::consolidate_legacy_settings(expected.as_object_mut().unwrap());
        assert_eq!(load(root).unwrap(), expected);

        let directory = root.join(SETTINGS_DIR);
        let basic = read_json(&directory.join(BASIC_FILE), "basic").unwrap();
        let advanced = read_json(&directory.join(ADVANCED_FILE), "advanced").unwrap();
        let projects = read_json(&directory.join(ANYBLOB_PROJECTS_FILE), "projects").unwrap();
        let permissions = read_json(&directory.join(ANYBLOB_PERMISSIONS_FILE), "permissions").unwrap();
        assert_eq!(basic["settings"]["fontFamily"], "Inter");
        assert_eq!(advanced["settings"]["backgroundExperiment"], "autostart-background");
        assert_eq!(projects["projects"]["stable-id"]["future"], true);
        assert_eq!(permissions, json!({"formatVersion":2,"spaces":{"full-space-id":{"access":"createDelete","future":1}}}));
        assert!(directory.join(PRIVATE_CONNECTION_FILE).is_file());
        assert!(directory.join(PRIVATE_CREDENTIAL_FILE).parent().unwrap().is_dir());
    }

    #[test]
    fn upgrades_a_split_store_without_restoring_revoked_permissions() {
        let temp = TempDir::new();
        let root = &temp.0;
        let directory = root.join(SETTINGS_DIR);
        for (file, content) in [
            (META_FILE, json!({"futureMetadata": true})),
            (DATA_FILE, json!({"fields": {}, "privateFields": {}, "shape": {
                "settings": {"kind": "object"},
                "settingsV2": {"kind": "object", "rest": {"version": 2}, "settings": {"kind": "object"}}
            }})),
            (BASIC_FILE, json!({"locations": {
                "settings": {"fontFamily": "Inter", "includeSelections": true},
                "settingsV2.settings": {"includeSelections": false}
            }})),
            (ADVANCED_FILE, json!({"locations": {}})),
            (CALENDAR_FILE, json!({"fields": {}})),
            (KANBAN_FILE, json!({"aggregateFields": {}, "legacyConfiguration": {"version": 2, "definitions": [], "display": "balanced"}})),
            (KEYBINDS_FILE, json!({"present": false})),
            (ANYBLOB_SETTINGS_FILE, json!({"aggregate": {"locations": {
                "settings": {"kind": "object", "base": {}},
                "settingsV2.settings": {"kind": "object", "base": {}}
            }}, "legacyState": {"enabled": true}})),
            (ANYBLOB_PROJECTS_FILE, json!({"aggregate": {"locations": {}}})),
            (ANYBLOB_PERMISSIONS_FILE, json!({"aggregate": {"locations": {
                "settings": {"revoked": {"access": "createDelete"}},
                "settingsV2.settings": {"kept": {"access": "read"}}
            }}})),
        ] {
            let mut content = content;
            content["formatVersion"] = json!(1);
            write_json(&directory.join(file), &content).unwrap();
        }

        ensure(root, &root.join("legacy")).unwrap();
        let settings = load(root).unwrap();
        assert_eq!(settings["settings"]["fontFamily"], "Inter");
        assert_eq!(settings["settings"]["includeSelections"], false);
        assert!(settings.get("settingsV2").is_none());
        assert_eq!(
            read_json(&directory.join(ANYBLOB_PERMISSIONS_FILE), "permissions").unwrap(),
            json!({"formatVersion": 2, "spaces": {"kept": {"access": "read"}}})
        );
        assert_eq!(read_formula(root).unwrap().unwrap()["version"], 2);
        assert_eq!(read_json(&directory.join(ANYBLOB_SETTINGS_FILE), "Anyblob settings").unwrap()["enabled"], true);
        assert_eq!(read_json(&directory.join(META_FILE), "metadata").unwrap()["futureMetadata"], true);
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
        assert_eq!(read_json(&settings.join(ANYBLOB_SETTINGS_FILE), "anyblob").unwrap()["verifier"], "opaque-hash");
        assert_eq!(read_formula(&root).unwrap().unwrap()["version"], 2);
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
