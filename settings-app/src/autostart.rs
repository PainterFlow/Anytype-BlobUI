const RUN_VALUE_NAME: &str = "BlobUI Settings";
const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AutostartMode {
    #[default]
    #[serde(rename = "off")]
    Disabled,
    #[serde(rename = "foreground")]
    Normal,
    #[serde(rename = "background")]
    Background,
}

#[derive(Clone, Debug)]
pub struct RegistrySnapshot(Option<(u32, Vec<u8>)>);

pub fn snapshot() -> Result<RegistrySnapshot, String> {
    #[cfg(windows)]
    {
        windows::snapshot()
    }
    #[cfg(not(windows))]
    {
        Ok(RegistrySnapshot(None))
    }
}
pub fn restore(value: &RegistrySnapshot) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::restore(value)
    }
    #[cfg(not(windows))]
    {
        let _ = value;
        Ok(())
    }
}
pub fn get_mode() -> Result<AutostartMode, String> {
    #[cfg(windows)]
    {
        windows::get_mode()
    }
    #[cfg(not(windows))]
    {
        Ok(AutostartMode::Disabled)
    }
}
pub fn set_mode(mode: AutostartMode) -> Result<AutostartMode, String> {
    #[cfg(windows)]
    {
        windows::set_mode(mode)
    }
    #[cfg(not(windows))]
    {
        if mode == AutostartMode::Disabled {
            Ok(mode)
        } else {
            Err("Starting BlobUI Settings when you sign in is supported on Windows.".into())
        }
    }
}
pub fn is_enabled() -> Result<bool, String> {
    #[cfg(windows)]
    {
        Ok(get_mode()? != AutostartMode::Disabled)
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

pub fn set_enabled(enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        Ok(set_mode(if enabled {
            AutostartMode::Normal
        } else {
            AutostartMode::Disabled
        })? != AutostartMode::Disabled)
    }
    #[cfg(not(windows))]
    {
        if enabled {
            Err("Starting BlobUI Settings when you sign in is supported on Windows.".into())
        } else {
            Ok(false)
        }
    }
}

fn startup_command(executable: &str) -> String {
    format!("\"{}\"", shell_path(executable))
}

// Explorer's Run commands use shell paths, while an installer launch can leave
// current_exe() with the verbatim prefix used by Windows canonicalize().
fn shell_path(path: &str) -> String {
    if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{path}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
    }
}

fn startup_entry_mode(value: &str, executable: &std::path::Path) -> Option<AutostartMode> {
    let quoted = value.strip_prefix('"')?;
    let end = quoted.find('"')?;
    let command_path = &quoted[..end];
    let mode = match &quoted[end + 1..] {
        "" => AutostartMode::Normal,
        " --background" => AutostartMode::Background,
        _ => return None,
    };
    startup_executable_aliases(executable)
        .iter()
        .any(|alias| {
            shell_path(command_path).eq_ignore_ascii_case(&shell_path(&alias.to_string_lossy()))
        })
        .then_some(mode)
}
#[cfg(test)]
fn startup_entry_matches_executable(value: &str, executable: &std::path::Path) -> bool {
    startup_entry_mode(value, executable).is_some()
}

fn startup_executable_aliases(executable: &std::path::Path) -> Vec<std::path::PathBuf> {
    use std::path::Path;

    let mut aliases = vec![executable.to_path_buf()];
    if !executable
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("blobui-settings.exe"))
    {
        return aliases;
    }

    let Some(parent) = executable.parent() else {
        return aliases;
    };
    let install_root = if parent
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("dist"))
        && parent
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name.eq_ignore_ascii_case("settings-app"))
    {
        parent.parent().and_then(Path::parent)
    } else {
        Some(parent)
    };
    let Some(install_root) = install_root else {
        return aliases;
    };

    aliases.push(install_root.join("blobui-settings.exe"));
    aliases.push(
        install_root
            .join("settings-app")
            .join("dist")
            .join("blobui-settings.exe"),
    );
    aliases
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS,
    };
    use windows_sys::Win32::System::Registry::*;
    struct RegistryKey(HKEY);
    impl Drop for RegistryKey {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    fn error(action: &str, code: u32) -> String {
        format!(
            "Windows could not {action}: {}",
            std::io::Error::from_raw_os_error(code as i32)
        )
    }
    fn open(access: u32) -> Result<Option<RegistryKey>, String> {
        let mut key = null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY_PATH).as_ptr(),
                0,
                access,
                &mut key,
            )
        };
        match status {
            ERROR_SUCCESS => Ok(Some(RegistryKey(key))),
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Ok(None),
            code => Err(error("inspect the per-user startup settings", code)),
        }
    }
    pub fn snapshot() -> Result<RegistrySnapshot, String> {
        let Some(key) = open(KEY_QUERY_VALUE)? else {
            return Ok(RegistrySnapshot(None));
        };
        let mut kind = 0;
        let mut size = 0;
        let name = wide(RUN_VALUE_NAME);
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                null(),
                &mut kind,
                null_mut(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(RegistrySnapshot(None));
        }
        if status != ERROR_SUCCESS {
            return Err(error("read the per-user startup setting", status));
        }
        if size > 1024 * 1024 {
            return Err("The per-user startup setting is too large to preserve.".into());
        }
        let mut bytes = vec![0; size as usize];
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                null(),
                &mut kind,
                bytes.as_mut_ptr(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(RegistrySnapshot(None));
        }
        if status != ERROR_SUCCESS {
            return Err(error("read the per-user startup setting", status));
        }
        bytes.truncate(size as usize);
        Ok(RegistrySnapshot(Some((kind, bytes))))
    }
    fn decode(snapshot: &RegistrySnapshot) -> Option<String> {
        let (kind, bytes) = snapshot.0.as_ref()?;
        if *kind != REG_SZ || bytes.len() < 2 || bytes.len() % 2 != 0 {
            return None;
        }
        let mut units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        if units.pop() != Some(0) || units.contains(&0) {
            return None;
        }
        String::from_utf16(&units).ok()
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn registry_ownership_rejects_non_string_and_hidden_trailing_commands() {
            let command = startup_command(r"C:\BlobUI\blobui-settings.exe");
            let raw: Vec<u8> = wide(&command)
                .iter()
                .flat_map(|unit| unit.to_le_bytes())
                .collect();
            assert_eq!(
                decode(&RegistrySnapshot(Some((REG_SZ, raw.clone())))),
                Some(command.clone())
            );
            assert_eq!(
                decode(&RegistrySnapshot(Some((REG_BINARY, raw.clone())))),
                None
            );
            let mut unterminated = raw.clone();
            unterminated.truncate(unterminated.len() - 2);
            assert_eq!(
                decode(&RegistrySnapshot(Some((REG_SZ, unterminated)))),
                None
            );
            let mut hidden = raw;
            hidden.extend(wide(" --extra").iter().flat_map(|unit| unit.to_le_bytes()));
            assert_eq!(decode(&RegistrySnapshot(Some((REG_SZ, hidden)))), None);
        }
    }

    fn executable() -> Result<std::path::PathBuf, String> {
        std::env::current_exe()
            .map_err(|e| format!("Could not locate BlobUI Settings for startup: {e}"))
    }
    pub fn get_mode() -> Result<AutostartMode, String> {
        let saved = snapshot()?;
        let executable = executable()?;
        Ok(decode(&saved)
            .and_then(|v| startup_entry_mode(&v, &executable))
            .unwrap_or(AutostartMode::Disabled))
    }
    pub fn restore(snapshot: &RegistrySnapshot) -> Result<(), String> {
        let name = wide(RUN_VALUE_NAME);
        let Some((kind, bytes)) = snapshot.0.as_ref() else {
            let Some(key) = open(KEY_SET_VALUE)? else {
                return Ok(());
            };
            let status = unsafe { RegDeleteValueW(key.0, name.as_ptr()) };
            return match status {
                ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
                code => Err(error("remove the per-user startup setting", code)),
            };
        };
        let mut key = null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY_PATH).as_ptr(),
                0,
                null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                null(),
                &mut key,
                null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(error("prepare the per-user startup setting", status));
        }
        let key = RegistryKey(key);
        let status = unsafe {
            RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                *kind,
                bytes.as_ptr(),
                bytes.len() as u32,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(error("save the per-user startup setting", status))
        }
    }
    pub fn set_mode(mode: AutostartMode) -> Result<AutostartMode, String> {
        let saved = snapshot()?;
        let executable = executable()?;
        let owned = decode(&saved)
            .and_then(|v| startup_entry_mode(&v, &executable))
            .is_some();
        if saved.0.is_some() && !owned {
            if mode == AutostartMode::Disabled {
                return Ok(mode);
            }
            return Err("The BlobUI Settings startup name belongs to another command. It was left unchanged.".into());
        }
        let replacement = if mode == AutostartMode::Disabled {
            RegistrySnapshot(None)
        } else {
            let path = executable.to_str().ok_or(
                "The BlobUI Settings path cannot be represented as a Windows startup command.",
            )?;
            let command = format!(
                "{}{}",
                startup_command(path),
                if mode == AutostartMode::Background {
                    " --background"
                } else {
                    ""
                }
            );
            let bytes = wide(&command)
                .iter()
                .flat_map(|u| u.to_le_bytes())
                .collect();
            RegistrySnapshot(Some((REG_SZ, bytes)))
        };
        restore(&replacement)?;
        Ok(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::{startup_command, startup_entry_matches_executable};
    use std::path::Path;

    #[test]
    fn startup_modes_accept_only_owned_quoted_commands_with_supported_arguments() {
        use super::{startup_entry_mode, AutostartMode};
        let executable = Path::new(r"C:\Program Files\BlobUI\blobui-settings.exe");
        let command = startup_command(&executable.to_string_lossy());
        assert_eq!(
            startup_entry_mode(&command, executable),
            Some(AutostartMode::Normal)
        );
        assert_eq!(
            startup_entry_mode(&format!("{command} --background"), executable),
            Some(AutostartMode::Background)
        );
        for suffix in [
            " --background --extra",
            " --background ",
            " --foreground",
            " extra",
        ] {
            assert_eq!(
                startup_entry_mode(&format!("{command}{suffix}"), executable),
                None
            );
        }
        assert_eq!(
            startup_entry_mode(&executable.to_string_lossy(), executable),
            None
        );
        assert_eq!(
            startup_entry_mode(r#""C:\Other\blobui-settings.exe" --background"#, executable),
            None
        );
    }

    #[test]
    fn startup_command_quotes_executable_paths_with_spaces_and_launches_normally() {
        let command = startup_command(r"C:\Program Files\BlobUI\settings-app.exe");
        assert_eq!(command, r#""C:\Program Files\BlobUI\settings-app.exe""#);
        assert!(!command.contains("--background"));
    }

    #[test]
    fn startup_state_matches_only_the_running_settings_executable() {
        let executable = Path::new(r"C:\Program Files\BlobUI\blobui-settings.exe");
        assert!(startup_entry_matches_executable(
            r#""C:\Program Files\BlobUI\blobui-settings.exe""#,
            executable
        ));
        assert!(startup_entry_matches_executable(
            r#""c:\program files\blobui\BLOBUI-SETTINGS.EXE""#,
            executable
        ));
        assert!(!startup_entry_matches_executable(
            r#""C:\Old BlobUI\blobui-settings.exe""#,
            executable
        ));
        assert!(startup_entry_matches_executable(
            r#""C:\Program Files\BlobUI\blobui-settings.exe" --background"#,
            executable
        ));
    }

    #[test]
    fn startup_state_matches_both_installed_executable_aliases() {
        let root_executable = Path::new(r"C:\Program Files\BlobUI\blobui-settings.exe");
        let dist_executable =
            Path::new(r"C:\Program Files\BlobUI\settings-app\dist\blobui-settings.exe");

        assert!(startup_entry_matches_executable(
            r#""C:\Program Files\BlobUI\blobui-settings.exe""#,
            dist_executable
        ));
        assert!(startup_entry_matches_executable(
            r#""C:\Program Files\BlobUI\settings-app\dist\blobui-settings.exe""#,
            root_executable
        ));
        assert!(!startup_entry_matches_executable(
            r#""C:\Old BlobUI\blobui-settings.exe""#,
            dist_executable
        ));
    }

    #[test]
    fn startup_commands_use_shell_paths_for_verbatim_install_locations() {
        assert_eq!(
            startup_command(r"\\?\C:\Program Files\BlobUI\blobui-settings.exe"),
            r#""C:\Program Files\BlobUI\blobui-settings.exe""#
        );
        assert_eq!(
            startup_command(r"\\?\UNC\server\apps\BlobUI\blobui-settings.exe"),
            r#""\\server\apps\BlobUI\blobui-settings.exe""#
        );
    }

    #[test]
    fn startup_detection_accepts_verbatim_paths_in_either_direction() {
        for (ordinary, verbatim) in [
            (
                r"C:\Program Files\BlobUI\blobui-settings.exe",
                r"\\?\C:\Program Files\BlobUI\blobui-settings.exe",
            ),
            (
                r"\\server\apps\BlobUI\blobui-settings.exe",
                r"\\?\UNC\server\apps\BlobUI\blobui-settings.exe",
            ),
        ] {
            assert!(startup_entry_matches_executable(
                &startup_command(ordinary),
                Path::new(verbatim)
            ));
            assert!(startup_entry_matches_executable(
                &format!("\"{verbatim}\""),
                Path::new(ordinary)
            ));
        }
        assert!(startup_entry_matches_executable(
            r#""C:\Program Files\BlobUI\blobui-settings.exe""#,
            Path::new(r"\\?\C:\Program Files\BlobUI\settings-app\dist\blobui-settings.exe")
        ));
        assert!(!startup_entry_matches_executable(
            r#""\\?\C:\Other BlobUI\blobui-settings.exe""#,
            Path::new(r"C:\Program Files\BlobUI\blobui-settings.exe")
        ));
    }
}
