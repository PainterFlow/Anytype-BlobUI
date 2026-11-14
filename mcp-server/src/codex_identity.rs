//! Resolve a Codex chat to its stable local project ID using read-only host metadata.
use crate::{codex_projects, ProjectInfo, RemoteMembership};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Membership {
    Project(String),
    Projectless,
}

const HOST_ID_ERROR: &str = "Could not read or create the Anyblob host ID.";

pub fn resolve_remote_at(
    home: &Path,
    thread_id: &str,
    host_id: &str,
) -> Result<RemoteMembership, String> {
    let catalog = codex_projects::read_at(home)?;
    let membership = resolve_at(home, thread_id, &catalog)?;
    let project_id = match membership {
        Membership::Project(id) => Some(id),
        Membership::Projectless => None,
    };
    Ok(RemoteMembership {
        host_id: host_id.into(),
        project_id,
    })
}

pub fn resolve_remote(thread_id: &str) -> Result<RemoteMembership, String> {
    let home = codex_projects::codex_home()?;
    let host_id = host_id_at(&home)?;
    resolve_remote_at(&home, thread_id, &host_id)
}

pub fn resolve_at(
    home: &Path,
    thread_id: &str,
    catalog: &[ProjectInfo],
) -> Result<Membership, String> {
    if thread_id.is_empty()
        || thread_id.len() > 128
        || !thread_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("Codex supplied invalid chat identity metadata.".into());
    }
    let state: Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".codex-global-state.json"))
            .map_err(|_| "Could not read Codex chat membership metadata.")?,
    )
    .map_err(|_| "Codex chat membership metadata is not valid JSON.")?;

    if state["thread-project-membership-host-ids"]
        .get(thread_id)
        .is_some_and(|host| host.as_str() != Some("local"))
    {
        return Err("Anyblob project permissions support local Codex chats only.".into());
    }

    let is_projectless = || {
        state["projectless-thread-ids"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(thread_id)))
    };
    if let Some(assignment) = state["thread-project-assignments"].get(thread_id) {
        if is_projectless() {
            return Err("Codex chat membership conflicts between local records.".into());
        }
        if assignment["projectKind"] != "local" {
            return Err("Anyblob project permissions support local Codex chats only.".into());
        }
        let id = assignment["projectId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("Codex returned invalid chat project membership.")?;
        return canonical_project(&state, id, catalog);
    }
    if is_projectless() {
        return Ok(Membership::Projectless);
    }

    let mut databases = Vec::new();
    for entry in std::fs::read_dir(home).map_err(|_| "Could not inspect Codex chat membership.")? {
        let entry = entry.map_err(|_| "Could not inspect Codex chat membership.")?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(version) = name
            .strip_prefix("state_")
            .and_then(|value| value.strip_suffix(".sqlite"))
            .and_then(|value| value.parse::<u32>().ok())
        {
            databases.push((version, entry.path()));
        }
    }
    databases.sort_by_key(|(version, _)| *version);
    let path = &databases
        .last()
        .ok_or("Codex has not supplied resolvable chat membership.")?
        .1;
    match database_project(path, thread_id)? {
        Some(Some(id)) => canonical_project(&state, &id, catalog),
        Some(None) => Ok(Membership::Projectless),
        None => Err("Codex chat identity was not found in local membership metadata.".into()),
    }
}

fn canonical_project(
    state: &Value,
    id: &str,
    catalog: &[ProjectInfo],
) -> Result<Membership, String> {
    let mut legacy = std::collections::BTreeSet::new();
    if let Some(hosts) = state["app-server-project-id-by-legacy-project-id-by-host"].as_object() {
        for (host, mapping) in hosts {
            if !host.starts_with("local:") {
                continue;
            }
            if let Some(mapping) = mapping.as_object() {
                for (old, new) in mapping {
                    if new.as_str() == Some(id) {
                        legacy.insert(old.clone());
                    }
                }
            }
        }
    }
    if legacy.len() > 1 {
        return Err("Codex project identity is ambiguous. Refresh Projects and retry.".into());
    }
    let id = legacy.into_iter().next().unwrap_or_else(|| id.into());
    if !catalog.iter().any(|project| project.id == id) {
        return Err("Codex chat project is not present in the current project catalog.".into());
    }
    Ok(Membership::Project(id))
}

pub fn host_id_at(home: &Path) -> Result<String, String> {
    static HOST_ID_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = HOST_ID_LOCK.lock().map_err(|_| HOST_ID_ERROR.to_string())?;
    let path = home.join("anyblob-host-id");
    match read_host_id(&path) {
        Ok(id) => return Ok(id),
        Err(HostIdReadError::NotFound) => {}
        Err(HostIdReadError::Invalid) => return Err(HOST_ID_ERROR.into()),
    }

    let mut bytes = [0u8; 16];
    random_bytes(&mut bytes)?;
    let id = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    bytes.fill(0);

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            use std::io::Write;
            let result = file.write_all(id.as_bytes()).and_then(|_| file.sync_all());
            if result.is_err() {
                drop(file);
                let _ = std::fs::remove_file(&path);
                Err(HOST_ID_ERROR.into())
            } else {
                Ok(id)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            for _ in 0..20 {
                if let Ok(id) = read_host_id(&path) {
                    return Ok(id);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(HOST_ID_ERROR.into())
        }
        Err(_) => Err(HOST_ID_ERROR.into()),
    }
}

enum HostIdReadError {
    NotFound,
    Invalid,
}

fn read_host_id(path: &Path) -> Result<String, HostIdReadError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(HostIdReadError::NotFound)
        }
        Err(_) => return Err(HostIdReadError::Invalid),
    };
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked || !metadata.is_file() {
        return Err(HostIdReadError::Invalid);
    }
    let id = std::fs::read_to_string(path).map_err(|_| HostIdReadError::Invalid)?;
    let id = id.trim();
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(HostIdReadError::Invalid);
    }
    Ok(id.into())
}

#[cfg(windows)]
fn random_bytes(bytes: &mut [u8]) -> Result<(), String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    let ok = unsafe { SystemFunction036(bytes.as_mut_ptr(), bytes.len() as u32) };
    if ok == 0 {
        Err("Windows could not generate the Anyblob host ID.".into())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn random_bytes(bytes: &mut [u8]) -> Result<(), String> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(bytes))
        .map_err(|_| "The operating system could not generate the Anyblob host ID.".into())
}

#[cfg(not(any(windows, unix)))]
fn random_bytes(_: &mut [u8]) -> Result<(), String> {
    Err("This platform cannot generate the Anyblob host ID.".into())
}

#[cfg(windows)]
mod sqlite {
    use super::*;
    use std::{
        ffi::{c_char, c_int, c_void, CStr, CString},
        ptr,
    };
    #[link(name = "winsqlite3", kind = "raw-dylib")]
    unsafe extern "C" {
        fn sqlite3_open_v2(
            name: *const c_char,
            db: *mut *mut c_void,
            flags: c_int,
            vfs: *const c_char,
        ) -> c_int;
        fn sqlite3_close(db: *mut c_void) -> c_int;
        fn sqlite3_busy_timeout(db: *mut c_void, ms: c_int) -> c_int;
        fn sqlite3_prepare_v2(
            db: *mut c_void,
            sql: *const c_char,
            length: c_int,
            stmt: *mut *mut c_void,
            tail: *mut *const c_char,
        ) -> c_int;
        fn sqlite3_bind_text(
            stmt: *mut c_void,
            column: c_int,
            text: *const c_char,
            length: c_int,
            destructor: Option<unsafe extern "C" fn(*mut c_void)>,
        ) -> c_int;
        fn sqlite3_step(stmt: *mut c_void) -> c_int;
        fn sqlite3_column_type(stmt: *mut c_void, column: c_int) -> c_int;
        fn sqlite3_column_text(stmt: *mut c_void, column: c_int) -> *const u8;
        fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
    }
    pub fn project(path: &Path, thread_id: &str) -> Result<Option<Option<String>>, String> {
        unsafe { query_project(path, thread_id) }
    }
    unsafe fn query_project(
        path: &Path,
        thread_id: &str,
    ) -> Result<Option<Option<String>>, String> {
        query_project_impl(path, thread_id)
    }
    include!("codex_identity_sqlite.in.rs");
}

#[cfg(unix)]
mod sqlite {
    use super::*;
    use std::{
        ffi::{c_char, c_int, c_void, CStr, CString},
        ptr,
    };
    // Keep Cargo's bundled SQLite native library dependency attached to this crate.
    const _: i32 = libsqlite3_sys::SQLITE_VERSION_NUMBER;
    #[link(name = "sqlite3", kind = "static")]
    unsafe extern "C" {
        fn sqlite3_open_v2(
            name: *const c_char,
            db: *mut *mut c_void,
            flags: c_int,
            vfs: *const c_char,
        ) -> c_int;
        fn sqlite3_close(db: *mut c_void) -> c_int;
        fn sqlite3_busy_timeout(db: *mut c_void, ms: c_int) -> c_int;
        fn sqlite3_prepare_v2(
            db: *mut c_void,
            sql: *const c_char,
            length: c_int,
            stmt: *mut *mut c_void,
            tail: *mut *const c_char,
        ) -> c_int;
        fn sqlite3_bind_text(
            stmt: *mut c_void,
            column: c_int,
            text: *const c_char,
            length: c_int,
            destructor: Option<unsafe extern "C" fn(*mut c_void)>,
        ) -> c_int;
        fn sqlite3_step(stmt: *mut c_void) -> c_int;
        fn sqlite3_column_type(stmt: *mut c_void, column: c_int) -> c_int;
        fn sqlite3_column_text(stmt: *mut c_void, column: c_int) -> *const u8;
        fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
    }
    pub fn project(path: &Path, thread_id: &str) -> Result<Option<Option<String>>, String> {
        unsafe { query_project_impl(path, thread_id) }
    }
    include!("codex_identity_sqlite.in.rs");
}

// The included query is shared by the Windows and Unix SQLite link declarations above.
fn database_project(path: &Path, thread_id: &str) -> Result<Option<Option<String>>, String> {
    sqlite::project(path, thread_id)
}

#[cfg(not(any(windows, unix)))]
fn database_project(_: &Path, _: &str) -> Result<Option<Option<String>>, String> {
    Err("Codex membership requires SQLite support on this platform.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_assigned_projectless_inherited_and_rejects_missing_or_invalid_membership() {
        let home = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.test-run/mcp-codex-membership")
            .join(std::process::id().to_string());
        std::fs::create_dir_all(&home).unwrap();
        let db_path = home.join("state_5.sqlite");
        if db_path.exists() {
            std::fs::remove_file(&db_path).unwrap();
        }
        sqlite::fixture(&db_path);
        let mut state = json!({
            "local-projects": {
                "legacy-a":{"id":"legacy-a","name":"Project A"},
                "legacy-b":{"id":"legacy-b","name":"Project B"}
            },
            "thread-project-assignments": {
                "assigned":{"projectKind":"local","projectId":"modern-a"}
            },
            "projectless-thread-ids": ["projectless"],
            "thread-project-membership-host-ids": {"assigned":"local","projectless":"local"},
            "app-server-project-id-by-legacy-project-id-by-host": {"local:fixture":{"legacy-a":"modern-a"}}
        });
        std::fs::write(home.join(".codex-global-state.json"), state.to_string()).unwrap();
        let catalog = vec![
            ProjectInfo {
                id: "legacy-a".into(),
                name: "Project A".into(),
            },
            ProjectInfo {
                id: "legacy-b".into(),
                name: "Project B".into(),
            },
        ];
        assert_eq!(
            resolve_at(&home, "assigned", &catalog).unwrap(),
            Membership::Project("legacy-a".into())
        );
        assert_eq!(
            resolve_at(&home, "projectless", &catalog).unwrap(),
            Membership::Projectless
        );
        assert_eq!(
            resolve_at(&home, "child", &catalog).unwrap(),
            Membership::Project("legacy-a".into())
        );
        assert!(resolve_at(&home, "unknown", &catalog).is_err());
        assert!(resolve_at(&home, "cycle", &catalog).is_err());
        assert!(resolve_at(&home, "orphan", &catalog).is_err());

        state["thread-project-membership-host-ids"]["assigned"] = json!("remote");
        std::fs::write(home.join(".codex-global-state.json"), state.to_string()).unwrap();
        assert!(resolve_at(&home, "assigned", &catalog).is_err());
        state["thread-project-membership-host-ids"]["assigned"] = json!("local");
        state["projectless-thread-ids"] = json!(["assigned"]);
        std::fs::write(home.join(".codex-global-state.json"), state.to_string()).unwrap();
        assert!(resolve_at(&home, "assigned", &catalog)
            .unwrap_err()
            .contains("conflicts"));
        assert!(resolve_at(&home, "assigned", &[]).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn persists_a_stable_host_id_and_rejects_bad_files() {
        let home = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.test-run/mcp-host-id")
            .join(std::process::id().to_string());
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let first = host_id_at(&home).unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(host_id_at(&home).unwrap(), first);
        std::fs::write(home.join("anyblob-host-id"), "invalid id").unwrap();
        assert!(host_id_at(&home).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }
}
