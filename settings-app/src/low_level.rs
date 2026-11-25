use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "low-lvl-settings.json";
static FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn path(install: &Path) -> Result<PathBuf, String> {
    Ok(crate::settings_root_for_install(install)?.join("Settings").join(FILE_NAME))
}

fn defaults() -> Value {
    json!({
        "version": 1,
        "sidebarWidth": 280.0,
        "sidebarCollapsed": false,
        "fullscreen": false,
        "uiScale": 1.0,
        "windowWidth": 960.0,
        "windowHeight": 740.0
    })
}

fn clamp_number(value: Option<&Value>, fallback: f64, min: f64, max: f64) -> f64 {
    value.and_then(Value::as_f64).unwrap_or(fallback).clamp(min, max)
}

fn sanitize(value: &Value) -> Value {
    let defaults = defaults();
    json!({
        "version": 1,
        "sidebarWidth": clamp_number(value.get("sidebarWidth"), 280.0, 160.0, 520.0),
        "sidebarCollapsed": value.get("sidebarCollapsed").and_then(Value::as_bool).unwrap_or(false),
        "fullscreen": value.get("fullscreen").and_then(Value::as_bool).unwrap_or(false),
        "uiScale": clamp_number(value.get("uiScale"), 1.0, 0.8, 1.5),
        "windowWidth": clamp_number(value.get("windowWidth"), 960.0, 800.0, 3840.0),
        "windowHeight": clamp_number(value.get("windowHeight"), 740.0, 650.0, 2160.0),
    }).as_object().map(|object| {
        let mut object = object.clone();
        if !object.contains_key("version") { object.insert("version".into(), defaults["version"].clone()); }
        Value::Object(object)
    }).unwrap_or_default()
}

fn write(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path.parent().ok_or("The low-level settings path is invalid.")?;
    fs::create_dir_all(parent).map_err(|_| "The Settings folder could not be created.".to_string())?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "The low-level settings could not be encoded.".to_string())?;
    {
        let mut file = fs::File::create(&temporary).map_err(|_| "The low-level settings file could not be written.".to_string())?;
        file.write_all(&bytes).and_then(|_| file.write_all(b"\n")).map_err(|_| "The low-level settings file could not be written.".to_string())?;
        file.sync_all().ok();
    }
    fs::rename(&temporary, path).map_err(|_| "The low-level settings file could not be replaced.".to_string())
}

pub fn ensure(install: &Path) -> Result<Value, String> {
    let _guard = FILE_LOCK.lock().map_err(|_| "The low-level settings lock is unavailable.")?;
    let path = path(install)?;
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            let clean = sanitize(&value);
            if clean != value { write(&path, &clean)?; }
            return Ok(clean);
        }
    }
    let clean = defaults();
    write(&path, &clean)?;
    Ok(clean)
}

pub fn update(install: &Path, patch: &Value) -> Result<Value, String> {
    let _guard = FILE_LOCK.lock().map_err(|_| "The low-level settings lock is unavailable.")?;
    let path = path(install)?;
    let mut current = fs::read_to_string(&path).ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|value| sanitize(&value))
        .unwrap_or_else(defaults);
    let object = current.as_object_mut().ok_or("The low-level settings file is malformed.")?;
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            if matches!(key.as_str(), "sidebarWidth" | "sidebarCollapsed" | "fullscreen" | "uiScale" | "windowWidth" | "windowHeight") {
                object.insert(key.clone(), value.clone());
            }
        }
    }
    let clean = sanitize(&current);
    write(&path, &clean)?;
    Ok(clean)
}

