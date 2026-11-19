use std::path::Path;

pub fn add_launcher_directory(app: &Path) -> Result<bool, String> {
    update_launcher_directory(&app.join("bin"), true)
}

pub fn remove_launcher_directory(app: &Path) -> Result<bool, String> {
    update_launcher_directory(&app.join("bin"), false)
}

#[cfg(windows)]
fn update_launcher_directory(directory: &Path, add: bool) -> Result<bool, String> {
    use std::io;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    const MAX_PATH_VALUE_BYTES: u32 = 1024 * 1024;
    let candidate = directory.to_string_lossy().into_owned();
    if candidate.contains(';') || candidate.contains('\0') {
        return Err("The BlobUI MCP launcher folder cannot be added to PATH.".into());
    }

    let key_name = "Environment\0".encode_utf16().collect::<Vec<_>>();
    let value_name = "Path\0".encode_utf16().collect::<Vec<_>>();
    let mut key: HKEY = std::ptr::null_mut();
    let mut disposition = 0;
    let opened = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_name.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            &mut disposition,
        )
    };
    if opened != ERROR_SUCCESS {
        return Err(format!(
            "Windows could not open the current user's Environment settings: {}",
            io::Error::from_raw_os_error(opened as i32)
        ));
    }

    let result = (|| {
        let mut value_type = 0;
        let mut value_bytes = 0;
        let queried = unsafe {
            RegQueryValueExW(
                key,
                value_name.as_ptr(),
                std::ptr::null(),
                &mut value_type,
                std::ptr::null_mut(),
                &mut value_bytes,
            )
        };
        let (current, value_type) = if queried == ERROR_FILE_NOT_FOUND {
            (String::new(), REG_EXPAND_SZ)
        } else if queried != ERROR_SUCCESS {
            return Err(format!(
                "Windows could not read the current user's PATH: {}",
                io::Error::from_raw_os_error(queried as i32)
            ));
        } else {
            if value_type != REG_SZ && value_type != REG_EXPAND_SZ {
                return Err(
                    "The current user's PATH has an unsupported registry value type.".into(),
                );
            }
            if value_bytes > MAX_PATH_VALUE_BYTES || value_bytes % 2 != 0 {
                return Err("The current user's PATH has an invalid registry value.".into());
            }
            let mut value = vec![0u16; value_bytes as usize / 2];
            let queried = unsafe {
                RegQueryValueExW(
                    key,
                    value_name.as_ptr(),
                    std::ptr::null(),
                    &mut value_type,
                    value.as_mut_ptr().cast(),
                    &mut value_bytes,
                )
            };
            if queried != ERROR_SUCCESS || value_type != REG_SZ && value_type != REG_EXPAND_SZ {
                return Err(format!(
                    "Windows could not read the current user's PATH: {}",
                    io::Error::from_raw_os_error(queried as i32)
                ));
            }
            if value_bytes % 2 != 0 || value_bytes as usize / 2 > value.len() {
                return Err("The current user's PATH has an invalid registry value.".into());
            }
            value.truncate(value_bytes as usize / 2);
            if value.pop() != Some(0) || value.contains(&0) {
                return Err("The current user's PATH is not valid text.".into());
            }
            let current = String::from_utf16(&value)
                .map_err(|_| "The current user's PATH is not valid text.")?;
            (current, value_type)
        };

        let Some(updated) = path_with_entry(&current, &candidate, add) else {
            return Ok(false);
        };
        let mut wide = updated.encode_utf16().collect::<Vec<_>>();
        wide.push(0);
        let bytes = wide
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(|| "The current user's PATH is too long to update.".to_string())?;
        let written = unsafe {
            RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                value_type,
                wide.as_ptr().cast(),
                bytes,
            )
        };
        if written != ERROR_SUCCESS {
            return Err(format!(
                "Windows could not update the current user's PATH: {}",
                io::Error::from_raw_os_error(written as i32)
            ));
        }
        notify_environment_changed();
        Ok(true)
    })();

    unsafe { RegCloseKey(key) };
    result
}

#[cfg(not(windows))]
fn update_launcher_directory(_: &Path, _: bool) -> Result<bool, String> {
    Ok(false)
}

fn path_with_entry(current: &str, candidate: &str, add: bool) -> Option<String> {
    if add {
        if current
            .split(';')
            .any(|entry| same_path_entry(entry, candidate))
        {
            return None;
        }
        return Some(if current.is_empty() {
            candidate.to_string()
        } else if current.ends_with(';') {
            format!("{current}{candidate}")
        } else {
            format!("{current};{candidate}")
        });
    }

    let entries = current.split(';').collect::<Vec<_>>();
    let remaining = entries
        .iter()
        .copied()
        .filter(|entry| !same_path_entry(entry, candidate))
        .collect::<Vec<_>>();
    (remaining.len() != entries.len()).then(|| remaining.join(";"))
}

fn same_path_entry(left: &str, right: &str) -> bool {
    fn normalize(entry: &str) -> String {
        entry
            .trim()
            .trim_matches('"')
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_ascii_lowercase()
    }
    normalize(left) == normalize(right)
}

#[cfg(windows)]
fn notify_environment_changed() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    let environment = "Environment\0".encode_utf16().collect::<Vec<_>>();
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::path_with_entry;

    #[test]
    fn adding_the_launcher_directory_is_idempotent() {
        let existing = r"C:\Windows\System32;C:\Users\Ada\Programs\BlobUI\bin";
        assert_eq!(
            path_with_entry(existing, r"c:\users\ada\Programs\BlobUI\BIN\", true),
            None
        );
        assert_eq!(
            path_with_entry(r"C:\Windows\System32", r"C:\BlobUI\bin", true),
            Some(r"C:\Windows\System32;C:\BlobUI\bin".into())
        );
    }

    #[test]
    fn removing_the_launcher_directory_preserves_other_path_entries() {
        assert_eq!(
            path_with_entry(
                r#"C:\Tools;"C:\Users\Ada\Programs\BlobUI\bin";D:\Tools"#,
                r"c:\users\ada\programs\blobui\bin",
                false
            ),
            Some(r"C:\Tools;D:\Tools".into())
        );
        assert_eq!(
            path_with_entry("C:\\Tools;D:\\Tools", r"C:\BlobUI\bin", false),
            None
        );
    }
}
