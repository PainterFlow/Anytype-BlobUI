//! Transactional file replacement shared with Settings storage tests.
use crate::storage::{
    preflight, prepare_temp, reject_reparse_path, remove_if_exists, replace_file, restore, snapshot,
};
use std::fs;
use std::path::PathBuf;

pub struct FileChange {
    pub path: PathBuf,
    pub contents: Option<Vec<u8>>,
}

pub fn commit_update(changes: Vec<FileChange>) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let mut snapshots = Vec::with_capacity(changes.len());
    for change in &changes {
        if !seen.insert(change.path.clone()) {
            return Err(format!(
                "The update contains a duplicate file target: {}",
                change.path.display()
            ));
        }
        reject_reparse_path(&change.path)?;
        snapshots.push(snapshot(&change.path, false)?);
        if change.contents.is_some() {
            preflight(&change.path)?;
        }
    }

    let mut candidates = Vec::with_capacity(changes.len());
    for (change, before) in changes.iter().zip(snapshots.iter()) {
        if let Some(contents) = &change.contents {
            match prepare_temp(&change.path, contents, before.permissions.as_ref(), false) {
                Ok(path) => candidates.push(Some(path)),
                Err(error) => {
                    for candidate in candidates.iter().flatten() {
                        remove_if_exists(candidate);
                    }
                    return Err(error);
                }
            }
        } else {
            candidates.push(None);
        }
    }

    let result = (|| {
        for (change, candidate) in changes.iter().zip(candidates.iter()) {
            match (&change.contents, candidate) {
                (Some(_), Some(candidate)) => {
                    replace_file(candidate, &change.path).map_err(|error| {
                        format!(
                            "Could not install update file {}: {error}",
                            change.path.display()
                        )
                    })?
                }
                (None, _) => match fs::remove_file(&change.path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "Could not remove obsolete update file {}: {error}",
                            change.path.display()
                        ))
                    }
                },
                _ => return Err("An update file was not prepared safely.".into()),
            }
        }
        Ok(())
    })();

    for candidate in candidates.iter().flatten() {
        remove_if_exists(candidate);
    }
    if let Err(error) = result {
        let mut rollback_errors = Vec::new();
        for before in snapshots.iter().rev() {
            if let Err(rollback) = restore(before) {
                rollback_errors.push(rollback);
            }
        }
        if rollback_errors.is_empty() {
            return Err(error);
        }
        return Err(format!(
            "{error} Rollback could not restore every saved file: {}",
            rollback_errors.join("; ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn failed_uninstall_transaction_restores_prior_deletions_and_replacements() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run")
            .join(format!("installer-rollback-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let owned = root.join("owned.css");
        let backup = root.join("backup.css");
        fs::write(&owned, b"managed theme").unwrap();
        fs::write(&backup, b"previous backup").unwrap();
        crate::storage::FAIL_AFTER_REPLACE.with(|failure| {
            *failure.borrow_mut() = Some(backup.clone());
        });

        let error = commit_update(vec![
            FileChange {
                path: owned.clone(),
                contents: None,
            },
            FileChange {
                path: backup.clone(),
                contents: Some(b"new uninstall backup".to_vec()),
            },
        ])
        .unwrap_err();

        assert!(error.contains("injected post-replacement failure"));
        assert_eq!(
            fs::read(&owned).unwrap(),
            b"managed theme",
            "a file removed earlier in the transaction must be restored"
        );
        assert_eq!(
            fs::read(&backup).unwrap(),
            b"previous backup",
            "a replacement that failed after changing its target must be restored"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transaction_preflights_every_target_before_removing_owned_files() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run")
            .join(format!(
                "installer-preflight-{}-{nonce}",
                std::process::id()
            ));
        fs::create_dir_all(&root).unwrap();
        let owned = root.join("owned.css");
        let invalid_target = root.join("managed-directory");
        fs::write(&owned, b"managed theme").unwrap();
        fs::create_dir(&invalid_target).unwrap();

        assert!(commit_update(vec![
            FileChange {
                path: owned.clone(),
                contents: None
            },
            FileChange {
                path: invalid_target,
                contents: None
            },
        ])
        .is_err());

        assert_eq!(
            fs::read(&owned).unwrap(),
            b"managed theme",
            "an earlier owned file must remain when a later target is invalid"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
