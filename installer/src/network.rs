use serde_json::Value;

const OWNER: &str = "PainterFlow";
const REPOSITORY: &str = "Anytype-BlobUI";
const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_PACKAGE_BYTES: usize = 256 * 1024 * 1024;

pub fn latest_package() -> Result<(Vec<u8>, String), String> {
    let response = http_get(
        "api.github.com",
        &format!("/repos/{OWNER}/{REPOSITORY}/releases/latest"),
        "application/vnd.github+json",
        MAX_JSON_BYTES,
    )?;
    if response.status == 404 {
        return Err("No published BlobUI release was found.".into());
    }
    if response.status != 200 {
        return Err(format!(
            "GitHub returned HTTP {} while checking for BlobUI.",
            response.status
        ));
    }
    let release: Value = serde_json::from_slice(&response.body)
        .map_err(|_| "GitHub returned invalid release information.".to_string())?;
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .filter(|tag| {
            !tag.is_empty()
                && tag.len() <= 64
                && tag
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'+' | b'v'))
        })
        .ok_or_else(|| "The latest BlobUI release has an invalid version tag.".to_string())?
        .trim_start_matches('v')
        .to_string();
    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| "The latest BlobUI release has no package assets.".to_string())?;
    let url = assets
        .iter()
        .find(|asset| asset.get("name").and_then(Value::as_str) == Some("blobui-windows.zip"))
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "The latest BlobUI release does not include blobui-windows.zip.".to_string()
        })?;
    let parsed = url::Url::parse(url)
        .map_err(|_| "The BlobUI release package URL is invalid.".to_string())?;
    let path = parsed.path();
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !path.starts_with(&format!("/{OWNER}/{REPOSITORY}/releases/download/"))
        || !path.ends_with("/blobui-windows.zip")
    {
        return Err("The latest release points to an unexpected package location.".into());
    }
    let host = parsed.host_str().unwrap();
    let path = format!("{}", parsed.path());
    let package = http_get(host, &path, "application/octet-stream", MAX_PACKAGE_BYTES)?;
    if package.status != 200 {
        return Err(format!(
            "GitHub returned HTTP {} while downloading the BlobUI package.",
            package.status
        ));
    }
    Ok((package.body, tag))
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
