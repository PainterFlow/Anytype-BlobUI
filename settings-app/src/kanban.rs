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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub space_id: String,
    pub object_id: String,
    pub block_id: String,
    pub view_id: String,
}
pub fn validate_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err("A saved view has an invalid identity. Reload the view list.".into());
    }
    Ok(())
}
impl Target {
    pub fn validate(&self) -> Result<(), String> {
        for id in [
            &self.space_id,
            &self.object_id,
            &self.block_id,
            &self.view_id,
        ] {
            validate_id(id)?;
        }
        Ok(())
    }
}

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
            || !matches!(self.display.as_str(), "compact" | "balanced" | "spacious")
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
    pub properties: Vec<Property>,
    pub unsupported: Option<String>,
}
#[derive(Serialize)]
pub struct Catalog {
    pub views: Vec<View>,
    pub warning: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CategoryResult {
    pub target: Target,
    pub category_id: String,
    pub value: Option<f64>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub display: String,
    pub calculated_at: String,
    pub results: Vec<CategoryResult>,
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
fn path() -> PathBuf {
    workspace().join(".blobui-kanban-formulas.json")
}
fn load_at(path: &Path, legacy: &Path) -> Result<Configuration, String> {
    crate::storage::reject_reparse_path(path)?;
    match fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.len() > 1024 * 1024 {
                return Err("The Kanban formula file is unsupported or too large. It has been left unchanged.".into());
            }
            let configuration: Configuration = serde_json::from_slice(
                &fs::read(path).map_err(|_| "Could not read the Kanban formula file.")?,
            )
            .map_err(|_| "The Kanban formula file is malformed. It has been left unchanged.")?;
            configuration.validate()?;
            Ok(configuration)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut configuration = Configuration::default();
            crate::storage::reject_reparse_path(legacy)?;
            if fs::metadata(legacy).is_ok_and(|m| m.is_file() && m.len() <= 65536) {
                if let Ok(bytes) = fs::read(legacy) {
                    if let Ok(old) = serde_json::from_slice::<Value>(&bytes) {
                        if old["version"] == 1
                            && matches!(
                                old["display"].as_str(),
                                Some("compact" | "balanced" | "spacious")
                            )
                        {
                            configuration.display = old["display"].as_str().unwrap().into();
                        }
                    }
                }
            }
            Ok(configuration)
        }
        Err(_) => Err(
            "Could not read the Kanban formula file. Check workspace folder permissions.".into(),
        ),
    }
}
fn load() -> Result<Configuration, String> {
    load_at(&path(), &workspace().join(".blobui-formula-prototype.json"))
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
        version: 1,
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
    let client = if configuration.definitions.is_empty() {
        None
    } else {
        Some(Client::connect()?)
    };
    publish_at(configuration, &install, &path(), |definition| {
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
                value: Some(2.0),
            }])
        })
        .unwrap();
        assert_eq!(published.categories, 1);
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
                value: Some(9.0)
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
        let old=br#"{"version":1,"display":"compact","formulas":{"work/tasks":{"expression":"sum(estimate)"}}}"#;
        fs::write(&legacy, old).unwrap();
        let configuration = load_at(&current, &legacy).unwrap();
        assert_eq!(configuration.display, "compact");
        assert!(configuration.definitions.is_empty());
        assert_eq!(fs::read(&legacy).unwrap(), old);
        fs::write(&current, b"broken").unwrap();
        assert!(load_at(&current, &legacy).is_err());
        assert_eq!(fs::read(&current).unwrap(), b"broken");
    }
}
