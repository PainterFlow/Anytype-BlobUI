use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

struct SecretBuffer(String);

impl SecretBuffer {
    fn new(value: String) -> Self { Self(value) }
}

impl std::ops::Deref for SecretBuffer {
    type Target = str;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Drop for SecretBuffer {
    fn drop(&mut self) {
        unsafe { self.0.as_mut_vec().fill(0); }
    }
}

pub fn verify_connection(api_key: &str, ip: &str, port: u16) -> Result<(), String> {
    let response = request(api_key, ip, port, "GET", "/v2/spaces?limit=1", None)?;
    if response.get("data").is_none() {
        return Err("This port did not return an Anytype API v2 spaces response.".into());
    }
    Ok(())
}

pub fn calendar_colors(api_key: &str, ip: &str, port: u16) -> Result<Value, String> {
    let spaces = paged(api_key, ip, port, "GET", "/v2/spaces?ids=full", None)?;
    if spaces.is_empty() {
        return Err("The API key cannot access any Anytype spaces.".into());
    }
    let mut colors = serde_json::Map::new();
    for space in spaces {
        let space_id = space.get("id").and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "The Anytype spaces response included a space without an ID.".to_string())?;
        let escaped_space = encode_segment(space_id);
        let property_path = format!("/v2/spaces/{escaped_space}/properties");
        let properties = paged(api_key, ip, port, "GET", &property_path, None)?;
        let matches = properties.iter().filter(|property| {
            property.get("name").and_then(Value::as_str)
                .is_some_and(|name| name.eq_ignore_ascii_case("Calendar Color"))
        }).collect::<Vec<_>>();
        if matches.is_empty() { continue; }
        if matches.len() > 1 {
            return Err(format!("The Anytype space '{space_id}' has more than one property named 'Calendar Color'; give the color property a unique name before saving."));
        }
        let key = matches[0].get("key").and_then(Value::as_str)
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| format!("The 'Calendar Color' property in Anytype space '{space_id}' has no API key."))?;
        let search_path = format!("/v2/spaces/{escaped_space}/search");
        let objects = paged(api_key, ip, port, "POST", &search_path, Some(&json!({"fields": [key]})))?;
        for object in objects {
            let Some(raw) = object_property(&object, key, "Calendar Color") else { continue; };
            let Some(value) = property_value(raw)? else { continue; };
            let Some(color) = value.as_str() else {
                if value.is_null() { continue; }
                return Err("An Anytype Calendar Color property was not text.".into());
            };
            if color.trim().is_empty() || color.trim().eq_ignore_ascii_case("default") { continue; }
            // Validate and normalize values in the CSS compiler before they are committed.
            let id = object.get("id").and_then(Value::as_str)
                .filter(|id| valid_object_id(id))
                .ok_or_else(|| format!("An object with Calendar Color in Anytype space '{space_id}' has an invalid object ID."))?;
            let normalized = canonical_calendar_color(color)?;
            crate::css::validate_calendar_color(&normalized)?;
            if let Some(previous) = colors.get(id) {
                if previous != &Value::String(normalized.clone()) {
                    return Err(format!("Anytype object '{id}' has conflicting Calendar Color values across accessible spaces."));
                }
            } else {
                colors.insert(id.to_string(), Value::String(normalized));
            }
        }
    }
    Ok(Value::Object(colors))
}

fn canonical_calendar_color(value: &str) -> Result<String, String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    let palette_variables = [
        "var(--color-grey)", "var(--color-yellow)",
        "var(--color-amber, var(--color-orange))", "var(--color-red)",
        "var(--color-pink)", "var(--color-purple)", "var(--color-blue)",
        "var(--color-sky, var(--color-ice))", "var(--color-teal)",
        "var(--color-green, var(--color-lime))",
    ];
    let canonical = match lower.as_str() {
        "grey" => "Grey", "yellow" => "Yellow", "amber" => "Amber", "red" => "Red",
        "pink" => "Pink", "purple" => "Purple", "blue" => "Blue", "sky" => "Sky",
        "teal" => "Teal", "green" => "Green", "blob pink" => "Blob Pink",
        _ if value.starts_with('#') => {
            let digits = &value[1..];
            if (digits.len() == 3 || digits.len() == 6) && digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Ok(value.to_ascii_uppercase());
            }
            return Err("Anytype returned an unsupported Calendar Color value.".into());
        }
        _ if lower.starts_with("rgb(") || lower.starts_with("rgba(") => return Ok(value.into()),
        _ if palette_variables.iter().any(|candidate| value == *candidate) => return Ok(value.into()),
        _ => return Err("Anytype returned an unsupported Calendar Color value.".into()),
    };
    Ok(canonical.into())
}

fn valid_object_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn object_property<'a>(object: &'a Value, key: &str, display_name: &str) -> Option<&'a Value> {
    let properties = object.get("properties")?;
    if let Some(map) = properties.as_object() {
        if let Some(value) = map.get(key).or_else(|| map.get(display_name)) { return Some(value); }
    }
    let rows = properties.as_array()?;
    rows.iter().find(|property| property.get("key").and_then(Value::as_str) == Some(key))
        .or_else(|| rows.iter().find(|property| property.get("name").and_then(Value::as_str) == Some(display_name)))
}

fn property_value(raw: &Value) -> Result<Option<Value>, String> {
    if raw.is_string() || raw.is_number() || raw.is_boolean() { return Ok(Some(raw.clone())); }
    let Some(map) = raw.as_object() else { return Ok(None); };
    let format = map.get("format").and_then(Value::as_str).unwrap_or_default();
    if !format.is_empty() {
        if let Some(value) = map.get(format) {
            if format == "select" {
                return value.get("name").cloned().map(Some)
                    .or_else(|| value.as_str().map(|text| Value::String(text.to_string())).map(Some))
                    .ok_or_else(|| "The 'Calendar Color' select value has no option name.".into());
            }
            return Ok(Some(value.clone()));
        }
    }
    for candidate in ["text", "select", "number", "checkbox", "date", "url", "email", "phone", "files"] {
        if let Some(value) = map.get(candidate) {
            if candidate == "select" {
                return value.get("name").cloned().map(Some)
                    .or_else(|| value.as_str().map(|text| Value::String(text.to_string())).map(Some))
                    .ok_or_else(|| "The 'Calendar Color' select value has no option name.".into());
            }
            return Ok(Some(value.clone()));
        }
    }
    Err("The 'Calendar Color' property value has no supported typed value.".into())
}

fn paged(api_key: &str, ip: &str, port: u16, method: &str, path: &str, body: Option<&Value>) -> Result<Vec<Value>, String> {
    let mut offset = 0usize;
    let mut all = Vec::new();
    loop {
        let separator = if path.contains('?') { '&' } else { '?' };
        let page_path = format!("{path}{separator}limit=100&offset={offset}");
        let response = request(api_key, ip, port, method, &page_path, body)?;
        let items = response.get("data").and_then(Value::as_array)
            .ok_or_else(|| "Anytype API v2 returned a paged response without data.".to_string())?;
        let has_more = response.get("has_more").and_then(Value::as_bool)
            .ok_or_else(|| "Anytype API v2 returned a paged response without the required has_more field.".to_string())?;
        if has_more && items.is_empty() {
            return Err("Anytype returned an empty page while more results were reported.".into());
        }
        all.extend(items.iter().cloned());
        if !has_more { break; }
        offset = offset.checked_add(items.len()).ok_or_else(|| "Anytype returned too many paged results.".to_string())?;
        if offset > 1_000_000 { return Err("Anytype returned too many paged results.".into()); }
    }
    Ok(all)
}

pub(crate) fn request(api_key: &str, ip: &str, port: u16, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
    let address = ip.parse::<IpAddr>().map_err(|_| "API key IP must be a valid IPv4 or IPv6 address.".to_string())?;
    let socket = SocketAddr::new(address, port);
    let mut stream = TcpStream::connect_timeout(&socket, TIMEOUT).map_err(|error| network_error(&error))?;
    stream.set_read_timeout(Some(TIMEOUT)).map_err(|_| "Could not configure the Anytype API connection timeout.".to_string())?;
    stream.set_write_timeout(Some(TIMEOUT)).map_err(|_| "Could not configure the Anytype API connection timeout.".to_string())?;

    let payload = body.map(serde_json::to_vec).transpose()
        .map_err(|_| "Could not encode an Anytype API request.".to_string())?;
    let mut request = SecretBuffer::new(format!("{method} {path} HTTP/1.1\r\nHost: {socket}\r\nAuthorization: Bearer {api_key}\r\nAccept: application/json\r\nConnection: close\r\n"));
    if let Some(payload) = &payload {
        request.0.push_str("Content-Type: application/json\r\n");
        request.0.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    request.0.push_str("\r\n");
    stream.write_all(request.as_bytes()).map_err(|error| network_error(&error))?;
    if let Some(payload) = payload { stream.write_all(&payload).map_err(|error| network_error(&error))?; }

    let mut response = Vec::new();
    stream.take((MAX_RESPONSE_BYTES + 64 * 1024) as u64).read_to_end(&mut response)
        .map_err(|error| network_error(&error))?;
    let split = response.windows(4).position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Anytype returned an invalid HTTP response.".to_string())?;
    let header = std::str::from_utf8(&response[..split])
        .map_err(|_| "Anytype returned an invalid HTTP response.".to_string())?;
    let status = header.lines().next().and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "Anytype returned an invalid HTTP response.".to_string())?;
    let headers = header.lines().skip(1).filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let mut response_body = &response[split + 4..];
    if headers.iter().any(|(name, value)| name == "transfer-encoding" && value.contains("chunked")) {
        let decoded = decode_chunked(response_body)?;
        return parse_json_status(status, &decoded);
    }
    if let Some((_, length)) = headers.iter().find(|(name, _)| name == "content-length") {
        let length = length.parse::<usize>().map_err(|_| "Anytype returned an invalid HTTP response.".to_string())?;
        if length > MAX_RESPONSE_BYTES { return Err("Anytype returned a response that is too large.".into()); }
        if response_body.len() < length { return Err("Anytype returned an incomplete HTTP response.".into()); }
        response_body = &response_body[..length];
    }
    parse_json_status(status, response_body)
}

fn parse_json_status(status: u16, body: &[u8]) -> Result<Value, String> {
    if !(200..300).contains(&status) { return Err(http_error(status)); }
    if body.len() > MAX_RESPONSE_BYTES { return Err("Anytype returned a response that is too large.".into()); }
    serde_json::from_slice(body).map_err(|_| "Anytype returned invalid JSON from its local API.".into())
}

fn http_error(status: u16) -> String {
    match status {
        400 => "Anytype rejected the API v2 request (HTTP 400). Check the connection settings.".into(),
        401 => "Anytype rejected this key (HTTP 401). Paste a current API v2 key.".into(),
        403 => "Anytype denied API access (HTTP 403). Give this key read access to the spaces you want to use.".into(),
        404 => "This port did not provide Anytype API v2. Check the port and your Anytype version.".into(),
        _ => format!("Anytype returned HTTP {status} while reading local API data."),
    }
}

fn network_error(error: &std::io::Error) -> String {
    if matches!(error.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
        "The request to Anytype Local API timed out. Confirm Anytype is open and Local API is enabled.".into()
    } else {
        "Could not reach Anytype Local API. Confirm Anytype is open and Local API is enabled.".into()
    }
}

pub(crate) fn encode_segment(value: &str) -> String {
    let mut escaped = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            escaped.push(char::from(byte));
        } else {
            escaped.push_str(&format!("%{byte:02X}"));
        }
    }
    escaped
}

fn decode_chunked(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoded = Vec::new();
    loop {
        let line_end = body.windows(2).position(|window| window == b"\r\n")
            .ok_or_else(|| "Anytype returned an invalid chunked HTTP response.".to_string())?;
        let size_text = std::str::from_utf8(&body[..line_end]).ok()
            .and_then(|line| line.split(';').next())
            .ok_or_else(|| "Anytype returned an invalid chunked HTTP response.".to_string())?;
        let size = usize::from_str_radix(size_text.trim(), 16)
            .map_err(|_| "Anytype returned an invalid chunked HTTP response.".to_string())?;
        body = &body[line_end + 2..];
        if size == 0 { break; }
        if size > MAX_RESPONSE_BYTES || decoded.len().saturating_add(size) > MAX_RESPONSE_BYTES || body.len() < size + 2 {
            return Err("Anytype returned an invalid or oversized chunked HTTP response.".into());
        }
        decoded.extend_from_slice(&body[..size]);
        if &body[size..size + 2] != b"\r\n" { return Err("Anytype returned an invalid chunked HTTP response.".into()); }
        body = &body[size + 2..];
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut chunk = [0u8; 2048];
        let (header_end, content_length) = loop {
            let count = stream.read(&mut chunk).unwrap();
            assert_ne!(count, 0, "client closed before sending request headers");
            request.extend_from_slice(&chunk[..count]);
            if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..end]);
                let content_length = header.lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                break (end + 4, content_length);
            }
        };
        while request.len() < header_end + content_length {
            let count = stream.read(&mut chunk).unwrap();
            assert_ne!(count, 0, "client closed before sending request body");
            request.extend_from_slice(&chunk[..count]);
        }
        request
    }

    fn serve(responses: Vec<Value>, expected_requests: usize) -> (u16, thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            for index in 0..expected_requests {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                let text = String::from_utf8_lossy(&request);
                assert!(text.contains("Authorization: Bearer test-key"));
                if index == 2 { assert!(text.starts_with("POST /v2/spaces/space-1/search?")); }
                let body = serde_json::to_vec(&responses[index]).unwrap();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        (port, handle)
    }

    #[test]
    fn authenticates_and_reads_calendar_colors_without_a_shell() {
        let pages = vec![
            json!({"data": [{"id":"space-1"}], "has_more": false}),
            json!({"data": [{"name":"Calendar Color", "key":"calendar-color"}], "has_more": false}),
            json!({"data": [{"id":"event-1", "properties":{"calendar-color":{"format":"select", "select":{"name":"Green"}}}}], "has_more": false}),
        ];
        let (port, server) = serve(pages, 3);
        let colors = calendar_colors("test-key", "127.0.0.1", port).unwrap();
        assert_eq!(colors["event-1"], "Green");
        server.join().unwrap();
    }

    #[test]
    fn fixed_http_errors_never_include_response_bodies_or_keys() {
        assert!(http_error(401).contains("401"));
        assert!(!http_error(401).contains("test-key"));
        assert_eq!(encode_segment("a/b c"), "a%2Fb%20c");
        assert_eq!(decode_chunked(b"4\r\ntest\r\n0\r\n\r\n").unwrap(), b"test");
    }
}
