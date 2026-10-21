include!("../scripts/build-version.rs");

fn main() {
    emit_build_version();
    tauri_build::build()
}
