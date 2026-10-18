#[cfg(windows)]
pub fn encrypt_api_key(api_key: &str) -> Result<String, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let error = || "Windows could not encrypt the Anytype API key for this account.".to_string();
    if api_key.is_empty() || api_key.len() > 4096 || api_key.chars().any(char::is_control) {
        return Err("Enter a valid Anytype API v2 key.".into());
    }
    let mut plaintext = api_key.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(plaintext.len()).map_err(|_| error())?,
        pbData: plaintext.as_mut_ptr(),
    };
    let description: Vec<u16> = "BlobUI Anytype API key\0".encode_utf16().collect();
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if CryptProtectData(
            &input,
            description.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        ) == 0
        {
            for byte in &mut plaintext { std::ptr::write_volatile(byte, 0); }
            return Err(error());
        }
        for byte in &mut plaintext { std::ptr::write_volatile(byte, 0); }
        let protected = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        // ConvertFrom-SecureString uses this 20-byte serialization header
        // before the current-user DPAPI blob. Keep the existing on-disk format
        // so the legacy installer tooling and the Rust app can read each
        // other's credentials.
        const POWERSHELL_DPAPI_HEADER: &str = "01000000D08C9DDF0115D1118C7A00C04FC297EB";
        let encoded = format!("{POWERSHELL_DPAPI_HEADER}{}", protected.iter().map(|byte| format!("{byte:02X}")).collect::<String>());
        for byte in protected { std::ptr::write_volatile(byte, 0); }
        LocalFree(output.pbData.cast());
        Ok(encoded)
    }
}

#[cfg(not(windows))]
pub fn encrypt_api_key(_: &str) -> Result<String, String> {
    Err("Encrypted API key storage requires Windows DPAPI.".into())
}

#[cfg(windows)]
pub fn decrypt_saved_key(encrypted: &str) -> Result<String, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let error = || "Could not decrypt the saved API key for this Windows account.".to_string();
    // ConvertFrom-SecureString stores DPAPI bytes as hexadecimal. Its plaintext
    // is UTF-16LE. Read that format directly without launching PowerShell.
    if encrypted.is_empty() || encrypted.len() % 2 != 0 || !encrypted.is_ascii() {
        return Err(error());
    }
    const POWERSHELL_DPAPI_HEADER: &str = "01000000D08C9DDF0115D1118C7A00C04FC297EB";
    let blob_text = encrypted.strip_prefix(POWERSHELL_DPAPI_HEADER)
        .or_else(|| encrypted.strip_prefix("01000000d08c9ddf0115d1118c7a00c04fc297eb"))
        .unwrap_or(encrypted);
    if blob_text.is_empty() || blob_text.len() % 2 != 0 {
        return Err(error());
    }
    let mut bytes = (0..blob_text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&blob_text[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error())?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| error())?,
        pbData: bytes.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        ) == 0
        {
            return Err(error());
        }
        let plaintext = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = if plaintext.len() % 2 == 0 {
            let mut units = plaintext
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            let decoded = String::from_utf16(&units).map_err(|_| error());
            for unit in &mut units {
                std::ptr::write_volatile(unit, 0);
            }
            decoded
        } else {
            Err(error())
        };
        for byte in plaintext {
            std::ptr::write_volatile(byte, 0);
        }
        LocalFree(output.pbData.cast());
        result
    }
}

#[cfg(not(windows))]
pub fn decrypt_saved_key(_: &str) -> Result<String, String> {
    Err("Encrypted API key storage requires Windows DPAPI.".into())
}
