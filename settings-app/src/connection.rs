//! Only fixed error codes cross the process boundary; raw output may contain keys.
pub fn connection_error(output: &[u8], port: u16) -> String {
    let text = String::from_utf8_lossy(output);
    let code = text
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("BLOBUI_CONNECTION_ERROR="));
    let (code, native_error) = code
        .map(|value| value.split_once(':').unwrap_or((value, "0")))
        .unwrap_or(("", "0"));
    let rollback_failed = code.ends_with("-rollback-failed");
    let code = code.strip_suffix("-rollback-failed").unwrap_or(code);
    let message: String = match code {
        "unauthorized" => "Anytype rejected this key (HTTP 401). Paste a current API v2 key.".into(),
        "forbidden" => "Anytype denied API access (HTTP 403). Give this key read access to the spaces you want to use.".into(),
        "api-version" => "This port did not provide Anytype API v2. Check the port and your Anytype version.".into(),
        "connection" => format!("Could not reach Anytype's Local API on port {port}. Keep Anytype open and enable Local API on this port."),
        "credential" => "The API connection succeeded, but Windows could not encrypt the key for your account.".into(),
        "settings" => "BlobUI could not prepare the saved settings. Your connection settings were not changed. Check your local theme settings.".into(),
        "compile" => "BlobUI could not compile CSS. Your connection settings were not changed. Check your font and accent settings.".into(),
        "install" => "BlobUI could not finish the save. Update the source runtime scripts to get the detailed save diagnostic.".into(),
        "install-prepare" => "BlobUI could not prepare its installed files. Check that the source folder includes scripts, Dev, and assets.".into(),
        "install-inspect" => "BlobUI could not read an existing installed file or its manifest before saving.".into(),
        "install-assets" => "BlobUI could not save the theme assets under blob/assets.".into(),
        "install-runtime" => "BlobUI unexpectedly tried to update runtime files during this save. Update the Rebuild.ps1 and core module together.".into(),
        "install-css" => "BlobUI could not replace custom.css in the selected Anytype data folder.".into(),
        "install-settings" => "BlobUI could not replace blob/data.json. The connection was not saved.".into(),
        "install-credential" => "BlobUI could not replace blob/credentials/anytype-api-key.dpapi. The new key was not saved.".into(),
        "install-credential-permissions" => "Windows could not apply private access permissions to the encrypted key file.".into(),
        "install-manifest" => "BlobUI could not save blob/.blobui-managed-files.json.".into(),
        "install-rollback" => "The save failed and BlobUI could not restore every previous file. Check the installed files before retrying.".into(),
        "setup" => "BlobUI could not load its installation. Check the Anytype data folder and installed runtime scripts.".into(),
        _ => "Connection setup failed before it returned a diagnostic. Rebuild the Settings app and update its runtime scripts, then try again.".into(),
    };
    let reason = if code.starts_with("install-") {
        match native_error {
            "5" => " Windows reported access denied.",
            "19" => " Windows reported that the destination is write-protected.",
            "32" | "33" => " Windows reported that another process is holding the file open.",
            "112" => " Windows reported that the disk is full.",
            _ => "",
        }
    } else {
        ""
    };
    let recovery = if rollback_failed {
        " BlobUI also could not finish restoring the previous files."
    } else {
        ""
    };
    format!("{message}{reason}{recovery}")
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
        assert!(
            connection_error(b"BLOBUI_CONNECTION_ERROR=install-css:32", 31009)
                .contains("holding the file open")
        );
        assert!(connection_error(
            b"BLOBUI_CONNECTION_ERROR=install-credential-permissions:5",
            31009
        )
        .contains("access denied"));
    }
    #[test]
    fn rollback_failure_preserves_the_original_save_operation() {
        let error = connection_error(
            b"BLOBUI_CONNECTION_ERROR=install-credential-rollback-failed:5",
            31009,
        );
        assert!(error.contains("anytype-api-key.dpapi"));
        assert!(error.contains("access denied"));
        assert!(error.contains("restoring"));
    }
}
