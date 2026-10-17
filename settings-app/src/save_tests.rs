//! Exercise the same Rust -> PowerShell -> transaction path as the Save command.
//! All fixtures stay in the repository and use no live Anytype data or API key.
#![cfg(windows)]

use super::*;
use std::ffi::OsString;

struct EnvironmentGuard(Vec<(&'static str, Option<OsString>)>);

impl EnvironmentGuard {
    fn set(values: &[(&'static str, &Path)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, value)| {
                let previous = env::var_os(key);
                env::set_var(key, value);
                (*key, previous)
            })
            .collect();
        Self(previous)
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}

#[test]
fn native_save_migrates_rebuilds_and_preserves_files_on_failure() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let fixture_parent = source.join(".test-run");
    let fixture = fixture_parent.join(format!(
        "native-save-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let blob = fixture.join("blob");
    fs::create_dir_all(&blob).unwrap();
    let _environment = EnvironmentGuard::set(&[
        ("BLOBUI_INSTALL_DIRECTORY", &fixture),
        ("BLOBUI_SOURCE_DIRECTORY", &source),
    ]);
    let data_path = blob.join("data.json");
    let css_path = fixture.join("custom.css");
    let original_data = json!({
        "schemaVersion": 1,
        "installDirectory": fixture,
        "settings": {
            "fontFamily": "Libron",
            "codeFontFamily": "Consolas",
            "accentColor": "#DE50DE",
            "bannerPath": "spacebanner\\banner.png"
        },
        "calendarObjectColors": { "fixture-event": "Green" }
    });
    fs::write(
        &data_path,
        serde_json::to_vec_pretty(&original_data).unwrap(),
    )
    .unwrap();
    fs::write(&css_path, "previous stylesheet").unwrap();
    fs::write(fixture.join("unrelated.txt"), "keep this file").unwrap();

    let draft = SettingsDraft {
        font_family: "Inter".into(),
        code_font_family: "Cascadia Code".into(),
        accent_color: "#27A879".into(),
        accent_mode: "custom".into(),
        accent_preset: String::new(),
    };
    let result = save_settings_blocking(draft.clone()).expect("first native Save must rebuild");
    assert_eq!(result.calendar_status, "cached");
    let saved_data = read_json_file(&data_path).unwrap();
    assert_eq!(saved_data["settingsV2"]["version"], 2);
    assert_eq!(
        saved_data["settingsV2"]["settings"]["accentColor"],
        "#27A879"
    );
    assert_eq!(saved_data["settings"]["fontFamily"], "Inter");
    assert_eq!(saved_data["calendarObjectColors"]["fixture-event"], "Green");
    let saved_css = fs::read_to_string(&css_path).unwrap();
    assert!(saved_css.contains("#27A879"));
    assert!(saved_css.contains("Cascadia Code"));
    assert!(saved_css.contains("fixture-event"));

    // White passes the transport validation, but the shared compiler rejects it.
    // The failure must leave both installed files byte-for-byte unchanged.
    let previous_data_bytes = fs::read(&data_path).unwrap();
    let previous_css_bytes = fs::read(&css_path).unwrap();
    let invalid = SettingsDraft {
        accent_color: "#FFFFFF".into(),
        ..draft.clone()
    };
    assert!(save_settings_blocking(invalid).is_err());
    assert_eq!(fs::read(&data_path).unwrap(), previous_data_bytes);
    assert_eq!(fs::read(&css_path).unwrap(), previous_css_bytes);
    assert_eq!(
        fs::read_to_string(fixture.join("unrelated.txt")).unwrap(),
        "keep this file"
    );

    // A source-folder settings save must not replace an in-use runtime script.
    let runtime_rebuild = blob.join("runtime").join("scripts").join("Rebuild.ps1");
    let runtime_text =
        fs::read_to_string(&runtime_rebuild).unwrap() + "\n# runtime-preservation-fixture\n";
    fs::write(&runtime_rebuild, &runtime_text).unwrap();
    {
        use std::os::windows::fs::OpenOptionsExt;
        let _locked = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&runtime_rebuild)
            .unwrap();
        save_settings_blocking(draft.clone())
            .expect("Settings save must not redeploy locked runtime files");
        assert_eq!(fs::read_to_string(&runtime_rebuild).unwrap(), runtime_text);
    }

    // The installed runtime must also be usable without the source checkout.
    let runtime = blob.join("runtime");
    env::set_var("BLOBUI_SOURCE_DIRECTORY", &runtime);
    let runtime_draft = SettingsDraft {
        accent_color: "Default".into(),
        accent_mode: "native".into(),
        accent_preset: String::new(),
        ..draft
    };
    save_settings_blocking(runtime_draft).expect("Save from the installed runtime must rebuild");
    assert_eq!(
        read_json_file(&data_path).unwrap()["settings"]["accentColor"],
        "var(--color-system-accent-100)"
    );
    assert!(fs::read_to_string(&css_path)
        .unwrap()
        .contains("fixture-event"));
    assert!(fs::read_dir(blob.join("staging")).unwrap().next().is_none());

    configure_connection_blocking(ConnectionDraft {
        enabled: false,
        api_key: String::new(),
        port: None,
    })
    .expect("No API mode must rebuild through stdin without a key");
    let disabled = read_json_file(&data_path).unwrap();
    assert_eq!(disabled["apiConnection"]["enabled"], false);
    assert_eq!(disabled["apiConnection"]["port"], 31009);
    assert!(!fs::read_to_string(&css_path)
        .unwrap()
        .contains("fixture-event"));
    assert!(load_app_state().unwrap().onboarding_complete);
    let disabled_save = save_settings_blocking(SettingsDraft::default()).unwrap();
    assert_eq!(disabled_save.calendar_status, "disabled");
    let before_invalid_connection = fs::read(&data_path).unwrap();
    assert!(configure_connection_blocking(ConnectionDraft {
        enabled: true,
        api_key: "test-key".into(),
        port: Some(0),
    })
    .is_err());
    assert_eq!(fs::read(&data_path).unwrap(), before_invalid_connection);

    let canonical_fixture = fixture.canonicalize().unwrap();
    assert!(canonical_fixture.starts_with(fixture_parent.canonicalize().unwrap()));
    fs::remove_dir_all(canonical_fixture).unwrap();
}
