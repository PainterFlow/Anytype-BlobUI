pub fn emit_build_version() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    println!("cargo:rerun-if-changed={}", root.join(".git/HEAD").display());
    println!("cargo:rerun-if-changed={}", root.join(".git/refs").display());
    println!("cargo:rerun-if-changed={}", root.join(".git/packed-refs").display());
    println!("cargo:rerun-if-env-changed=BLOBUI_BUILD_CHANNEL");
    if std::env::var_os("CARGO_FEATURE_HEADLESS_CLI").is_some() {
        let channel = std::env::var("BLOBUI_BUILD_CHANNEL").unwrap_or_else(|_| "main".into());
        let metadata_path = root.join("blobui-headless-release.json");
        println!("cargo:rerun-if-changed={}", metadata_path.display());
        let metadata: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&metadata_path).expect("Headless release metadata is required"),
        )
        .expect("Headless release metadata must be valid JSON");
        assert_eq!(metadata["formatVersion"].as_u64(), Some(1), "Unsupported Headless release metadata format");
        let key = match channel.as_str() {
            "main" => "version",
            "dev" => "devVersion",
            _ => panic!("BLOBUI_BUILD_CHANNEL must be main or dev"),
        };
        let version = metadata[key]
            .as_str()
            .filter(|version| !version.is_empty())
            .expect("Headless release metadata has no version for the selected channel");
        let version_body = if channel == "dev" {
            version.strip_prefix("dev-").expect("Headless dev version must use the dev-X.Y.Z format")
        } else {
            version
        };
        let parts = version_body.split('.').collect::<Vec<_>>();
        assert!(parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())), "Headless release versions must use X.Y.Z format");
        if channel == "dev" {
            assert!(version.starts_with("dev-"), "Headless dev version must use the dev-X.Y.Z format");
        }
        println!("cargo:rustc-env=BLOBUI_BUILD_VERSION={version}");
        println!("cargo:rustc-env=BLOBUI_BUILD_CHANNEL={channel}");
        return;
    }
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root).output().expect("Git is required to identify this build");
    assert!(output.status.success(), "Could not read the local Git commit");
    let commit = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=BLOBUI_BUILD_VERSION=Dev+{}", commit.trim());
}
