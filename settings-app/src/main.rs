#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod mcp_launcher;

fn main() {
    #[cfg(windows)]
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        blobui_mcp::serve_bridge_stdio();
        return;
    }

    #[cfg(windows)]
    if let Err(error) = mcp_launcher::ensure_launcher_is_on_path() {
        eprintln!("Could not register the BlobUI MCP launcher: {error}");
    }

    blobui_settings::app_main();
}
