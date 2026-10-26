use crate::{
    calculator::{Calculator, Token},
    kanban_api::Client,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub use crate::kanban_snapshot::{validate_id, CategoryResult, Snapshot, Target};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub target: Target,
    pub tokens: Vec<Token>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub version: u32,
    pub display: String,
    pub definitions: Vec<Definition>,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            version: 2,
            display: "balanced".into(),
            definitions: Vec::new(),
        }
    }
}
impl Configuration {
    fn validate(&self) -> Result<(), String> {
        if self.version != 2
            || !matches!(self.display.as_str(), "balanced" | "spacious")
            || self.definitions.len() > 100
        {
            return Err("This Kanban formula configuration is unsupported or too large.".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for definition in &self.definitions {
            definition.target.validate()?;
            if !seen.insert(&definition.target) {
                return Err("A view has more than one saved calculation.".into());
            }
            Calculator::parse(&definition.tokens)?;
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
pub struct Property {
    pub key: String,
    pub name: String,
    pub format: String,
}
#[derive(Serialize)]
pub struct View {
    pub target: Target,
    pub name: String,
    pub space: String,
    pub source: String,
    pub kind: String,
    #[serde(rename = "sourceSignature", skip_serializing_if = "Option::is_none")]
    pub source_signature: Option<String>,
    pub properties: Vec<Property>,
    pub unsupported: Option<String>,
}
#[derive(Serialize)]
pub struct Catalog {
    pub views: Vec<View>,
    pub warning: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Loaded {
    configuration: Configuration,
    calculated_at: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Published {
    calculated_at: String,
    categories: usize,
}
#[derive(Serialize)]
pub struct Evaluation {
    categories: usize,
}

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace parent")
}
fn path_for_install(install: &Path) -> PathBuf {
    install.join("blob/.blobui-kanban-formulas.json")
}
pub(crate) fn load_at(path: &Path, legacy: &Path, prototype: &Path) -> Result<Configuration, String> {
    crate::storage::reject_reparse_path(path)?;
    match fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.len() > 1024 * 1024 {
                return Err("The Kanban formula file is unsupported or too large. It has been left unchanged.".into());
            }
            let bytes = fs::read(path).map_err(|_| "Could not read the Kanban formula file.")?;
            let mut configuration = match serde_json::from_slice::<Configuration>(&bytes) {
                Ok(configuration) => configuration,
                Err(_) => {
                    let configuration = migrate_legacy_configuration(&bytes)?;
                    persist_migrated_configuration(path, &configuration)?;
                    return Ok(configuration);
                }
            };
            if configuration.display == "compact" {
                configuration.display = "balanced".into();
            }
            configuration.validate()?;
            Ok(configuration)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::storage::reject_reparse_path(legacy)?;
            match fs::symlink_metadata(legacy) {
                Ok(metadata) if metadata.is_file() && metadata.len() <= 1024 * 1024 => {
                    let bytes = fs::read(legacy).map_err(|_| "Could not read the legacy Kanban formula file. It has been left unchanged.".to_string())?;
                    let configuration = migrate_legacy_configuration(&bytes)?;
                    persist_migrated_configuration(path, &configuration)?;
                    return Ok(configuration);
                }
                Ok(_) => return Err("The legacy Kanban formula file is unsupported or too large. It has been left unchanged.".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Could not inspect the legacy Kanban formula file. It has been left unchanged.".into()),
            }
            let mut configuration = Configuration::default();
            crate::storage::reject_reparse_path(prototype)?;
            if let Ok(metadata) = fs::symlink_metadata(prototype) {
                if metadata.is_file() && metadata.len() <= 65536 {
                    if let Ok(bytes) = fs::read(prototype) {
                        if let Ok(old) = serde_json::from_slice::<Value>(&bytes) {
                            if old["version"] == 1 {
                                match old["display"].as_str() {
                                    Some("compact" | "balanced") => configuration.display = "balanced".into(),
                                    Some("spacious") => configuration.display = "spacious".into(),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            if configuration.display != "balanced" {
                persist_migrated_configuration(path, &configuration)?;
            }
            Ok(configuration)
        }
        Err(_) => Err(
            "Could not read the Kanban formula file. Check workspace folder permissions.".into(),
        ),
    }
}
fn load() -> Result<Configuration, String> {
    let install = crate::resolve_install_directory(&crate::app_data_directory())?;
    load_at(
        &path_for_install(&install),
        &workspace().join(".blobui-kanban-formulas.json"),
        &workspace().join(".blobui-formula-prototype.json"),
    )
}

pub(crate) fn prepare_backup(install: &Path) -> Result<(), String> {
    let _ = load_at(
        &path_for_install(install),
        &workspace().join(".blobui-kanban-formulas.json"),
        &workspace().join(".blobui-formula-prototype.json"),
    )?;
    Ok(())
}

fn migrate_legacy_configuration(bytes: &[u8]) -> Result<Configuration, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "The legacy Kanban formula file is malformed. It has been left unchanged.".to_string())?;
    if value.get("version").and_then(Value::as_u64) == Some(1) {
        let mut configuration = Configuration::default();
        match value.get("display").and_then(Value::as_str) {
            Some("compact" | "balanced") => configuration.display = "balanced".into(),
            Some("spacious") => configuration.display = "spacious".into(),
            _ => {}
        }
        return Ok(configuration);
    }
    let mut configuration: Configuration = serde_json::from_value(value)
        .map_err(|_| "The legacy Kanban formula file is unsupported. It has been left unchanged.".to_string())?;
    if configuration.display == "compact" {
        configuration.display = "balanced".into();
    }
    configuration.validate()?;
    Ok(configuration)
}

fn persist_migrated_configuration(path: &Path, configuration: &Configuration) -> Result<(), String> {
    use crate::storage::transaction::{commit_update, FileChange};
    let bytes = serde_json::to_vec_pretty(configuration)
        .map_err(|_| "Could not prepare the migrated Kanban formula file.".to_string())?;
    let metadata_path = path.parent().ok_or_else(|| "The formula file has no parent folder.".to_string())?.join("blob.meta.json");
    commit_update(vec![
        FileChange { path: path.into(), contents: Some(bytes) },
        FileChange { path: metadata_path, contents: Some(crate::backup::metadata_bytes()?) },
    ])
}

pub(crate) fn validate_saved_configuration(path: &Path) -> Result<(), String> {
    crate::storage::reject_reparse_path(path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "The saved Kanban formula file is missing.".to_string())?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err("The saved Kanban formula file is unsupported or too large.".into());
    }
    let bytes = fs::read(path).map_err(|_| "Could not read the saved Kanban formula file.".to_string())?;
    migrate_legacy_configuration(&bytes).map(|_| ())
}

pub(crate) fn initialize_imported_configuration(path: &Path) -> Result<(), String> {
    crate::storage::reject_reparse_path(path)?;
    match fs::symlink_metadata(path) {
        Ok(_) => validate_saved_configuration(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            persist_migrated_configuration(path, &Configuration::default())
        }
        Err(error) => Err(format!("Could not inspect the imported Kanban formula file: {error}")),
    }
}

fn publish_at(
    configuration: Configuration,
    install: &Path,
    config_path: &Path,
    calculate: impl Fn(&Definition) -> Result<Vec<CategoryResult>, String>,
) -> Result<Published, String> {
    configuration.validate()?;
    let (_, data_path, css_path, _) = crate::save_paths(install)?;
    let runtime = crate::resolve_runtime_directory_for_install(install)?;
    let mut data = crate::read_json_file(&data_path)?;
    if !data.is_object() {
        return Err("BlobUI data.json must contain an object.".into());
    }
    let mut results = Vec::new();
    for definition in &configuration.definitions {
        results.extend(calculate(definition)?);
        if results.len() > 4096 {
            return Err("Too many category results to publish safely.".into());
        }
    }
    let calculated_at = chrono::Utc::now().to_rfc3339();
    let categories = results.len();
    data["kanbanFormulaResults"] = json!(Snapshot {
        version: 2,
        display: configuration.display.clone(),
        calculated_at: calculated_at.clone(),
        results
    });
    let css = crate::css::compile(&runtime, install, &data)?;
    let config_bytes = serde_json::to_vec_pretty(&configuration)
        .map_err(|_| "Could not prepare Kanban formulas.")?;
    // The snapshot, stylesheet, and saved baseline change together. A failed
    // API read or replacement never publishes half of a calculation set.
    use crate::storage::transaction::{commit_update, FileChange};
    commit_update(vec![
        FileChange {
            path: config_path.into(),
            contents: Some(config_bytes),
        },
        FileChange {
            path: install.join("blob/blob.meta.json"),
            contents: Some(crate::backup::metadata_bytes()?),
        },
        FileChange {
            path: data_path,
            contents: Some(crate::serialize_data(&data)?),
        },
        FileChange {
            path: css_path,
            contents: Some(css.into_bytes()),
        },
    ])?;
    Ok(Published {
        calculated_at,
        categories,
    })
}
fn publish(configuration: Configuration) -> Result<Published, String> {
    let install = crate::resolve_install_directory(&crate::app_data_directory())?;
    let config_path = path_for_install(&install);
    let client = if configuration.definitions.is_empty() {
        None
    } else {
        Some(Client::connect()?)
    };
    publish_at(configuration, &install, &config_path, |definition| {
        client
            .as_ref()
            .ok_or("The Anytype connection is unavailable.")?
            .calculate(&definition.target, &Calculator::parse(&definition.tokens)?)
    })
}

#[tauri::command]
pub fn load_kanban_formulas() -> Result<Loaded, String> {
    let configuration = load()?;
    let calculated_at = crate::resolve_install_directory(&crate::app_data_directory())
        .ok()
        .and_then(|install| crate::read_json_file(&install.join("blob/data.json")).ok())
        .and_then(|data| {
            data.pointer("/kanbanFormulaResults/calculatedAt")
                .and_then(Value::as_str)
                .map(String::from)
        });
    Ok(Loaded {
        configuration,
        calculated_at,
    })
}
#[tauri::command]
pub async fn discover_kanban_views() -> Result<Catalog, String> {
    tauri::async_runtime::spawn_blocking(|| Client::connect()?.discover())
        .await
        .map_err(|_| "The Kanban discovery task stopped. Try loading views again.".to_string())?
}
#[tauri::command]
pub async fn evaluate_kanban_formula(
    target: Target,
    tokens: Vec<Token>,
) -> Result<Evaluation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let calculator = Calculator::parse(&tokens)?;
        let results = Client::connect()?.calculate(&target, &calculator)?;
        Ok(Evaluation {
            categories: results.len(),
        })
    })
    .await
    .map_err(|_| "The calculator task stopped. Try again.".to_string())?
}
#[tauri::command]
pub async fn save_kanban_formulas(configuration: Configuration) -> Result<Published, String> {
    let guard = crate::acquire_save_guard("Wait for the current settings save to finish.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        publish(configuration)
    })
    .await
    .map_err(|_| "The formula save stopped. Your pending edits are kept.".to_string())?
}
#[tauri::command]
pub async fn refresh_kanban_results() -> Result<Published, String> {
    let guard = crate::acquire_save_guard("Wait for the current settings save to finish.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        publish(load()?)
    })
    .await
    .map_err(|_| "The formula refresh stopped. Previous results are kept.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publication_commits_all_files_and_rolls_back_after_partial_failure() {
        let root = workspace().join(".test-run").join(format!(
            "kanban-publish-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("blob")).unwrap();
        let data = root.join("blob/data.json");
        let css = root.join("custom.css");
        let config = root.join("configuration.json");
        let mut initial = json!({"installDirectory":root});
        crate::apply_settings(&mut initial, &root, &crate::SettingsDraft::default());
        fs::write(&data, serde_json::to_vec(&initial).unwrap()).unwrap();
        fs::write(&css, b"old css").unwrap();
        let target = Target {
            space_id: "space".into(),
            object_id: "object-a".into(),
            block_id: "board".into(),
            view_id: "view-a".into(),
        };
        let configuration = Configuration {
            version: 2,
            display: "balanced".into(),
            definitions: vec![Definition {
                target: target.clone(),
                tokens: vec![Token::Count],
            }],
        };
        let published = publish_at(configuration.clone(), &root, &config, |_| {
            Ok(vec![CategoryResult {
                target: target.clone(),
                category_id: "empty".into(),
                value: Some(crate::calculator::CalculationValue::Boolean(true)),
            }])
        })
        .unwrap();
        assert_eq!(published.categories, 1);
        let saved: Value = serde_json::from_slice(&fs::read(&data).unwrap()).unwrap();
        assert_eq!(saved["kanbanFormulaResults"]["version"], json!(2));
        assert_eq!(saved["kanbanFormulaResults"]["results"][0]["value"], json!(true));
        let before = [
            fs::read(&config).unwrap(),
            fs::read(&data).unwrap(),
            fs::read(&css).unwrap(),
        ];
        assert!(publish_at(configuration.clone(), &root, &config, |_| Err(
            "API unavailable".into()
        ))
        .is_err());
        crate::storage::FAIL_AFTER_REPLACE.with(|fail| *fail.borrow_mut() = Some(css.clone()));
        assert!(publish_at(configuration, &root, &config, |_| Ok(vec![
            CategoryResult {
                target: target.clone(),
                category_id: "empty".into(),
                value: Some((9.0).into())
            }
        ]))
        .is_err());
        assert_eq!(fs::read(config).unwrap(), before[0]);
        assert_eq!(fs::read(data).unwrap(), before[1]);
        assert_eq!(fs::read(css).unwrap(), before[2]);
    }
    #[test]
    fn migration_preserves_old_file_and_rejects_malformed_live_config() {
        let root = workspace().join(".test-run").join(format!(
            "kanban-config-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let legacy = root.join("legacy.json");
        let current = root.join("config.json");
        let prototype = root.join("prototype.json");
        let old=br#"{"version":1,"display":"compact","formulas":{"work/tasks":{"expression":"sum(estimate)"}}}"#;
        fs::write(&legacy, old).unwrap();
        let configuration = load_at(&current, &legacy, &prototype).unwrap();
        assert_eq!(configuration.display, "balanced");
        assert!(configuration.definitions.is_empty());
        assert_eq!(fs::read(&legacy).unwrap(), old);
        fs::write(&current, br#"{"version":2,"display":"compact","definitions":[]}"#).unwrap();
        let configuration = load_at(&current, &legacy, &prototype).unwrap();
        assert_eq!(configuration.display, "balanced");
        fs::write(&current, b"broken").unwrap();
        assert!(load_at(&current, &legacy, &prototype).is_err());
        assert_eq!(fs::read(&current).unwrap(), b"broken");
    }

    #[test]
    fn legacy_formula_definitions_migrate_atomically_into_the_configured_blob() {
        let root = workspace().join(".test-run").join(format!(
            "kanban-legacy-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let blob = root.join("blob");
        fs::create_dir_all(&blob).unwrap();
        let legacy = root.join(".blobui-kanban-formulas.json");
        let current = blob.join(".blobui-kanban-formulas.json");
        let prototype = root.join(".blobui-formula-prototype.json");
        let saved = br#"{"version":2,"display":"spacious","definitions":[]}"#;
        fs::write(&legacy, saved).unwrap();

        let configuration = load_at(&current, &legacy, &prototype).unwrap();
        assert_eq!(configuration.display, "spacious");
        assert_eq!(fs::read(&current).unwrap(), serde_json::to_vec_pretty(&configuration).unwrap());
        assert_eq!(fs::read(&legacy).unwrap(), saved);
        let metadata: Value = serde_json::from_slice(&fs::read(blob.join("blob.meta.json")).unwrap()).unwrap();
        assert_eq!(metadata["blobuiVersion"], env!("BLOBUI_BUILD_VERSION"));
        fs::remove_dir_all(root).unwrap();
    }
}
