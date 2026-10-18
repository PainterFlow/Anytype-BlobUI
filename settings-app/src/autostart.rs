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

#[cfg(windows)]
mod windows {
    use super::{startup_command, RUN_KEY_PATH, RUN_VALUE_NAME};
    use std::mem::size_of_val;
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
            ERROR_SUCCESS => Ok(value_type == REG_SZ && size >= size_of::<u16>() as u32),
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
    use super::startup_command;

    #[test]
    fn startup_command_quotes_executable_paths_with_spaces_and_launches_normally() {
        let command = startup_command(r"C:\Program Files\BlobUI\settings-app.exe");
        assert_eq!(command, r#""C:\Program Files\BlobUI\settings-app.exe""#);
        assert!(!command.contains("--background"));
    }
}
