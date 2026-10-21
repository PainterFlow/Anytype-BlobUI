pub fn emit_build_version() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    println!("cargo:rerun-if-changed={}", root.join(".git/HEAD").display());
    println!("cargo:rerun-if-changed={}", root.join(".git/refs").display());
    println!("cargo:rerun-if-changed={}", root.join(".git/packed-refs").display());
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root).output().expect("Git is required to identify this build");
    assert!(output.status.success(), "Could not read the local Git commit");
    let commit = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=BLOBUI_BUILD_VERSION=Dev+{}", commit.trim());
}
