//! Private bridge credential. This module never reads an Anytype API key.
use std::path::PathBuf;
pub fn path() -> Result<PathBuf, String> {
    if let Some(instance) = std::env::var_os("ANYBLOB_BRIDGE_INSTANCE") {
        if instance != "default" {
            return std::env::var_os("ANYBLOB_CREDENTIAL_FILE")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .ok_or("An isolated bridge requires an explicit credential file.".into());
        }
    }
    if cfg!(test) {
        return Err("Tests require an isolated credential file.".into());
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("BlobUI/Anyblob/bridge.dpapi"))
        .ok_or("Windows local application data is unavailable.".into())
}
pub fn reject_links(path: &std::path::Path) -> Result<(), String> {
    for part in path.ancestors() {
        match std::fs::symlink_metadata(part) {
            Ok(m) => {
                #[cfg(windows)]
                let link = {
                    use std::os::windows::fs::MetadataExt;
                    m.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let link = m.file_type().is_symlink();
                if link {
                    return Err("Anyblob cannot use a linked destination.".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Anyblob could not inspect its destination.".into()),
        }
    }
    Ok(())
}
#[cfg(windows)]
fn crypt(bytes: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    let mut input_bytes = bytes.to_vec();
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_bytes.len() as u32,
        pbData: input_bytes.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let ok = if protect {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        input_bytes
            .iter_mut()
            .for_each(|v| std::ptr::write_volatile(v, 0));
        if ok == 0 {
            return Err("Windows could not access the private Anyblob credential.".into());
        }
        let data = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = data.to_vec();
        data.iter_mut().for_each(|v| std::ptr::write_volatile(v, 0));
        LocalFree(output.pbData.cast());
        Ok(result)
    }
}
#[cfg(not(windows))]
fn crypt(_: &[u8], _: bool) -> Result<Vec<u8>, String> {
    Err("Anyblob credential storage requires Windows DPAPI.".into())
}
pub fn encrypt(token: &str) -> Result<Vec<u8>, String> {
    crypt(token.as_bytes(), true)
}
pub fn read() -> Result<String, String> {
    if std::env::var_os("ANYBLOB_REMOTE_ADDRESS").is_some() {
        let token = std::env::var("ANYBLOB_REMOTE_TOKEN")
            .map_err(|_| "Set ANYBLOB_REMOTE_TOKEN for the HTTPS Cloudflare Tunnel connection.")?;
        if token.len() != 64 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Invalid remote Anyblob credential.".into());
        }
        return Ok(token);
    }
    read_at(&path()?)
}
pub fn read_at(path: &std::path::Path) -> Result<String, String> {
    reject_links(path)?;
    let data = std::fs::read(path)
        .map_err(|_| "Enable Anyblob in BlobUI to create its private credential.")?;
    let token =
        String::from_utf8(crypt(&data, false)?).map_err(|_| "Invalid Anyblob credential.")?;
    if token.len() != 64 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid Anyblob credential.".into());
    }
    Ok(token)
}
