const RUN_VALUE_NAME: &str = "BlobUI Settings";
const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn is_enabled() -> Result<bool, String> {
    #[cfg(windows)]
    {
        windows::is_enabled()
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

pub fn set_enabled(enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        windows::set_enabled(enabled)
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
    format!("\"{executable}\"")
}

fn startup_entry_matches_executable(value: &str, executable: &std::path::Path) -> bool {
    let command_path = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value);
    if command_path.contains('"') {
        return false;
    }
    startup_executable_aliases(executable)
        .iter()
        .any(|alias| command_path.eq_ignore_ascii_case(&alias.to_string_lossy()))
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
    use super::{
        startup_command, startup_entry_matches_executable, RUN_KEY_PATH, RUN_VALUE_NAME,
    };
    use std::mem::{size_of, size_of_val};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

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

    fn registry_error(action: &str, code: u32) -> String {
        format!(
            "Windows could not {action}: {}",
            std::io::Error::from_raw_os_error(code as i32)
        )
    }

    fn open_run_key(access: u32) -> Result<Option<RegistryKey>, String> {
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
            error => Err(registry_error(
                "inspect the per-user startup settings",
                error,
            )),
        }
    }

    pub fn is_enabled() -> Result<bool, String> {
        let Some(key) = open_run_key(KEY_QUERY_VALUE)? else {
            return Ok(false);
        };
        let mut value_type = 0;
        let mut size = 0;
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                wide(RUN_VALUE_NAME).as_ptr(),
                null(),
                &mut value_type,
                null_mut(),
                &mut size,
            )
        };
        match status {
            ERROR_SUCCESS => {
                if value_type != REG_SZ
                    || size < size_of::<u16>() as u32
                    || size > 64 * 1024 + 8
                    || size % 2 != 0
                {
                    return Ok(false);
                }
                let mut value = vec![0u16; size as usize / size_of::<u16>()];
                let mut read_size = size;
                let status = unsafe {
                    RegQueryValueExW(
                        key.0,
                        wide(RUN_VALUE_NAME).as_ptr(),
                        null(),
                        &mut value_type,
                        value.as_mut_ptr().cast(),
                        &mut read_size,
                    )
                };
                match status {
                    ERROR_SUCCESS => {}
                    ERROR_FILE_NOT_FOUND => return Ok(false),
                    error => {
                        return Err(registry_error(
                            "read the per-user startup setting",
                            error,
                        ))
                    }
                }
                if value_type != REG_SZ
                    || read_size > size
                    || read_size % 2 != 0
                {
                    return Ok(false);
                }
                value.truncate(read_size as usize / size_of::<u16>());
                if let Some(end) = value.iter().position(|unit| *unit == 0) {
                    value.truncate(end);
                }
                let command = String::from_utf16(&value)
                    .map_err(|_| "The per-user startup setting is not valid text.".to_string())?;
                let executable = std::env::current_exe().map_err(|error| {
                    format!("Could not locate BlobUI Settings for startup: {error}")
                })?;
                Ok(startup_entry_matches_executable(&command, &executable))
            }
            ERROR_FILE_NOT_FOUND => Ok(false),
            error => Err(registry_error("read the per-user startup setting", error)),
        }
    }

    fn current_command() -> Result<Vec<u16>, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("Could not locate BlobUI Settings for startup: {error}"))?;
        let executable = executable.to_str().ok_or_else(|| {
            "The BlobUI Settings path cannot be represented as a Windows startup command."
                .to_string()
        })?;
        Ok(wide(&startup_command(executable)))
    }

    pub fn set_enabled(enabled: bool) -> Result<bool, String> {
        let value_name = wide(RUN_VALUE_NAME);
        if enabled {
            let command = current_command()?;
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
                return Err(registry_error(
                    "prepare the per-user startup setting",
                    status,
                ));
            }
            let key = RegistryKey(key);
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    command.as_ptr().cast::<u8>(),
                    size_of_val(command.as_slice()),
                )
            };
            let status = unsafe {
                RegSetValueExW(
                    key.0,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                )
            };
            if status != ERROR_SUCCESS {
                return Err(registry_error("save the per-user startup setting", status));
            }
            Ok(true)
        } else {
            let Some(key) = open_run_key(KEY_SET_VALUE)? else {
                return Ok(false);
            };
            let status = unsafe { RegDeleteValueW(key.0, value_name.as_ptr()) };
            match status {
                ERROR_SUCCESS => Ok(false),
                ERROR_FILE_NOT_FOUND => Ok(false),
                error => Err(registry_error("remove the per-user startup setting", error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{startup_command, startup_entry_matches_executable};
    use std::path::Path;

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
        assert!(!startup_entry_matches_executable(
            r#""C:\Program Files\BlobUI\blobui-settings.exe" --background"#,
            executable
        ));
    }

    #[test]
    fn startup_state_matches_both_installed_executable_aliases() {
        let root_executable = Path::new(r"C:\Program Files\BlobUI\blobui-settings.exe");
        let dist_executable = Path::new(
            r"C:\Program Files\BlobUI\settings-app\dist\blobui-settings.exe",
        );

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
}
