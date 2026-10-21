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
        // CryptProtectData already returns the complete DPAPI blob that
        // ConvertFrom-SecureString serializes as hexadecimal. Do not add a
        // second copy of its header.
        let encoded = protected.iter().map(|byte| format!("{byte:02X}")).collect::<String>();
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
    // ConvertFrom-SecureString stores the complete DPAPI blob as hexadecimal.
    // The plaintext is UTF-16LE; decode it directly without launching PowerShell.
    let mut bytes = decode_dpapi_blob(encrypted)?;
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| error())?,
        pbData: bytes.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let mut succeeded = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        // Try the complete blob first so a legitimate DPAPI blob is never
        // shortened. Older Rust builds stored a duplicate leading header;
        // retry with one copy removed only when the full blob cannot decrypt.
        if succeeded == 0 && strip_duplicate_dpapi_header(&mut bytes) {
            input.cbData = u32::try_from(bytes.len()).map_err(|_| error())?;
            input.pbData = bytes.as_mut_ptr();
            output = CRYPT_INTEGER_BLOB::default();
            succeeded = CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            );
        }
        if succeeded == 0 {
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

#[cfg(any(windows, test))]
const DPAPI_BLOB_HEADER: [u8; 20] = [
    0x01, 0x00, 0x00, 0x00, 0xD0, 0x8C, 0x9D, 0xDF, 0x01, 0x15, 0xD1, 0x11, 0x8C, 0x7A,
    0x00, 0xC0, 0x4F, 0xC2, 0x97, 0xEB,
];

#[cfg(any(windows, test))]
fn decode_dpapi_blob(encrypted: &str) -> Result<Vec<u8>, String> {
    let error = || "Could not decrypt the saved API key for this Windows account.".to_string();
    if encrypted.is_empty() || encrypted.len() % 2 != 0 || !encrypted.is_ascii() {
        return Err(error());
    }
    let bytes = (0..encrypted.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&encrypted[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error())?;

    Ok(bytes)
}

#[cfg(any(windows, test))]
fn strip_duplicate_dpapi_header(bytes: &mut Vec<u8>) -> bool {
    let header_len = DPAPI_BLOB_HEADER.len();
    if bytes.len() >= header_len * 2
        && bytes.starts_with(&DPAPI_BLOB_HEADER)
        && bytes[header_len..].starts_with(&DPAPI_BLOB_HEADER)
    {
        bytes.drain(..header_len);
        true
    } else {
        false
    }
}

#[cfg(not(windows))]
pub fn decrypt_saved_key(_: &str) -> Result<String, String> {
    Err("Encrypted API key storage requires Windows DPAPI.".into())
}

#[cfg(test)]
mod tests {
    use super::{decode_dpapi_blob, strip_duplicate_dpapi_header, DPAPI_BLOB_HEADER};

    #[test]
    fn saved_credentials_keep_native_header_and_recognize_old_duplicate() {
        let encode_hex = |bytes: &[u8]| {
            bytes.iter().map(|byte| format!("{byte:02X}")).collect::<String>()
        };
        let mut native_blob = DPAPI_BLOB_HEADER.to_vec();
        native_blob.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0xAA, 0xBB]);

        let decoded_native = decode_dpapi_blob(&encode_hex(&native_blob)).unwrap();
        assert_eq!(decoded_native, native_blob);
        let mut native_candidate = decoded_native.clone();
        assert!(!strip_duplicate_dpapi_header(&mut native_candidate));
        assert_eq!(native_candidate, native_blob);

        let mut doubled_blob = DPAPI_BLOB_HEADER.to_vec();
        doubled_blob.extend_from_slice(&native_blob);
        let mut decoded_doubled = decode_dpapi_blob(&encode_hex(&doubled_blob)).unwrap();
        assert!(strip_duplicate_dpapi_header(&mut decoded_doubled));
        assert_eq!(decoded_doubled, native_blob);
    }
}
