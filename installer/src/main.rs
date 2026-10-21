mod archive;
// These shared modules also contain code used only by the Settings app.
#[allow(dead_code)]
#[path = "../../settings-app/src/css.rs"]
mod css;
mod install;
mod transaction;
mod network;
#[allow(dead_code)]
#[path = "../../settings-app/src/storage.rs"]
mod storage;

use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const RELEASE_FILE: &str = "blobui-release.json";

struct Options {
    no_pause: bool,
    anytype: Option<PathBuf>,
    app: Option<PathBuf>,
    update_mode: bool,
    uninstall: bool,
    release_tag: Option<String>,
    wait_pid: Option<u32>,
    validate_package: Option<PathBuf>,
    #[cfg(feature = "development")]
    package: Option<PathBuf>,
}

fn main() {
    println!("blobui-app {}", env!("BLOBUI_BUILD_VERSION"));
    let no_pause = match parse_args() {
        Ok(options) => {
            let no_pause = options.no_pause;
            let uninstall = options.uninstall;
            if let Err(error) = run(options) {
                let action = if uninstall {
                    "uninstallation"
                } else {
                    "installation"
                };
                eprintln!("BlobUI {action} failed: {error}");
                if !no_pause {
                    pause();
                }
                std::process::exit(1);
            }
            no_pause
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if env::args().any(|arg| arg == "--uninstall") {
        println!("BlobUI uninstallation completed.");
    } else if env::args().any(|arg| arg == "--validate-package") {
        println!("BlobUI release package validated.");
    } else {
        println!("BlobUI installation completed.");
    }
    if !no_pause {
        pause();
    }
}

fn parse_args() -> Result<Options, String> {
    let mut no_pause = false;
    let mut anytype = None;
    let mut app = None;
    let mut update_mode = false;
    let mut uninstall = false;
    let mut release_tag = None;
    let mut wait_pid = None;
    let mut validate_package = None;
    #[cfg(feature = "development")]
    let mut package = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-pause" => no_pause = true,
            "--update-mode" => update_mode = true,
            "--uninstall" => uninstall = true,
            "--release-tag" => {
                release_tag = Some(args.next().ok_or("Missing value for --release-tag")?)
            }
            "--wait-pid" => {
                let value = args.next().ok_or("Missing value for --wait-pid")?;
                wait_pid = Some(
                    value
                        .parse()
                        .map_err(|_| "The Settings process id is invalid.")?,
                );
            }
            "--validate-package" => {
                validate_package = Some(PathBuf::from(
                    args.next().ok_or("Missing value for --validate-package")?,
                ))
            }
            "--anytype-directory" => {
                anytype = Some(PathBuf::from(
                    args.next().ok_or("Missing value for --anytype-directory")?,
                ))
            }
            "--app-directory" => {
                app = Some(PathBuf::from(
                    args.next().ok_or("Missing value for --app-directory")?,
                ))
            }
            #[cfg(feature = "development")]
            "--package" => {
                package = Some(PathBuf::from(
                    args.next().ok_or("Missing value for --package")?,
                ))
            }
            "--help" | "-h" => return Err(usage().into()),
            _ => return Err(format!("Unknown argument: {arg}\n{}", usage())),
        }
    }
    Ok(Options {
        no_pause,
        anytype,
        app,
        update_mode,
        uninstall,
        release_tag,
        wait_pid,
        validate_package,
        #[cfg(feature = "development")]
        package,
    })
}

fn usage() -> &'static str {
    #[cfg(feature = "development")]
    {
        "Usage: blobui-installer-dev.exe [--no-pause] [--package ZIP] [--anytype-directory DIR] [--app-directory DIR] | --validate-package DIR | --uninstall --anytype-directory DIR --app-directory DIR"
    }
    #[cfg(not(feature = "development"))]
    {
        "Usage: blobui-installer.exe [--no-pause] [--anytype-directory DIR] [--app-directory DIR] | --validate-package DIR | --update-mode --release-tag TAG --wait-pid PID | --uninstall --anytype-directory DIR --app-directory DIR [--wait-pid PID]"
    }
}

fn run(options: Options) -> Result<(), String> {
    if let Some(package) = options.validate_package.as_deref() {
        if options.uninstall || options.update_mode || options.release_tag.is_some()
            || options.wait_pid.is_some() || options.anytype.is_some() || options.app.is_some()
        {
            return Err("Package validation cannot be combined with install or update options.".into());
        }
        install::validate_release_package(package)?;
        return Ok(());
    }
    if options.uninstall {
        if options.update_mode || options.release_tag.is_some() {
            return Err("Uninstall cannot be combined with update mode or a release tag.".into());
        }
        let app = absolute(&options.app.ok_or("Uninstall requires --app-directory.")?)?;
        let anytype = absolute(
            &options
                .anytype
                .ok_or("Uninstall requires --anytype-directory.")?,
        )?;
        install::validate_update_target(&app, &anytype)?;
        let app = fs::canonicalize(&app)
            .map_err(|_| "The existing BlobUI app folder could not be resolved safely.")?;
        let anytype = fs::canonicalize(&anytype)
            .map_err(|_| "The existing Anytype data folder could not be resolved safely.")?;
        if let Some(pid) = options.wait_pid {
            if pid == 0 || pid == std::process::id() {
                return Err("Invalid Settings process id.".into());
            }
            ProcessWaiter::open(pid)?.wait()?;
        } else {
            print!(
                "Remove BlobUI from {}? Type UNINSTALL to confirm: ",
                anytype.display()
            );
            io::stdout().flush().map_err(|e| e.to_string())?;
            let mut answer = String::new();
            io::stdin()
                .read_line(&mut answer)
                .map_err(|e| e.to_string())?;
            if answer.trim() != "UNINSTALL" {
                return Err("Uninstall cancelled.".into());
            }
        }
        install::uninstall(&app, &anytype)?;
        if let Err(error) = remove_startup_registration(&app) {
            eprintln!(
                "BlobUI was removed, but its per-user startup entry could not be cleared: {error}"
            );
        }
        return Ok(());
    }
    if options.update_mode {
        if cfg!(feature = "development") {
            return Err(
                "The development installer cannot update an existing app installation.".into(),
            );
        }
        return run_update_mode(options);
    }
    if options.release_tag.is_some() || options.wait_pid.is_some() {
        return Err("--release-tag and --wait-pid require --update-mode.".into());
    }
    let exe_dir = env::current_exe()
        .map_err(|e| format!("Could not locate the installer: {e}"))?
        .parent()
        .ok_or("Could not locate the installer folder")?
        .to_path_buf();
    #[cfg(feature = "development")]
    let test_root = exe_dir.join("BlobUI-Installer-Test");
    #[cfg(feature = "development")]
    {
        if options.anytype.is_some() || options.app.is_some() {
            return Err(
                "Development installer paths are fixed inside BlobUI-Installer-Test.".into(),
            );
        }
        validate_no_links(&test_root)?;
        fs::create_dir_all(&test_root)
            .map_err(|e| format!("Could not create development test folder: {e}"))?;
        fs::create_dir_all(test_root.join("Anytype"))
            .map_err(|e| format!("Could not create development Anytype folder: {e}"))?;
    }

    let (anytype, app, shortcut) = if cfg!(feature = "development") {
        let root = exe_dir.join("BlobUI-Installer-Test");
        (
            root.join("Anytype"),
            root.join(format!("BlobUI-{}", unique_nonce())),
            false,
        )
    } else {
        let anytype_default = env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("anytype"))
            .unwrap_or_default();
        let app_default = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("Programs").join("BlobUI"))
            .unwrap_or_default();
        (
            resolve_path(
                options.anytype,
                anytype_default,
                "Anytype data folder (contains custom.css)",
            )?,
            resolve_path(options.app, app_default, "Install BlobUI app in")?,
            true,
        )
    };

    #[cfg(feature = "development")]
    let (package_bytes, expected_version) = if let Some(path) = options.package {
        (
            fs::read(&path).map_err(|e| {
                format!("Could not read development package {}: {e}", path.display())
            })?,
            None,
        )
    } else {
        let (bytes, version) = network::latest_package()?;
        (bytes, Some(version))
    };
    #[cfg(not(feature = "development"))]
    let (package_bytes, expected_version) = {
        let (bytes, version) = network::latest_package()?;
        (bytes, Some(version))
    };

    #[cfg(feature = "development")]
    let stage = staging_path(&exe_dir.join("BlobUI-Installer-Test"));
    #[cfg(not(feature = "development"))]
    let stage = staging_path(&env::temp_dir());
    validate_no_links(&stage)?;
    archive::extract_zip(&package_bytes, &stage)?;
    let result = (|| {
        let version = read_release_version(&stage)?;
        if expected_version
            .as_deref()
            .is_some_and(|expected| expected != version)
        {
            return Err(
                "The downloaded package version does not match the published release tag.".into(),
            );
        }
        if !anytype.is_dir() {
            return Err(format!("The Anytype data folder does not exist: {}. Open Anytype once or select its custom data folder.", anytype.display()));
        }
        validate_destinations(&anytype, &app, &stage, cfg!(feature = "development"))?;
        let installer = env::current_exe().map_err(|e| format!("Could not locate the installer: {e}"))?;
        install::install(&stage, &anytype, &app, &version, shortcut, &installer)?;
        Ok(version)
    })();
    let _ = fs::remove_dir_all(&stage);
    let version = result?;
    println!("Installed BlobUI {version}.");
    #[cfg(feature = "development")]
    println!(
        "Development data is kept under {}.",
        exe_dir.join("BlobUI-Installer-Test").display()
    );
    Ok(())
}

fn run_update_mode(options: Options) -> Result<(), String> {
    let anytype = options
        .anytype
        .ok_or_else(|| "Update mode requires --anytype-directory.".to_string())?;
    let app = options
        .app
        .ok_or_else(|| "Update mode requires --app-directory.".to_string())?;
    let tag = options
        .release_tag
        .ok_or_else(|| "Update mode requires --release-tag.".to_string())?;
    let wait_pid = options
        .wait_pid
        .ok_or_else(|| "Update mode requires --wait-pid.".to_string())?;
    if wait_pid == std::process::id() || wait_pid == 0 {
        return Err("Update mode received an invalid Settings process id.".into());
    }

    let anytype = absolute(&anytype)?;
    let app = absolute(&app)?;
    validate_no_links(&anytype)?;
    validate_no_links(&app)?;
    install::validate_update_target(&app, &anytype)?;
    // Keep a handle to the caller before downloading. The Settings process
    // exits only after the installer has been spawned, and this handle avoids
    // a PID-reuse race while network work is in progress.
    let mut caller = ProcessWaiter::open(wait_pid)?;

    let operation = (|| {
        let (package_bytes, expected_version) = network::package_for_tag(&tag)?;
        // The extracted package must be separate from Anytype because the
        // transaction rejects overlapping source and destination trees.
        let staging_root = env::temp_dir();
        validate_no_links(&staging_root)?;
        fs::create_dir_all(&staging_root)
            .map_err(|e| format!("Could not prepare update staging: {e}"))?;
        let stage = staging_path(&staging_root);
        validate_no_links(&stage)?;
        archive::extract_zip(&package_bytes, &stage)?;
        let result = (|| {
            install_update_package(&stage, &anytype, &app, &expected_version, || caller.wait())
        })();
        let _ = fs::remove_dir_all(&stage);
        result
    })();

    match operation {
        Ok(version) => {
            let message = format!("BlobUI Settings was updated to version {version}.");
            let _ = write_update_status(Some(&version), "updated", &message);
            match launch_settings(&app, &anytype) {
                Ok(()) => Ok(()),
                Err(error) => {
                    let message = format!(
                        "BlobUI was updated to version {version}, but Settings could not be reopened: {error}"
                    );
                    let _ = write_update_status(Some(&version), "error", &message);
                    Err(message)
                }
            }
        }
        Err(error) => {
            let version = tag.strip_prefix('v').unwrap_or(&tag);
            let mut message = error;
            // Persist failure before a reopened Settings process can perform
            // its next automatic check and overwrite the user-visible state.
            let _ = write_update_status(Some(version), "error", &message);
            match caller.wait() {
                Ok(()) => {
                    if let Err(reopen_error) = launch_settings(&app, &anytype) {
                        message
                            .push_str(&format!(" Settings could not be reopened: {reopen_error}"));
                    }
                }
                Err(wait_error) => {
                    message.push_str(&format!(" Settings was left running: {wait_error}"));
                }
            }
            let _ = write_update_status(Some(version), "error", &message);
            Err(message)
        }
    }
}

fn install_update_package<F>(
    package: &Path,
    anytype: &Path,
    app: &Path,
    expected_version: &str,
    wait_for_settings: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let version = read_release_version(package)?;
    if version != expected_version {
        return Err("The downloaded package version does not match the pinned release tag.".into());
    }
    validate_destinations(anytype, app, package, false)?;
    if overlaps(&absolute(package)?, &absolute(anytype)?) {
        return Err("The extracted update package must be separate from Anytype data.".into());
    }
    wait_for_settings()?;
    let installer = env::current_exe().map_err(|e| format!("Could not locate the installer: {e}"))?;
    let installer_target = app.join("blobui-installer.exe");
    let retained_installer = (installer != installer_target).then_some(installer.as_path());
    install::install_update(package, anytype, app, &version, retained_installer)?;
    Ok(version)
}

fn startup_entry_targets_app(value: &str, app: &Path) -> bool {
    let executable = if let Some(quoted) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        quoted
    } else {
        value
    };
    if executable.contains('"') {
        return false;
    }
    let Some(executable_parent) = Path::new(executable).parent() else {
        return false;
    };
    if Path::new(executable).file_name().is_none_or(|name| {
        !name
            .to_string_lossy()
            .eq_ignore_ascii_case("blobui-settings.exe")
    }) {
        return false;
    }
    let Ok(executable_parent) = executable_parent.canonicalize() else {
        return false;
    };
    let expected_parents = [app.to_path_buf(), app.join("settings-app/dist")];
    expected_parents.iter().any(|expected_parent| {
        expected_parent
            .canonicalize()
            .is_ok_and(|expected_parent| {
                executable_parent
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&expected_parent.to_string_lossy())
            })
    })
}

#[cfg(windows)]
fn remove_startup_registration(app: &Path) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
    };

    let key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let value_name = "BlobUI Settings\0".encode_utf16().collect::<Vec<_>>();
    let mut key: HKEY = std::ptr::null_mut();
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key,
        )
    };
    if opened == ERROR_FILE_NOT_FOUND || opened == ERROR_PATH_NOT_FOUND {
        return Ok(());
    }
    if opened != ERROR_SUCCESS {
        return Err(format!(
            "Windows could not inspect the per-user startup settings: {}",
            io::Error::from_raw_os_error(opened as i32)
        ));
    }
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
    if queried == ERROR_FILE_NOT_FOUND {
        unsafe { RegCloseKey(key) };
        return Ok(());
    }
    if queried != ERROR_SUCCESS {
        unsafe { RegCloseKey(key) };
        return Err(format!(
            "Windows could not inspect the per-user startup setting: {}",
            io::Error::from_raw_os_error(queried as i32)
        ));
    }
    if value_type != REG_SZ || value_bytes % std::mem::size_of::<u16>() as u32 != 0 {
        unsafe { RegCloseKey(key) };
        return Ok(());
    }
    let mut value = vec![0u16; value_bytes as usize / std::mem::size_of::<u16>()];
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
    if queried != ERROR_SUCCESS || value_type != REG_SZ {
        unsafe { RegCloseKey(key) };
        return Err(format!(
            "Windows could not inspect the per-user startup setting: {}",
            io::Error::from_raw_os_error(queried as i32)
        ));
    }
    let decoded = String::from_utf16(
        &value[..value
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(value.len())],
    );
    let value = match decoded {
        Ok(value) => value,
        Err(_) => {
            unsafe { RegCloseKey(key) };
            return Err("The per-user startup setting is not valid text.".into());
        }
    };
    if !startup_entry_targets_app(&value, app) {
        unsafe { RegCloseKey(key) };
        return Ok(());
    }
    let deleted = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
    unsafe { RegCloseKey(key) };
    if deleted == ERROR_SUCCESS || deleted == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format!(
            "Windows could not remove the per-user startup setting: {}",
            io::Error::from_raw_os_error(deleted as i32)
        ))
    }
}

#[cfg(not(windows))]
fn remove_startup_registration(_: &Path) -> Result<(), String> {
    Ok(())
}

fn write_update_status(version: Option<&str>, status: &str, message: &str) -> Result<(), String> {
    let app_data = env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "The current user's roaming app-data folder is unavailable.".to_string())?;
    let path = app_data.join("BlobUI/updater.json");
    validate_no_links(&path)?;
    let mut preferences = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
            .map_err(|_| "The saved updater preferences are invalid.".to_string())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => json!({
            "installDirectory": null,
            "automaticUpdatesEnabled": false,
            "automaticChecksEnabled": true,
            "lastCheckedAt": null,
            "latestVersion": null,
            "lastCheckStatus": null,
            "lastMessage": null
        }),
        Err(error) => return Err(format!("Could not read updater preferences: {error}")),
    };
    let object = preferences
        .as_object_mut()
        .ok_or_else(|| "The saved updater preferences are not a JSON object.".to_string())?;
    if let Some(version) = version {
        object.insert("latestVersion".into(), Value::String(version.to_string()));
    }
    object.insert("lastCheckStatus".into(), Value::String(status.to_string()));
    object.insert("lastMessage".into(), Value::String(message.to_string()));
    if status == "updated" {
        if let Some(version) = version {
            object.insert(
                "pendingChangelogVersion".into(),
                Value::String(version.to_string()),
            );
        }
    }
    if status == "error" {
        if let Some(version) = version {
            object.insert("lastFailedInstallVersion".into(), Value::String(version.to_string()));
        }
    } else if status == "updated" {
        object.remove("lastFailedInstallVersion");
    }
    let mut bytes = serde_json::to_vec_pretty(&preferences)
        .map_err(|_| "Could not encode updater preferences.".to_string())?;
    bytes.push(b'\n');
    storage::write_atomic(&path, &bytes)
}

fn launch_settings(app: &Path, anytype: &Path) -> Result<(), String> {
    let executable = app.join("settings-app/dist/blobui-settings.exe");
    validate_no_links(&executable)?;
    if !executable.is_file() {
        return Err("The installed BlobUI Settings executable is missing.".into());
    }
    launch_settings_executable(&executable, anytype)
}

#[cfg(windows)]
fn launch_settings_executable(executable: &Path, anytype: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::thread;
    use std::time::Duration;
    let mut child = Command::new(executable)
        .arg("--install-directory")
        .arg(anytype)
        .creation_flags(0x0800_0000)
        .spawn()
        .map_err(|e| format!("Could not start BlobUI Settings: {e}"))?;
    thread::sleep(Duration::from_millis(750));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| format!("Could not check the BlobUI Settings launch: {e}"))?
    {
        return Err(format!(
            "BlobUI Settings exited during startup with status {status}."
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn launch_settings_executable(_: &Path, _: &Path) -> Result<(), String> {
    Err("Reopening BlobUI Settings requires Windows.".into())
}

#[cfg(windows)]
struct ProcessWaiter {
    handle: windows_sys::Win32::Foundation::HANDLE,
    result: Option<Result<(), String>>,
}

#[cfg(windows)]
impl ProcessWaiter {
    fn open(pid: u32) -> Result<Self, String> {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_INVALID_PARAMETER};
        use windows_sys::Win32::System::Threading::OpenProcess;
        // SYNCHRONIZE is the standard process wait right (0x0010_0000).
        let handle = unsafe { OpenProcess(0x0010_0000, 0, pid) };
        if handle.is_null() {
            if unsafe { GetLastError() } == ERROR_INVALID_PARAMETER {
                return Ok(Self {
                    handle: std::ptr::null_mut(),
                    result: Some(Ok(())),
                });
            }
            return Err("Windows could not open the BlobUI Settings process safely.".into());
        }
        Ok(Self {
            handle,
            result: None,
        })
    }

    fn wait(&mut self) -> Result<(), String> {
        if let Some(result) = &self.result {
            return result.clone();
        }
        use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        let code = unsafe { WaitForSingleObject(self.handle, 120_000) };
        let result = if code == WAIT_OBJECT_0 {
            Ok(())
        } else if code == WAIT_TIMEOUT {
            Err("BlobUI Settings did not close in time; the update was cancelled.".into())
        } else {
            Err("Windows could not confirm that BlobUI Settings had closed.".into())
        };
        self.result = Some(result.clone());
        result
    }
}

#[cfg(windows)]
impl Drop for ProcessWaiter {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle) };
        }
    }
}

#[cfg(not(windows))]
struct ProcessWaiter;

#[cfg(not(windows))]
impl ProcessWaiter {
    fn open(_: u32) -> Result<Self, String> {
        Err("The BlobUI installer is supported on Windows.".into())
    }
    fn wait(&mut self) -> Result<(), String> {
        Err("The BlobUI installer is supported on Windows.".into())
    }
}

fn resolve_path(value: Option<PathBuf>, default: PathBuf, prompt: &str) -> Result<PathBuf, String> {
    if let Some(path) = value {
        return Ok(path);
    }
    print!("{prompt} [{}]: ", default.display());
    io::stdout()
        .flush()
        .map_err(|e| format!("Could not display installer prompt: {e}"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Could not read installer input: {e}"))?;
    let input = input.trim().trim_matches('"');
    Ok(if input.is_empty() {
        default
    } else {
        PathBuf::from(input)
    })
}

fn read_release_version(root: &Path) -> Result<String, String> {
    let path = root.join(RELEASE_FILE);
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(&path).map_err(|_| format!("The package is missing {RELEASE_FILE}."))?,
    )
    .map_err(|_| "The package release manifest is invalid JSON.".to_string())?;
    if value
        .get("formatVersion")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err("The package release manifest has an unsupported format.".into());
    }
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or("The package release version is missing")?;
    if version.len() > 64
        || !version
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'+'))
        || !version.as_bytes().first().is_some_and(u8::is_ascii_digit)
    {
        return Err("The package release version is invalid.".into());
    }
    for required in [
        "Dev",
        "assets",
        "settings-app/dist/blobui-settings.exe",
        "Changelogs.md",
        "Changelogs.yaml",
    ] {
        if !root.join(required).exists() {
            return Err(format!(
                "The package is missing required content: {required}."
            ));
        }
    }
    Ok(version.to_string())
}

fn validate_destinations(
    anytype: &Path,
    app: &Path,
    stage: &Path,
    development: bool,
) -> Result<(), String> {
    validate_no_links(anytype)?;
    validate_no_links(app)?;
    let anytype = absolute(anytype)?;
    let app = absolute(app)?;
    let stage = absolute(stage)?;
    if overlaps(&app, &anytype) || overlaps(&app, &stage) {
        return Err(
            "Choose an app folder separate from Anytype data and installer staging.".into(),
        );
    }
    for path in [&anytype, &app] {
        if path.components().any(|part| {
            part.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case("Releases")
        }) {
            return Err("Installation inside a Releases folder is not supported.".into());
        }
    }
    if development {
        // Use the same path representation for existing and not-yet-created
        // folders; Windows canonicalize adds a verbatim prefix to only the former.
        let expected = env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("Could not resolve test folder")?
            .join("BlobUI-Installer-Test");
        let app = absolute(&app)?;
        if !anytype.starts_with(&expected) || !app.starts_with(&expected) {
            return Err("Development installs must stay inside BlobUI-Installer-Test.".into());
        }
    }
    Ok(())
}
fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|e| e.to_string())
    }
}
fn overlaps(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}
fn staging_path(root: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    root.join(format!(".bi-{}-{nonce:x}", std::process::id()))
}
fn unique_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
fn validate_no_links(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;
    let absolute = absolute(path)?;
    let mut cursor = PathBuf::new();
    for component in absolute.components() {
        cursor.push(component.as_os_str());
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "Installer paths cannot pass through a link: {}",
                    cursor.display()
                ))
            }
            #[cfg(windows)]
            Ok(metadata) if metadata.file_attributes() & 0x400 != 0 => {
                return Err(format!(
                    "Installer paths cannot pass through a reparse point: {}",
                    cursor.display()
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Could not inspect installer path {}: {error}",
                    cursor.display()
                ))
            }
        }
    }
    Ok(())
}
fn pause() {
    print!("Press Enter to close...");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_cleanup_matches_only_this_app_executable() {
        let root = fixture("startup-ownership");
        let app = root.join("BlobUI app");
        let expected_dir = app.join("settings-app/dist");
        let other_dir = root.join("Another app/settings-app/dist");
        fs::create_dir_all(&expected_dir).unwrap();
        fs::create_dir_all(&other_dir).unwrap();
        let expected = expected_dir.join("blobui-settings.exe");
        let other = other_dir.join("blobui-settings.exe");

        assert!(startup_entry_targets_app(
            &format!("\"{}\"", expected.display()),
            &app
        ));
        assert!(!startup_entry_targets_app(
            &format!("\"{}\"", other.display()),
            &app
        ));
        assert!(!startup_entry_targets_app(
            &format!("\"{}\" --background", expected.display()),
            &app
        ));
        fs::remove_dir_all(root).unwrap();
    }

    fn fixture(label: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run")
            .join(format!(
                "installer-handoff-{label}-{}-{}",
                std::process::id(),
                unique_nonce()
            ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn package(root: &Path) -> PathBuf {
        let package = root.join("package");
        fs::create_dir_all(package.join("Dev")).unwrap();
        fs::create_dir_all(package.join("assets/spacebanner")).unwrap();
        fs::create_dir_all(package.join("settings-app/dist")).unwrap();
        fs::write(
            package.join("Dev/theme.css"),
            b".new-theme { color: purple; }",
        )
        .unwrap();
        fs::write(
            package.join("assets/spacebanner/banner.png"),
            b"release banner",
        )
        .unwrap();
        fs::write(
            package.join("settings-app/dist/blobui-settings.exe"),
            b"new settings executable",
        )
        .unwrap();
        fs::write(
            package.join("blobui-release.json"),
            br#"{"formatVersion":1,"version":"1.2.0"}"#,
        )
        .unwrap();
        fs::write(package.join("Changelogs.md"), "# Example release notes\n").unwrap();
        fs::write(package.join("Changelogs.yaml"), "images: []\n").unwrap();
        package
    }

    fn installed_fixture(root: &Path) -> (PathBuf, PathBuf) {
        let anytype = root.join("Anytype data");
        let blob = anytype.join("blob");
        let app = root.join("BlobUI app");
        fs::create_dir_all(blob.join("runtime/Dev")).unwrap();
        fs::create_dir_all(blob.join("runtime/assets/spacebanner")).unwrap();
        fs::create_dir_all(blob.join("assets/spacebanner")).unwrap();
        fs::create_dir_all(blob.join("credentials")).unwrap();
        fs::create_dir_all(app.join("settings-app/dist")).unwrap();
        fs::create_dir_all(app.join("Dev")).unwrap();
        fs::write(blob.join("data.json"), serde_json::to_vec(&json!({
            "installDirectory": anytype,
            "settings": {"fontFamily":"Inter","codeFontFamily":"Consolas","accentColor":"#2678B2","bannerImage":"","reducedMotion":false,"tabHoverEffect":false,"linkHoverEffect":false,"objectMentionHoverEffect":false},
            "apiConnection": {"enabled":false}, "calendarObjectColors":{}, "futureField":{"keep":true}
        })).unwrap()).unwrap();
        fs::write(
            blob.join("credentials/anytype-api-key.dpapi"),
            b"credential",
        )
        .unwrap();
        fs::write(blob.join("assets/spacebanner/banner.png"), b"custom banner").unwrap();
        fs::write(anytype.join("custom.css"), b"old css").unwrap();
        fs::write(
            blob.join("runtime/Dev/local.css"),
            b".local { color: teal; }",
        )
        .unwrap();
        fs::write(blob.join("runtime/Dev/old.css"), b"managed old").unwrap();
        fs::write(
            blob.join("runtime/settings-app.exe"),
            b"old runtime executable",
        )
        .unwrap();
        fs::write(app.join("blobui-settings.exe"), b"old root executable").unwrap();
        fs::write(
            app.join("settings-app/dist/blobui-settings.exe"),
            b"old nested executable",
        )
        .unwrap();
        fs::write(app.join("Dev/theme.css"), b"old theme").unwrap();
        fs::write(
            app.join(".blobui-install.json"),
            serde_json::to_vec(&json!({"installDirectory":anytype})).unwrap(),
        )
        .unwrap();
        fs::write(app.join(".blobui-app-managed-files.json"), serde_json::to_vec(&json!({"manifestVersion":1,"releaseVersion":"1.1.0","managedFiles":["Dev/theme.css","blobui-settings.exe","settings-app/dist/blobui-settings.exe"]})).unwrap()).unwrap();
        fs::write(blob.join(".blobui-managed-files.json"), serde_json::to_vec(&json!({"manifestVersion":1,"releaseVersion":"1.1.0","managedFiles":["runtime/Dev/old.css","runtime/settings-app.exe"]})).unwrap()).unwrap();
        (anytype, app)
    }

    #[test]
    fn update_handoff_stages_outside_anytype_waits_then_replaces_installed_app() {
        let root = fixture("success");
        let package = package(&root);
        let (anytype, app) = installed_fixture(&root);
        let mut waited = false;
        let version = install_update_package(&package, &anytype, &app, "1.2.0", || {
            waited = true;
            Ok(())
        })
        .unwrap();
        assert_eq!(version, "1.2.0");
        assert!(waited);
        assert_eq!(
            fs::read(app.join("settings-app/dist/blobui-settings.exe")).unwrap(),
            b"new settings executable"
        );
        assert_eq!(
            fs::read(anytype.join("blob/runtime/settings-app.exe")).unwrap(),
            b"new settings executable"
        );
        assert!(fs::read_to_string(app.join("Dev/theme.css"))
            .unwrap()
            .contains("new-theme"));
        assert_eq!(
            fs::read(anytype.join("blob/credentials/anytype-api-key.dpapi")).unwrap(),
            b"credential"
        );
        assert_eq!(
            fs::read(anytype.join("blob/assets/spacebanner/banner.png")).unwrap(),
            b"custom banner"
        );
        let compiled = fs::read_to_string(anytype.join("custom.css")).unwrap();
        assert!(compiled.contains("new-theme"));
        assert!(compiled.contains(".local"));
        assert!(!anytype.join("blob/runtime/Dev/old.css").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_handoff_rejects_package_staged_inside_anytype_before_waiting() {
        let root = fixture("overlap");
        let (anytype, app) = installed_fixture(&root);
        let package = package(&anytype.join("blob/staging").to_path_buf());
        let mut waited = false;
        let error = install_update_package(&package, &anytype, &app, "1.2.0", || {
            waited = true;
            Ok(())
        })
        .unwrap_err();
        assert!(error.contains("separate from Anytype"));
        assert!(!waited);
        assert_eq!(
            fs::read(app.join("settings-app/dist/blobui-settings.exe")).unwrap(),
            b"old nested executable"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_status_preserves_preferences_and_records_or_clears_failed_release() {
        let root = fixture("status");
        let previous = env::var_os("APPDATA");
        env::set_var("APPDATA", root.join("roaming"));
        fs::create_dir_all(root.join("roaming")).unwrap();
        let path = root.join("roaming/BlobUI/updater.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            br#"{"automaticUpdatesEnabled":true,"automaticChecksEnabled":true,"futurePreference":"keep"}"#,
        )
        .unwrap();

        write_update_status(Some("1.2.0"), "error", "install failed").unwrap();
        let failed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(failed["automaticUpdatesEnabled"], true);
        assert_eq!(failed["futurePreference"], "keep");
        assert_eq!(failed["lastFailedInstallVersion"], "1.2.0");

        write_update_status(Some("1.2.0"), "updated", "installed").unwrap();
        let updated: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(updated.get("lastFailedInstallVersion").is_none());
        assert_eq!(updated["automaticChecksEnabled"], true);
        if let Some(value) = previous {
            env::set_var("APPDATA", value);
        } else {
            env::remove_var("APPDATA");
        }
        fs::remove_dir_all(root).unwrap();
    }
}

