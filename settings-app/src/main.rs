#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    #[cfg(windows)]
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        blobui_mcp::serve_bridge_stdio();
        return;
    }
    blobui_settings::app_main();
}
