//! Persistent discovery catalogs. Object reads and authorization bypass this cache.
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

#[derive(Default, Serialize, Deserialize)]
struct File {
    version: u32,
    entries: BTreeMap<String, Value>,
}
#[derive(Default)]
struct Entry {
    completed: Option<Instant>,
    result: Option<Result<Value, String>>,
}
static FLIGHTS: OnceLock<Mutex<BTreeMap<String, Arc<Mutex<Entry>>>>> = OnceLock::new();
static DISK: Mutex<()> = Mutex::new(());
fn path() -> PathBuf {
    let data = crate::app_data_directory();
    data.parent()
        .unwrap_or(&data)
        .join("BlobUI/metadata-cache.json")
}
fn read(path: &Path) -> File {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<File>(&bytes).ok())
        .filter(|file| file.version == 1)
        .unwrap_or_else(|| File {
            version: 1,
            ..File::default()
        })
}
pub fn get<T: Serialize + DeserializeOwned>(
    catalog: &str,
    args: Value,
    force: bool,
    fetch: impl FnOnce(&crate::kanban_api::Client) -> Result<T, String>,
) -> Result<T, String> {
    let client = crate::kanban_api::Client::connect()?;
    let scope = client.metadata_scope();
    get_at(
        &path(),
        &scope,
        catalog,
        args,
        force,
        || fetch(&client),
        || {
            if crate::kanban_api::Client::connect()?.metadata_scope() == scope {
                Ok(())
            } else {
                Err("The saved Anytype connection changed. Refresh metadata again.".into())
            }
        },
    )
}
fn get_at<T: Serialize + DeserializeOwned>(
    path: &Path,
    scope: &str,
    catalog: &str,
    args: Value,
    force: bool,
    fetch: impl FnOnce() -> Result<T, String>,
    current: impl Fn() -> Result<(), String>,
) -> Result<T, String> {
    let started = Instant::now();
    let key = serde_json::to_string(&(scope, catalog, args)).map_err(|e| e.to_string())?;
    let flight = FLIGHTS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "Metadata cache lock failed.")?
        .entry(format!("{}:{key}", path.display()))
        .or_default()
        .clone();
    let mut entry = flight.lock().map_err(|_| "Metadata request lock failed.")?;
    // Calls arriving during one fetch share its result, including failures.
    if entry.completed.is_some_and(|time| time >= started) {
        current()?;
        return serde_json::from_value(entry.result.clone().unwrap()?).map_err(|e| e.to_string());
    }
    let result = (|| {
        if !force {
            let _disk = DISK.lock().map_err(|_| "Metadata file lock failed.")?;
            if let Some(value) = read(path).entries.get(&key) {
                if let Ok(value) = serde_json::from_value::<T>(value.clone()) {
                    return serde_json::to_value(value).map_err(|e| e.to_string());
                }
            }
        }
        let value = serde_json::to_value(fetch()?).map_err(|e| e.to_string())?;
        current()?;
        // A partial discovery must not replace the last complete catalog.
        if value
            .get("errors")
            .and_then(Value::as_array)
            .is_none_or(|errors| errors.is_empty())
        {
            let _disk = DISK.lock().map_err(|_| "Metadata file lock failed.")?;
            let mut file = read(path);
            file.entries.insert(key, value.clone());
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let bytes = serde_json::to_vec(&file).map_err(|e| e.to_string())?;
            // Windows can briefly deny replacement while a scanner has the old file open.
            let mut saved = crate::storage::write_atomic(path, &bytes);
            for _ in 0..3 {
                if saved.is_ok() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
                saved = crate::storage::write_atomic(path, &bytes);
            }
            saved?;
        }
        Ok(value)
    })();
    entry.completed = Some(Instant::now());
    entry.result = Some(result.clone());
    current()?;
    serde_json::from_value(result?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Barrier,
    };
    fn fixture(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.test-run/metadata-cache")
            .join(format!(
                "{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap().join("cache.json")
    }
    fn load(
        path: &Path,
        scope: &str,
        args: Value,
        force: bool,
        fetch: impl FnOnce() -> Result<Value, String>,
    ) -> Result<Value, String> {
        get_at(path, scope, "types", args, force, fetch, || Ok(()))
    }
    #[test]
    fn restart_hits_arguments_connections_and_forced_refresh() {
        let path = fixture("restart");
        assert_eq!(
            load(&path, "connection-a", json!(["space-a"]), false, || Ok(
                json!([1])
            ))
            .unwrap(),
            json!([1])
        );
        // Drop process-local request bookkeeping to model a fresh app instance.
        FLIGHTS
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .retain(|key, _| !key.starts_with(&path.display().to_string()));
        assert_eq!(
            load(&path, "connection-a", json!(["space-a"]), false, || panic!(
                "Restart fetched metadata"
            ))
            .unwrap(),
            json!([1])
        );
        assert_eq!(
            load(&path, "connection-a", json!(["space-b"]), false, || Ok(
                json!([2])
            ))
            .unwrap(),
            json!([2])
        );
        assert_eq!(
            load(&path, "connection-b", json!(["space-a"]), false, || Ok(
                json!([3])
            ))
            .unwrap(),
            json!([3])
        );
        assert_eq!(
            load(&path, "connection-a", json!(["space-a"]), true, || Ok(
                json!([4])
            ))
            .unwrap(),
            json!([4])
        );
        assert!(
            load(&path, "connection-a", json!(["space-a"]), true, || Err(
                "offline".into()
            ))
            .is_err()
        );
        assert_eq!(
            load(&path, "connection-a", json!(["space-a"]), false, || panic!(
                "Failed refresh destroyed cache"
            ))
            .unwrap(),
            json!([4])
        );
    }
    #[test]
    fn malformed_incompatible_and_wrong_shape_are_misses() {
        let path = fixture("corrupt");
        for contents in [
            "broken",
            r#"{"version":99,"entries":{}}"#,
            r#"{"version":1,"entries":{"[\"scope\",\"types\",null]":"wrong"}}"#,
        ] {
            std::fs::write(&path, contents).unwrap();
            let value: Vec<String> = get_at(
                &path,
                "scope",
                "types",
                Value::Null,
                false,
                || Ok(vec!["fresh".into()]),
                || Ok(()),
            )
            .unwrap();
            assert_eq!(value, vec!["fresh"]);
        }
    }
    #[test]
    fn outdated_and_partial_results_do_not_replace_success() {
        let path = fixture("outdated");
        load(&path, "scope", Value::Null, false, || {
            Ok(json!({"types":[1],"errors":[]}))
        })
        .unwrap();
        load(&path, "scope", Value::Null, true, || {
            Ok(json!({"types":[],"errors":[{"error":"offline"}]}))
        })
        .unwrap();
        let result: Result<Value, String> = get_at(
            &path,
            "scope",
            "types",
            Value::Null,
            true,
            || Ok(json!({"types":[2]})),
            || Err("connection changed".into()),
        );
        assert!(result.is_err());
        assert_eq!(
            load(&path, "scope", Value::Null, false, || panic!(
                "Previous success lost"
            ))
            .unwrap()["types"],
            json!([1])
        );
    }
    #[test]
    fn concurrent_forced_requests_share_success_and_failure() {
        for fail in [false, true] {
            let path = fixture("concurrent");
            let calls = AtomicUsize::new(0);
            let barrier = Barrier::new(8);
            std::thread::scope(|threads| {
                for _ in 0..8 {
                    let (path, calls, barrier) = (&path, &calls, &barrier);
                    threads.spawn(move || {
                        barrier.wait();
                        let result = load(path, "scope", Value::Null, true, || {
                            calls.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if fail {
                                Err("offline".into())
                            } else {
                                Ok(json!([1]))
                            }
                        });
                        assert_eq!(result.is_err(), fail);
                    });
                }
            });
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }
}
