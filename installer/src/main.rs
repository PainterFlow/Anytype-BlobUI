mod archive;
mod network;
#[path = "../../settings-app/src/css.rs"]
mod css;
#[path = "../../settings-app/src/storage.rs"]
mod storage;
mod install;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const RELEASE_FILE: &str = "blobui-release.json";

struct Options {
    no_pause: bool,
    anytype: Option<PathBuf>,
    app: Option<PathBuf>,
    #[cfg(feature = "development")]
    package: Option<PathBuf>,
}

fn main() {
    let no_pause = match parse_args() {
        Ok(options) => {
            let no_pause = options.no_pause;
            if let Err(error) = run(options) {
                eprintln!("BlobUI installation failed: {error}");
                if !no_pause { pause(); }
                std::process::exit(1);
            }
            no_pause
        }
        Err(error) => { eprintln!("{error}"); std::process::exit(2); }
    };
    println!("BlobUI installation completed.");
    if !no_pause { pause(); }
}

fn parse_args() -> Result<Options, String> {
    let mut no_pause = false;
    let mut anytype = None;
    let mut app = None;
    #[cfg(feature = "development")]
    let mut package = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-pause" => no_pause = true,
            "--anytype-directory" => anytype = Some(PathBuf::from(args.next().ok_or("Missing value for --anytype-directory")?)),
            "--app-directory" => app = Some(PathBuf::from(args.next().ok_or("Missing value for --app-directory")?)),
            #[cfg(feature = "development")]
            "--package" => package = Some(PathBuf::from(args.next().ok_or("Missing value for --package")?)),
            "--help" | "-h" => return Err(usage().into()),
            _ => return Err(format!("Unknown argument: {arg}\n{}", usage())),
        }
    }
    Ok(Options { no_pause, anytype, app, #[cfg(feature = "development")] package })
}

fn usage() -> &'static str {
    #[cfg(feature = "development")]
    { "Usage: blobui-installer-dev.exe [--no-pause] [--package ZIP] [--anytype-directory DIR] [--app-directory DIR]" }
    #[cfg(not(feature = "development"))]
    { "Usage: blobui-installer.exe [--no-pause] [--anytype-directory DIR] [--app-directory DIR]" }
}

fn run(options: Options) -> Result<(), String> {
    let exe_dir = env::current_exe().map_err(|e| format!("Could not locate the installer: {e}"))?.parent().ok_or("Could not locate the installer folder")?.to_path_buf();
    #[cfg(feature = "development")]
    let test_root = exe_dir.join("BlobUI-Installer-Test");
    #[cfg(feature = "development")]
    {
        if options.anytype.is_some() || options.app.is_some() { return Err("Development installer paths are fixed inside BlobUI-Installer-Test.".into()); }
        validate_no_links(&test_root)?;
        fs::create_dir_all(&test_root).map_err(|e| format!("Could not create development test folder: {e}"))?;
        fs::create_dir_all(test_root.join("Anytype")).map_err(|e| format!("Could not create development Anytype folder: {e}"))?;
    }

    let (anytype, app, shortcut) = if cfg!(feature = "development") {
        let root = exe_dir.join("BlobUI-Installer-Test");
        (root.join("Anytype"), root.join(format!("BlobUI-{}", unique_nonce())), false)
    } else {
        let anytype_default = env::var_os("APPDATA").map(PathBuf::from).map(|p| p.join("anytype")).unwrap_or_default();
        let app_default = env::var_os("LOCALAPPDATA").map(PathBuf::from).map(|p| p.join("Programs").join("BlobUI")).unwrap_or_default();
        (resolve_path(options.anytype, anytype_default, "Anytype data folder (contains custom.css)")?, resolve_path(options.app, app_default, "Install BlobUI app in")?, true)
    };

    #[cfg(feature = "development")]
    let (package_bytes, expected_version) = if let Some(path) = options.package {
        (fs::read(&path).map_err(|e| format!("Could not read development package {}: {e}", path.display()))?, None)
    } else { let (bytes, version) = network::latest_package()?; (bytes, Some(version)) };
    #[cfg(not(feature = "development"))]
    let (package_bytes, expected_version) = { let (bytes, version) = network::latest_package()?; (bytes, Some(version)) };

    #[cfg(feature = "development")]
    let stage = staging_path(&exe_dir.join("BlobUI-Installer-Test"));
    #[cfg(not(feature = "development"))]
    let stage = staging_path(&env::temp_dir());
    validate_no_links(&stage)?;
    archive::extract_zip(&package_bytes, &stage)?;
    let result = (|| {
        let version = read_release_version(&stage)?;
        if expected_version.as_deref().is_some_and(|expected| expected != version) { return Err("The downloaded package version does not match the published release tag.".into()); }
        if !anytype.is_dir() { return Err(format!("The Anytype data folder does not exist: {}. Open Anytype once or select its custom data folder.", anytype.display())); }
        validate_destinations(&anytype, &app, &stage, cfg!(feature = "development"))?;
        install::install(&stage, &anytype, &app, &version, shortcut)?;
        Ok(version)
    })();
    let _ = fs::remove_dir_all(&stage);
    let version = result?;
    println!("Installed BlobUI {version}.");
    #[cfg(feature = "development")]
    println!("Development data is kept under {}.", exe_dir.join("BlobUI-Installer-Test").display());
    Ok(())
}

fn resolve_path(value: Option<PathBuf>, default: PathBuf, prompt: &str) -> Result<PathBuf, String> {
    if let Some(path) = value { return Ok(path); }
    print!("{prompt} [{}]: ", default.display());
    io::stdout().flush().map_err(|e| format!("Could not display installer prompt: {e}"))?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| format!("Could not read installer input: {e}"))?;
    let input = input.trim().trim_matches('"');
    Ok(if input.is_empty() { default } else { PathBuf::from(input) })
}

fn read_release_version(root: &Path) -> Result<String, String> {
    let path = root.join(RELEASE_FILE);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).map_err(|_| format!("The package is missing {RELEASE_FILE}."))?).map_err(|_| "The package release manifest is invalid JSON.".to_string())?;
    if value.get("formatVersion").and_then(serde_json::Value::as_u64) != Some(1) { return Err("The package release manifest has an unsupported format.".into()); }
    let version = value.get("version").and_then(serde_json::Value::as_str).ok_or("The package release version is missing")?;
    if version.len() > 64 || !version.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'+')) || !version.as_bytes().first().is_some_and(u8::is_ascii_digit) { return Err("The package release version is invalid.".into()); }
    for required in ["Dev", "assets", "settings-app/dist/blobui-settings.exe"] { if !root.join(required).exists() { return Err(format!("The package is missing required content: {required}.")); } }
    Ok(version.to_string())
}

fn validate_destinations(anytype: &Path, app: &Path, stage: &Path, development: bool) -> Result<(), String> {
    validate_no_links(anytype)?;
    validate_no_links(app)?;
    let anytype = absolute(anytype)?;
    let app = absolute(app)?;
    let stage = absolute(stage)?;
    if overlaps(&app, &anytype) || overlaps(&app, &stage) { return Err("Choose an app folder separate from Anytype data and installer staging.".into()); }
    for path in [&anytype, &app] {
        if path.components().any(|part| part.as_os_str().to_string_lossy().eq_ignore_ascii_case("Releases")) { return Err("Installation inside a Releases folder is not supported.".into()); }
    }
    if development {
        // Use the same path representation for existing and not-yet-created
        // folders; Windows canonicalize adds a verbatim prefix to only the former.
        let expected = env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("Could not resolve test folder")?.join("BlobUI-Installer-Test");
        let app = absolute(&app)?;
        if !anytype.starts_with(&expected) || !app.starts_with(&expected) { return Err("Development installs must stay inside BlobUI-Installer-Test.".into()); }
    }
    Ok(())
}
fn absolute(path: &Path) -> Result<PathBuf, String> { if path.is_absolute() { Ok(path.to_path_buf()) } else { env::current_dir().map(|cwd| cwd.join(path)).map_err(|e| e.to_string()) } }
fn overlaps(a: &Path, b: &Path) -> bool { a == b || a.starts_with(b) || b.starts_with(a) }
fn staging_path(root: &Path) -> PathBuf { let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(); root.join(format!(".bi-{}-{nonce:x}", std::process::id())) }
fn unique_nonce() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() }
fn validate_no_links(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;
    let absolute = absolute(path)?;
    let mut cursor = PathBuf::new();
    for component in absolute.components() {
        cursor.push(component.as_os_str());
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Err(format!("Installer paths cannot pass through a link: {}", cursor.display())),
            #[cfg(windows)]
            Ok(metadata) if metadata.file_attributes() & 0x400 != 0 => return Err(format!("Installer paths cannot pass through a reparse point: {}", cursor.display())),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect installer path {}: {error}", cursor.display())),
        }
    }
    Ok(())
}
fn pause() { print!("Press Enter to close..."); let _ = io::stdout().flush(); let mut line = String::new(); let _ = io::stdin().read_line(&mut line); }
