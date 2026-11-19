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
    let _ = mcp_launcher::ensure_installed_launcher_is_on_path();

    blobui_settings::app_main();
}
