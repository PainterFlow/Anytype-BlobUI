use std::path::{Path, PathBuf};

#[path = "../../installer/src/mcp_path.rs"]
#[allow(dead_code)]
mod mcp_path;

pub fn ensure_launcher_is_on_path() -> Result<bool, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Could not locate BlobUI Settings: {error}"))?;
    let Some(app_root) = launcher_app_root(&executable) else {
        return Ok(false);
    };
    mcp_path::add_launcher_directory(&app_root)
}

fn launcher_app_root(executable: &Path) -> Option<PathBuf> {
    let directory = executable.parent()?;
    executable
        .ancestors()
        .skip(1)
        .take(5)
        .find(|candidate| {
            candidate.join("bin/blob-ui.exe").is_file()
                && (candidate.join(".blobui-app-managed-files.json").is_file()
                    || (*candidate == directory && candidate.join(".blobui-dev-build").is_file()))
        })
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::launcher_app_root;
    use std::path::{Path, PathBuf};

    #[test]
    fn finds_launchers_for_installed_and_developer_builds() {
        let root = fixture();
        let root_executable = root.join("blobui-settings.exe");
        let nested_executable = root.join("settings-app/dist/blobui-settings.exe");
        std::fs::create_dir_all(root_executable.parent().unwrap()).unwrap();
        std::fs::create_dir_all(nested_executable.parent().unwrap()).unwrap();
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join(".blobui-app-managed-files.json"), b"{}").unwrap();
        std::fs::write(root.join("bin/blob-ui.exe"), b"fixture").unwrap();

        assert_eq!(launcher_app_root(&root_executable), Some(root.clone()));
        assert_eq!(launcher_app_root(&nested_executable), Some(root.clone()));

        std::fs::remove_file(root.join("bin/blob-ui.exe")).unwrap();
        assert_eq!(launcher_app_root(&root_executable), None);
        assert_eq!(launcher_app_root(&nested_executable), None);

        std::fs::write(root.join("bin/blob-ui.exe"), b"fixture").unwrap();
        std::fs::remove_file(root.join(".blobui-app-managed-files.json")).unwrap();
        assert_eq!(launcher_app_root(&root_executable), None);
        assert_eq!(launcher_app_root(&nested_executable), None);

        std::fs::write(root.join(".blobui-dev-build"), b"").unwrap();
        assert_eq!(launcher_app_root(&root_executable), Some(root.clone()));
        assert_eq!(launcher_app_root(&nested_executable), None);

        std::fs::remove_dir_all(root).unwrap();
    }

    fn fixture() -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".test-run")
            .join(format!("mcp-launcher-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }
}
