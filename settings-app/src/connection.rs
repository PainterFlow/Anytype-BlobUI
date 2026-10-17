//! Only fixed error codes cross the process boundary; raw output may contain keys.
pub fn connection_error(output: &[u8], port: u16) -> String {
    let text = String::from_utf8_lossy(output);
    let code = text
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("BLOBUI_CONNECTION_ERROR="));
    match code {
        Some("unauthorized") => "Anytype rejected this key (HTTP 401). Paste a current API v2 key.".into(),
        Some("forbidden") => "Anytype denied API access (HTTP 403). Give this key read access to the spaces you want to use.".into(),
        Some("api-version") => "This port did not provide Anytype API v2. Check the port and your Anytype version.".into(),
        Some("connection") => format!("Could not reach Anytype's Local API on port {port}. Keep Anytype open and enable Local API on this port."),
        Some("credential") => "The API connection succeeded, but Windows could not encrypt the key for your account.".into(),
        Some("settings") => "BlobUI could not prepare the saved settings. Your connection settings were not changed. Check your local theme settings.".into(),
        Some("compile") => "BlobUI could not compile CSS. Your connection settings were not changed. Check your font and accent settings.".into(),
        Some("install") => "BlobUI could not save the CSS or encrypted key. Check permissions on your Anytype data folder.".into(),
        Some("setup") => "BlobUI could not load its installation. Check the Anytype data folder and installed runtime scripts.".into(),
        _ => "Connection setup failed before it returned a diagnostic. Rebuild the Settings app and update its runtime scripts, then try again.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_only_fixed_codes_and_never_echoes_process_output() {
        let secret = "test-secret-key";
        let output = format!("Authorization: Bearer {secret}\nBLOBUI_CONNECTION_ERROR=compile\n");
        let error = connection_error(output.as_bytes(), 31010);
        assert!(error.contains("CSS"));
        assert!(!error.contains(secret));
        assert!(!error.contains("Authorization"));
        let unknown = format!("BLOBUI_CONNECTION_ERROR={secret}");
        assert!(!connection_error(unknown.as_bytes(), 31010).contains(secret));
    }

    #[test]
    fn distinguishes_credentials_network_and_local_failures() {
        assert!(connection_error(b"BLOBUI_CONNECTION_ERROR=unauthorized", 31009).contains("401"));
        assert!(connection_error(b"BLOBUI_CONNECTION_ERROR=connection", 31010).contains("31010"));
        assert!(connection_error(b"BLOBUI_CONNECTION_ERROR=credential", 31009).contains("encrypt"));
        assert!(connection_error(b"BLOBUI_CONNECTION_ERROR=install", 31009).contains("permissions"));
    }
}
