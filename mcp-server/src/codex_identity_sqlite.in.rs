const SQLITE_ERROR: &str = "Could not read Codex's local membership database.";
const SQLITE_OPEN_READONLY: c_int = 1;
#[cfg(test)]
const SQLITE_OPEN_READWRITE: c_int = 2;
#[cfg(test)]
const SQLITE_OPEN_CREATE: c_int = 4;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_NULL: c_int = 5;

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

fn open(path: &Path, flags: c_int) -> Result<Database, String> {
    let path = CString::new(path.to_str().ok_or(SQLITE_ERROR)?).map_err(|_| SQLITE_ERROR)?;
    let mut db = ptr::null_mut();
    let result = unsafe { sqlite3_open_v2(path.as_ptr(), &mut db, flags, ptr::null()) };
    if result != 0 {
        if !db.is_null() {
            unsafe {
                sqlite3_close(db);
            }
        }
        return Err(SQLITE_ERROR.into());
    }
    let db = Database(db);
    unsafe {
        sqlite3_busy_timeout(db.0, 1000);
    }
    Ok(db)
}

fn prepare(db: &Database, sql: &str) -> Result<Statement, String> {
    let sql = CString::new(sql).map_err(|_| SQLITE_ERROR)?;
    let mut stmt = ptr::null_mut();
    if unsafe { sqlite3_prepare_v2(db.0, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) } != 0 {
        return Err(SQLITE_ERROR.into());
    }
    Ok(Statement(stmt))
}

unsafe fn query_project_impl(
    path: &Path,
    thread_id: &str,
) -> Result<Option<Option<String>>, String> {
    let db = open(path, SQLITE_OPEN_READONLY)?;
    let has_parent = prepare(&db, "SELECT attached_thread_parent_id FROM threads LIMIT 0").is_ok();
    let mut current = thread_id.to_owned();
    let mut visited = std::collections::BTreeSet::new();
    for _ in 0..32 {
        if !visited.insert(current.clone()) {
            return Err(SQLITE_ERROR.into());
        }
        let thread = CString::new(current.as_str()).map_err(|_| SQLITE_ERROR)?;
        let stmt = prepare(
            &db,
            if has_parent {
                "SELECT project_id, attached_thread_parent_id FROM threads WHERE id = ?1"
            } else {
                "SELECT project_id FROM threads WHERE id = ?1"
            },
        )?;
        if sqlite3_bind_text(stmt.0, 1, thread.as_ptr(), -1, None) != 0 {
            return Err(SQLITE_ERROR.into());
        }
        match sqlite3_step(stmt.0) {
            SQLITE_DONE if visited.len() == 1 => return Ok(None),
            SQLITE_DONE => return Err(SQLITE_ERROR.into()),
            SQLITE_ROW => {
                let value = |column: c_int| -> Result<Option<String>, String> {
                    if sqlite3_column_type(stmt.0, column) == SQLITE_NULL {
                        return Ok(None);
                    }
                    let text = sqlite3_column_text(stmt.0, column);
                    if text.is_null() {
                        return Err(SQLITE_ERROR.into());
                    }
                    let value = CStr::from_ptr(text.cast())
                        .to_str()
                        .map_err(|_| SQLITE_ERROR)?;
                    if value.is_empty() {
                        return Err(SQLITE_ERROR.into());
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
            _ => return Err(SQLITE_ERROR.into()),
        }
    }
    Err(SQLITE_ERROR.into())
}

#[cfg(test)]
pub fn fixture(path: &Path) {
    let db = open(path, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE).unwrap();
    for sql in [
        "CREATE TABLE threads (id TEXT PRIMARY KEY, project_id TEXT, attached_thread_parent_id TEXT)",
        "INSERT INTO threads VALUES ('parent','modern-a',NULL),('child',NULL,'parent'),('projectless',NULL,NULL),('cycle',NULL,'cycle'),('orphan',NULL,'missing')",
    ] {
        assert_eq!(unsafe { sqlite3_step(prepare(&db, sql).unwrap().0) }, SQLITE_DONE);
    }
}
