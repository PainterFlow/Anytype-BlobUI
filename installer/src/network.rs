use serde_json::Value;
use sha2::{Digest, Sha256};

const OWNER: &str = "PainterFlow";
const REPOSITORY: &str = "Anytype-BlobUI";
const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_PACKAGE_BYTES: usize = 256 * 1024 * 1024;

pub fn latest_package() -> Result<(Vec<u8>, String), String> {
    let release = fetch_release("latest")?;
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "The latest BlobUI release has an invalid version tag.".to_string())?;
    package_from_release(&release, tag)
}

pub fn package_for_tag(tag: &str) -> Result<(Vec<u8>, String), String> {
    validate_tag(tag)?;
    let release = fetch_release(&format!("tags/{tag}"))?;
    package_from_release(&release, tag)
}

fn fetch_release(selector: &str) -> Result<Value, String> {
    let response = http_get(
        "api.github.com",
        &format!("/repos/{OWNER}/{REPOSITORY}/releases/{selector}"),
        "application/vnd.github+json",
        MAX_JSON_BYTES,
    )?;
    if response.status == 404 {
        return Err("The requested published BlobUI release was not found.".into());
    }
    if response.status != 200 {
        return Err(format!(
            "GitHub returned HTTP {} while checking for BlobUI.",
            response.status
        ));
    }
    serde_json::from_slice(&response.body)
        .map_err(|_| "GitHub returned invalid release information.".to_string())
}

fn package_from_release(release: &Value, expected_tag: &str) -> Result<(Vec<u8>, String), String> {
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "The BlobUI release has an invalid version tag.".to_string())?;
    if tag != expected_tag {
        return Err("GitHub returned a release with a different version tag.".into());
    }
    let version = validate_tag(tag)?;
    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| "The BlobUI release has no package assets.".to_string())?;
    let mut matches = assets
        .iter()
        .filter(|asset| asset.get("name").and_then(Value::as_str) == Some("blobui-windows.zip"));
    let asset = matches
        .next()
        .ok_or_else(|| "The BlobUI release does not include blobui-windows.zip.".to_string())?;
    if matches.next().is_some() {
        return Err("The BlobUI release contains duplicate package assets.".into());
    }
    let url = asset
        .get("browser_download_url")
        .and_then(Value::as_str)
        .ok_or_else(|| "The BlobUI release package URL is missing.".to_string())?;
    validate_asset_url(url, tag, "blobui-windows.zip")?;
    let size = asset
        .get("size")
        .and_then(Value::as_u64)
        .filter(|size| *size > 0 && *size <= MAX_PACKAGE_BYTES as u64)
        .ok_or_else(|| "The BlobUI release package has an invalid size.".to_string())?;
    let digest = parse_digest(asset.get("digest"))?;
    let parsed = url::Url::parse(url)
        .map_err(|_| "The BlobUI release package URL is invalid.".to_string())?;
    let response = http_get(
        parsed.host_str().expect("validated URL host"),
        parsed.path(),
        "application/octet-stream",
        MAX_PACKAGE_BYTES,
    )?;
    if response.status != 200 {
        return Err(format!(
            "GitHub returned HTTP {} while downloading the BlobUI package.",
            response.status
        ));
    }
    verify_asset(&response.body, size, digest.as_deref())?;
    Ok((response.body, version))
}

fn validate_tag(tag: &str) -> Result<String, String> {
    if tag.is_empty()
        || tag.len() > 64
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err("The BlobUI release has an unsafe version tag.".into());
    }
    let version = tag.strip_prefix('v').unwrap_or(tag);
    semver::Version::parse(version)
        .map(|version| version.to_string())
        .map_err(|_| "The BlobUI release tag is not a supported semantic version.".into())
}

fn validate_asset_url(url: &str, tag: &str, name: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url)
        .map_err(|_| "The BlobUI release package URL is invalid.".to_string())?;
    let expected_path = format!("/{OWNER}/{REPOSITORY}/releases/download/{tag}/{name}");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some_and(|port| port != 443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != expected_path
    {
        return Err("The BlobUI release points to an unexpected package location.".into());
    }
    Ok(())
}

fn parse_digest(value: Option<&Value>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(value) = value.as_str() else {
        return Err("The BlobUI release package digest is invalid.".into());
    };
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err("The BlobUI release package uses an unsupported digest.".into());
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("The BlobUI release package digest is invalid.".into());
    }
    Ok(Some(hex.to_ascii_lowercase()))
}

fn verify_asset(
    bytes: &[u8],
    expected_size: u64,
    expected_digest: Option<&str>,
) -> Result<(), String> {
    if bytes.len() as u64 != expected_size {
        return Err(
            "The downloaded BlobUI package size does not match GitHub's release record.".into(),
        );
    }
    if let Some(expected) = expected_digest {
        let digest = Sha256::digest(bytes);
        let actual = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != expected {
            return Err("The downloaded BlobUI package failed its SHA-256 check.".into());
        }
    }
    Ok(())
}

struct HttpResponse {
    status: u32,
    body: Vec<u8>,
}

#[cfg(windows)]
fn http_get(
    host: &str,
    path: &str,
    accept: &str,
    max_bytes: usize,
) -> Result<HttpResponse, String> {
    use std::ptr;
    use windows_sys::Win32::Networking::WinHttp::*;
    struct Handle(*mut std::ffi::c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    WinHttpCloseHandle(self.0);
                }
            }
        }
    }
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    let user_agent = wide("BlobUI Native Installer");
    let host_wide = wide(host);
    let path_wide = wide(path);
    let get = wide("GET");
    let headers = wide(&format!(
        "Accept: {accept}\r\nUser-Agent: BlobUI-Installer\r\n"
    ));
    let session = Handle(unsafe {
        WinHttpOpen(
            user_agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            ptr::null(),
            ptr::null(),
            0,
        )
    });
    if session.0.is_null() {
        return Err("Windows could not start the BlobUI download connection.".into());
    }
    if unsafe { WinHttpSetTimeouts(session.0, 15_000, 15_000, 15_000, 45_000) } == 0 {
        return Err("Windows could not set download timeouts.".into());
    }
    let connection = Handle(unsafe { WinHttpConnect(session.0, host_wide.as_ptr(), 443, 0) });
    if connection.0.is_null() {
        return Err("Windows could not connect to GitHub.".into());
    }
    let request = Handle(unsafe {
        WinHttpOpenRequest(
            connection.0,
            get.as_ptr(),
            path_wide.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    });
    if request.0.is_null() {
        return Err("Windows could not prepare the GitHub request.".into());
    }
    if unsafe {
        WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            u32::MAX,
            ptr::null_mut(),
            0,
            0,
            0,
        )
    } == 0
        || unsafe { WinHttpReceiveResponse(request.0, ptr::null_mut()) } == 0
    {
        return Err("Could not reach GitHub to download BlobUI.".into());
    }
    let mut status = 0u32;
    let mut status_bytes = std::mem::size_of::<u32>() as u32;
    if unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut status_bytes,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err("GitHub did not return an HTTP status.".into());
    }
    let mut body = Vec::new();
    loop {
        let mut available = 0u32;
        if unsafe { WinHttpQueryDataAvailable(request.0, &mut available) } == 0 {
            return Err("GitHub's response could not be read.".into());
        }
        if available == 0 {
            break;
        }
        if body.len().saturating_add(available as usize) > max_bytes {
            return Err("GitHub's response exceeds the supported size limit.".into());
        }
        let start = body.len();
        body.resize(start + available as usize, 0);
        let mut read = 0u32;
        if unsafe {
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr().cast(),
                available,
                &mut read,
            )
        } == 0
        {
            return Err("GitHub's response could not be read.".into());
        }
        body.truncate(start + read as usize);
    }
    Ok(HttpResponse { status, body })
}

#[cfg(not(windows))]
fn http_get(_: &str, _: &str, _: &str, _: usize) -> Result<HttpResponse, String> {
    Err("The BlobUI installer is supported on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tags_are_semantic_versions_and_asset_urls_are_exact() {
        assert_eq!(validate_tag("v1.2.3").unwrap(), "1.2.3");
        assert!(validate_tag("../latest").is_err());
        assert!(validate_tag("1.2").is_err());
        assert!(validate_asset_url(
            "https://github.com/PainterFlow/Anytype-BlobUI/releases/download/v1.2.3/blobui-windows.zip",
            "v1.2.3",
            "blobui-windows.zip"
        )
        .is_ok());
        for url in [
            "https://github.com.evil/PainterFlow/Anytype-BlobUI/releases/download/v1.2.3/blobui-windows.zip",
            "https://github.com/PainterFlow/Anytype-BlobUI/releases/download/v1.2.2/blobui-windows.zip",
            "http://github.com/PainterFlow/Anytype-BlobUI/releases/download/v1.2.3/blobui-windows.zip",
            "https://github.com/PainterFlow/Anytype-BlobUI/releases/download/v1.2.3/other.zip",
        ] {
            assert!(validate_asset_url(url, "v1.2.3", "blobui-windows.zip").is_err());
        }
    }

    #[test]
    fn package_size_and_optional_digest_are_checked() {
        let bytes = b"package contents";
        let expected = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        verify_asset(bytes, bytes.len() as u64, Some(&expected)).unwrap();
        assert!(verify_asset(bytes, (bytes.len() + 1) as u64, Some(&expected)).is_err());
        assert!(verify_asset(bytes, bytes.len() as u64, Some(&"0".repeat(64))).is_err());
        assert!(parse_digest(Some(&Value::String(format!("sha256:{expected}")))).is_ok());
        assert!(parse_digest(Some(&Value::String("md5:abc".into()))).is_err());
    }
}
