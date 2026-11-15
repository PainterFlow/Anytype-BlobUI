//! Create and read the stable Anyblob identity for this MCP host.
use std::path::Path;

const HOST_ID_ERROR: &str = "Could not read or create the Anyblob host ID.";

pub fn host_id_at(home: &Path) -> Result<String, String> {
    static HOST_ID_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = HOST_ID_LOCK.lock().map_err(|_| HOST_ID_ERROR.to_string())?;
    let path = home.join("anyblob-host-id");
    match read_host_id(&path) {
        Ok(id) => return Ok(id),
        Err(HostIdReadError::NotFound) => {}
        Err(HostIdReadError::Invalid) => return Err(HOST_ID_ERROR.into()),
    }

    let mut bytes = [0u8; 16];
    random_bytes(&mut bytes)?;
    let id = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    bytes.fill(0);

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            use std::io::Write;
            let result = file.write_all(id.as_bytes()).and_then(|_| file.sync_all());
            if result.is_err() {
                drop(file);
                let _ = std::fs::remove_file(&path);
                Err(HOST_ID_ERROR.into())
            } else {
                Ok(id)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            for _ in 0..20 {
                if let Ok(id) = read_host_id(&path) {
                    return Ok(id);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(HOST_ID_ERROR.into())
        }
        Err(_) => Err(HOST_ID_ERROR.into()),
    }
}

enum HostIdReadError {
    NotFound,
    Invalid,
}

fn read_host_id(path: &Path) -> Result<String, HostIdReadError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(HostIdReadError::NotFound)
        }
        Err(_) => return Err(HostIdReadError::Invalid),
    };
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked || !metadata.is_file() {
        return Err(HostIdReadError::Invalid);
    }
    let id = std::fs::read_to_string(path).map_err(|_| HostIdReadError::Invalid)?;
    let id = id.trim();
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(HostIdReadError::Invalid);
    }
    Ok(id.into())
}

#[cfg(windows)]
fn random_bytes(bytes: &mut [u8]) -> Result<(), String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    let ok = unsafe { SystemFunction036(bytes.as_mut_ptr(), bytes.len() as u32) };
    if ok == 0 {
        Err("Windows could not generate the Anyblob host ID.".into())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn random_bytes(bytes: &mut [u8]) -> Result<(), String> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(bytes))
        .map_err(|_| "The operating system could not generate the Anyblob host ID.".into())
}

#[cfg(not(any(windows, unix)))]
fn random_bytes(_: &mut [u8]) -> Result<(), String> {
    Err("This platform cannot generate the Anyblob host ID.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_a_stable_host_id_and_rejects_bad_files() {
        let home = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.test-run/mcp-host-id")
            .join(std::process::id().to_string());
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let first = host_id_at(&home).unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(host_id_at(&home).unwrap(), first);
        std::fs::write(home.join("anyblob-host-id"), "invalid id").unwrap();
        assert!(host_id_at(&home).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }
}
