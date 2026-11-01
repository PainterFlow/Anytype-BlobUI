//! Resolve host-supplied chat IDs from local Codex metadata, never from tool arguments.
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Membership {
    Project(String),
    Projectless,
}

fn canonical_project(state: &Value, id: &str) -> Result<Membership, String> {
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
    Ok(Membership::Project(
        legacy.into_iter().next().unwrap_or_else(|| id.into()),
    ))
}

pub fn resolve_at(home: &Path, thread: &str) -> Result<Membership, String> {
    let state: Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".codex-global-state.json")).map_err(|_| {
            "Could not read Codex chat membership. Retry after opening the chat in Codex."
        })?,
    )
    .map_err(|_| "Codex chat membership is not valid JSON.")?;
    // A host marker alone does not prove that SQLite has the project assignment.
    // Conflicting metadata must never broaden a project's access to global rules.
    let migrated = state["thread-project-membership-host-ids"].get(thread);
    if migrated.is_some_and(|host| host.as_str() != Some("local")) {
        return Err("Anyblob project permissions support local Windows chats only.".into());
    }
    if migrated.is_none() {
        if let Some(assignment) = state["thread-project-assignments"].get(thread) {
            if assignment["projectKind"] != "local" {
                return Err("Anyblob project permissions support local Windows chats only.".into());
            }
            let id = assignment["projectId"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or("Codex returned invalid chat project membership.")?;
            return canonical_project(&state, id);
        }
        if state["projectless-thread-ids"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(thread)))
        {
            return Ok(Membership::Projectless);
        }
    }
    let mut databases = Vec::new();
    for entry in std::fs::read_dir(home).map_err(|_| "Could not inspect Codex chat membership.")? {
        let entry = entry.map_err(|_| "Could not inspect Codex chat membership.")?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(version) = name
            .strip_prefix("state_")
            .and_then(|s| s.strip_suffix(".sqlite"))
            .and_then(|s| s.parse::<u32>().ok())
        {
            databases.push((version, entry.path()));
        }
    }
    databases.sort_by_key(|(version, _)| *version);
    let path = &databases
        .last()
        .ok_or(
            "Codex has not supplied resolvable chat membership. Open the chat in Codex and retry.",
        )?
        .1;
    let membership = match database_project(path, thread)? {
        Some(Some(id)) => canonical_project(&state, &id)?,
        Some(None) => Membership::Projectless,
        None => return Err("Codex chat identity was not found in local membership metadata. Open the chat in Codex and retry.".into()),
    };
    if let Some(assignment) = state["thread-project-assignments"].get(thread) {
        if assignment["projectKind"] != "local" {
            return Err("Anyblob project permissions support local Windows chats only.".into());
        }
        let id = assignment["projectId"].as_str().filter(|id| !id.is_empty())
            .ok_or("Codex returned invalid chat project membership.")?;
        if canonical_project(&state, id)? != membership {
            return Err("Codex chat project membership conflicts between local records. Refresh the chat's project assignment and retry.".into());
        }
    }
    if membership != Membership::Projectless && state["projectless-thread-ids"]
        .as_array().is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(thread))) {
        return Err("Codex chat project membership conflicts between local records. Refresh the chat's project assignment and retry.".into());
    }
    Ok(membership)
}

pub fn resolve(thread: &str) -> Result<Membership, String> {
    #[cfg(test)]
    {
        let _ = thread;
        return Err("Native tests require isolated Codex membership fixtures.".into());
    }
    #[cfg(not(test))]
    {
        let home = PathBuf::from(
            std::env::var_os("USERPROFILE").ok_or("Windows user profile is unavailable.")?,
        )
        .join(".codex");
        resolve_at(&home, thread)
    }
}

// Windows supplies SQLite. Open the database READONLY and bind IDs as values.
// No Node/Python runtime, credentials, chat content, or Codex writes are required.
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
    struct Database(*mut c_void);
    impl Drop for Database {
        fn drop(&mut self) {
            unsafe {
                sqlite3_close(self.0);
            }
        }
    }
    struct Statement(*mut c_void);
    impl Drop for Statement {
        fn drop(&mut self) {
            unsafe {
                sqlite3_finalize(self.0);
            }
        }
    }
    const ERROR: &str =
        "Could not read Codex's local membership database. Retry after Codex finishes saving.";
    fn open(path: &Path, flags: c_int) -> Result<Database, String> {
        let path = CString::new(path.to_str().ok_or(ERROR)?).map_err(|_| ERROR)?;
        let mut db = ptr::null_mut();
        let result = unsafe { sqlite3_open_v2(path.as_ptr(), &mut db, flags, ptr::null()) };
        let db = Database(db);
        if result != 0 {
            return Err(ERROR.into());
        }
        unsafe {
            sqlite3_busy_timeout(db.0, 1000);
        }
        Ok(db)
    }
    fn prepare(db: &Database, sql: &str) -> Result<Statement, String> {
        let sql = CString::new(sql).map_err(|_| ERROR)?;
        let mut stmt = ptr::null_mut();
        if unsafe { sqlite3_prepare_v2(db.0, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) } != 0 {
            return Err(ERROR.into());
        }
        Ok(Statement(stmt))
    }
    pub fn project(path: &Path, thread: &str) -> Result<Option<Option<String>>, String> {
        let db = open(path, 1)?; // SQLITE_OPEN_READONLY; no create or write permission.
        let has_parent =
            prepare(&db, "SELECT attached_thread_parent_id FROM threads LIMIT 0").is_ok();
        let mut current = thread.to_string();
        let mut visited = std::collections::BTreeSet::new();
        for _ in 0..32 {
            if !visited.insert(current.clone()) {
                return Err(ERROR.into());
            }
            let thread = CString::new(current.as_str()).map_err(|_| ERROR)?;
            let stmt = prepare(
                &db,
                if has_parent {
                    "SELECT project_id, attached_thread_parent_id FROM threads WHERE id = ?1"
                } else {
                    "SELECT project_id FROM threads WHERE id = ?1"
                },
            )?;
            if unsafe { sqlite3_bind_text(stmt.0, 1, thread.as_ptr(), -1, None) } != 0 {
                return Err(ERROR.into());
            }
            match unsafe { sqlite3_step(stmt.0) } {
                101 => {
                    return if visited.len() == 1 {
                        Ok(None)
                    } else {
                        Err(ERROR.into())
                    }
                }
                100 => {
                    let value = |column| -> Result<Option<String>, String> {
                        if unsafe { sqlite3_column_type(stmt.0, column) } == 5 {
                            return Ok(None);
                        }
                        let text = unsafe { sqlite3_column_text(stmt.0, column) };
                        if text.is_null() {
                            return Err(ERROR.into());
                        }
                        let value = unsafe { CStr::from_ptr(text.cast()) }
                            .to_str()
                            .map_err(|_| ERROR)?;
                        if value.is_empty() {
                            return Err(ERROR.into());
                        }
                        Ok(Some(value.into()))
                    };
                    if let Some(id) = value(0)? {
                        return Ok(Some(Some(id)));
                    }
                    if has_parent {
                        if let Some(parent) = value(1)? {
                            current = parent;
                            continue;
                        }
                    }
                    return Ok(Some(None));
                }
                _ => return Err(ERROR.into()),
            }
        }
        Err(ERROR.into())
    }
    #[cfg(test)]
    pub fn fixture(path: &Path) {
        let db = open(path, 2 | 4).unwrap();
        for sql in ["CREATE TABLE threads (id TEXT PRIMARY KEY, project_id TEXT, attached_thread_parent_id TEXT)",
            "INSERT INTO threads VALUES ('chat-a','modern-a',NULL),('chat-b','modern-b',NULL),('no-project',NULL,NULL),('child',NULL,'chat-a'),('cycle',NULL,'cycle'),('orphan',NULL,'missing')"] {
            assert_eq!(unsafe { sqlite3_step(prepare(&db, sql).unwrap().0) }, 101);
        }
    }
}
#[cfg(windows)]
use sqlite::project as database_project;
#[cfg(not(windows))]
fn database_project(_: &Path, _: &str) -> Result<Option<Option<String>>, String> {
    Err("Codex membership requires Windows.".into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn resolves_migrated_and_legacy_chats_without_active_project_inference() {
        let home = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.test-run/codex-membership");
        std::fs::create_dir_all(&home).unwrap();
        let home = home.join(format!("{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        let database = home.join("state_5.sqlite");
        if database.exists() {
            std::fs::remove_file(&database).unwrap();
        }
        sqlite::fixture(&database);
        let mut state = json!({"selected-project":"wrong", "thread-project-assignments":{
            "old":{"projectKind":"local","projectId":"legacy-b"},
            "chat-a":{"projectKind":"local","projectId":"legacy-a"}},
            "thread-project-membership-host-ids":{"chat-a":"local","chat-b":"local","no-project":"local"},
            "app-server-project-id-by-legacy-project-id-by-host":{"local:fixture":{"legacy-a":"modern-a","legacy-b":"modern-b"}}});
        let save = |state: &Value| {
            std::fs::write(home.join(".codex-global-state.json"), state.to_string()).unwrap()
        };
        save(&state);
        assert_eq!(
            resolve_at(&home, "chat-a").unwrap(),
            Membership::Project("legacy-a".into())
        );
        assert_eq!(
            resolve_at(&home, "chat-b").unwrap(),
            Membership::Project("legacy-b".into())
        );
        assert_eq!(
            resolve_at(&home, "old").unwrap(),
            Membership::Project("legacy-b".into())
        );
        assert_eq!(
            resolve_at(&home, "no-project").unwrap(),
            Membership::Projectless
        );
        assert_eq!(
            resolve_at(&home, "child").unwrap(),
            Membership::Project("legacy-a".into())
        );
        // A host marker plus a null SQLite project must not enable global reads.
        state["thread-project-assignments"]["no-project"] = json!({"projectKind":"local","projectId":"legacy-a"});
        save(&state);
        assert!(resolve_at(&home, "no-project").unwrap_err().contains("conflicts"));
        state["thread-project-assignments"].as_object_mut().unwrap().remove("no-project");
        state["thread-project-assignments"]["chat-a"]["projectId"] = "legacy-b".into();
        save(&state);
        assert!(resolve_at(&home, "chat-a").unwrap_err().contains("conflicts"));
        state["thread-project-assignments"]["chat-a"]["projectId"] = "modern-a".into();
        save(&state);
        assert_eq!(resolve_at(&home, "chat-a").unwrap(), Membership::Project("legacy-a".into()));
        state["projectless-thread-ids"] = json!(["chat-a"]);
        save(&state);
        assert!(resolve_at(&home, "chat-a").unwrap_err().contains("conflicts"));
        state["projectless-thread-ids"] = json!([]);
        save(&state);
        assert!(resolve_at(&home, "cycle").is_err());
        assert!(resolve_at(&home, "orphan").is_err());
        assert!(resolve_at(&home, "unknown").is_err());
        assert!(database_project(&database, "' OR 1=1 --")
            .unwrap()
            .is_none());
        state["thread-project-membership-host-ids"]["chat-a"] = "remote".into();
        save(&state);
        assert!(resolve_at(&home, "chat-a").is_err());
    }
}
