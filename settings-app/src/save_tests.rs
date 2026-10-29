//! Exercise native CSS compilation, data migration, and transactional saves.
//! Fixtures stay under the repository and never touch the user's Anytype data.
#![cfg(windows)]

use super::*;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

static ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn calendar_rule_save_is_transactional_and_preserves_cache_on_outage() {
    let _lock = ENVIRONMENT_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixture = source.join(".test-run").join(format!("calendar-save-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    let install = fixture.join("Anytype"); let runtime = fixture.join("runtime");
    fs::create_dir_all(install.join("blob")).unwrap();
    copy_tree(&source.join("Dev"), &runtime.join("Dev")); copy_tree(&source.join("assets"), &runtime.join("assets"));
    let data_path = install.join("blob/data.json"); let css_path = install.join("custom.css");
    fs::write(&data_path, serde_json::to_vec(&json!({"calendarObjectColors":{"event":"Purple"}})).unwrap()).unwrap();
    fs::write(&css_path, b"previous css").unwrap();
    let _environment = EnvironmentGuard::set(&[("BLOBUI_INSTALL_DIRECTORY", &install), ("BLOBUI_SOURCE_DIRECTORY", &runtime)]);
    let config: calendar::Configuration = serde_json::from_value(json!({"version":1,"types":[{"spaceId":"space","typeKey":"task","default":{"mode":"none"},"overrides":[]}]})).unwrap();
    let draft = SettingsDraft { accent_color: "#27A879".into(), accent_mode: "custom".into(), accent_preset: String::new(), ..SettingsDraft::default() };
    let before = fs::read(&data_path).unwrap();
    let error = save_settings_with_calendar_rules(draft.clone(), Some(config.clone())).unwrap_err();
    assert!(error.contains("could not be refreshed"));
    assert_eq!(fs::read(&data_path).unwrap(), before); assert_eq!(fs::read(&css_path).unwrap(), b"previous css");
    let settings = SettingsDraft { calendar_colors_enabled:false, ..draft.clone() };
    assert_eq!(save_settings_with_calendar_rules(settings, Some(config.clone())).unwrap().calendar_status, "off");
    let saved = read_json_file(&data_path).unwrap();
    assert_eq!(serde_json::to_value(calendar::configuration(&saved).unwrap()).unwrap(), serde_json::to_value(config).unwrap());
    assert_eq!(saved["calendarObjectColors"]["event"], "Purple");
    assert!(!fs::read_to_string(&css_path).unwrap().contains("record-event"));
    let cached = save_settings_blocking(draft).unwrap();
    assert_eq!(cached.calendar_status, "cached");
    assert_eq!(read_json_file(&data_path).unwrap()["calendarObjectColors"]["event"], "Purple");
}

struct EnvironmentGuard(Vec<(&'static str, Option<OsString>)>);

impl EnvironmentGuard {
    fn set(values: &[(&'static str, &Path)]) -> Self {
        let mut isolated: Vec<(&'static str, PathBuf)> = values.iter()
            .map(|(key, value)| (*key, value.to_path_buf())).collect();
        if let Some((_, install)) = isolated.iter().find(|(key, _)| *key == "BLOBUI_INSTALL_DIRECTORY") {
            let profile = install.parent().expect("fixture install has a parent").join("test-profile");
            for (key, suffix) in [("APPDATA", "roaming"), ("LOCALAPPDATA", "local")] {
                if !isolated.iter().any(|(existing, _)| *existing == key) {
                    let directory = profile.join(suffix);
                    fs::create_dir_all(&directory).unwrap();
                    isolated.push((key, directory));
                }
            }
        }
        for (key, value) in &isolated {
            if matches!(*key, "APPDATA" | "LOCALAPPDATA") {
                let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
                assert!(value.starts_with(workspace.join(".test-run")), "test profile must be in workspace fixtures");
                fs::create_dir_all(value).unwrap();
            }
            assert_fixture_path(value).expect("native tests must use workspace fixtures");
        }
        let previous = isolated
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
fn native_fixture_profile_overrides_inherited_saved_folder_without_touching_credentials() {
    let _lock = ENVIRONMENT_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let fixture = source.join(".test-run").join(format!("profile-isolation-{}-{nonce}", std::process::id()));
    let install = fixture.join("Test Anytype");
    let other_install = fixture.join("Simulated user Anytype");
    fs::create_dir_all(install.join("blob")).unwrap();
    fs::create_dir_all(other_install.join("blob/credentials")).unwrap();
    fs::write(install.join("blob/data.json"), b"{}").unwrap();
    fs::write(other_install.join("blob/data.json"), b"{}").unwrap();
    let sentinel = other_install.join("blob/credentials/anytype-api-key.dpapi");
    fs::write(&sentinel, b"untouched simulated credential").unwrap();
    let inherited_profile = fixture.join("inherited-profile");
    let preferences = updater::preferences_path(&inherited_profile.join("anytype"));
    fs::create_dir_all(preferences.parent().unwrap()).unwrap();
    fs::write(&preferences, serde_json::to_vec(&json!({"installDirectory": other_install})).unwrap()).unwrap();
    {
        let _inherited = EnvironmentGuard::set(&[("APPDATA", &inherited_profile)]);
        let _isolated = EnvironmentGuard::set(&[("BLOBUI_INSTALL_DIRECTORY", &install)]);
        let resolved = resolve_install_directory(&app_data_directory()).unwrap();
        assert_eq!(resolved.canonicalize().unwrap(), install.canonicalize().unwrap());
        assert_fixture_path(&app_data_directory().parent().unwrap()).unwrap();
    }
    assert_eq!(fs::read(&sentinel).unwrap(), b"untouched simulated credential");
    fs::remove_dir_all(fixture).unwrap();
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn mock_api_server(
    ip: &str,
    expected_key: &'static str,
    responses: Vec<(u16, Value)>,
) -> (u16, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind((ip, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            let (header_end, content_length) = loop {
                let count = stream.read(&mut buffer).unwrap();
                assert_ne!(count, 0, "client closed before sending the request");
                request.extend_from_slice(&buffer[..count]);
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
                let count = stream.read(&mut buffer).unwrap();
                assert_ne!(count, 0, "client closed before sending the request body");
                request.extend_from_slice(&buffer[..count]);
            }
            let header = String::from_utf8_lossy(&request[..header_end]);
            assert!(header.contains(&format!("Authorization: Bearer {expected_key}")));
            requests.push(header.to_string());
            let response_body = serde_json::to_vec(&body).unwrap();
            let reason = if status == 200 { "OK" } else if status == 401 { "Unauthorized" } else { "Server Error" };
            write!(stream, "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response_body.len()).unwrap();
            stream.write_all(&response_body).unwrap();
        }
        requests
    });
    (port, worker)
}

fn successful_api_responses() -> Vec<(u16, Value)> {
    vec![
        (200, json!({"data": []})),
        (200, json!({"data": [{"id": "space-1"}], "has_more": false})),
        (200, json!({"data": [{"name": "Calendar Color", "key": "calendar-color"}], "has_more": false})),
        (200, json!({"data": [{"id": "event-1", "properties": {"calendar-color": {"format": "select", "select": {"name": "Green"}}}}], "has_more": false})),
    ]
}

#[test]
fn native_save_compiles_css_and_preserves_unmanaged_files() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixture_parent = source.join(".test-run");
    let fixture = fixture_parent.join(format!(
        "native-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let runtime = fixture.join("runtime");
    copy_tree(&source.join("Dev"), &runtime.join("Dev"));
    copy_tree(&source.join("assets"), &runtime.join("assets"));

    let install = fixture.join("Anytype data");
    let blob = install.join("blob");
    fs::create_dir_all(blob.join("assets")).unwrap();
    let data_path = blob.join("data.json");
    let css_path = install.join("custom.css");
    let original_data = json!({
        "schemaVersion": 1,
        "installDirectory": install,
        "settings": {
            "fontFamily": "Libron",
            "codeFontFamily": "Consolas",
            "accentColor": "#DE50DE",
            "bannerPath": "spacebanner\\banner.png"
        },
        "calendarObjectColors": { "fixture-event": "Green" }
    });
    fs::write(&data_path, serde_json::to_vec_pretty(&original_data).unwrap()).unwrap();
    fs::write(&css_path, "previous stylesheet").unwrap();
    fs::write(fixture.join("unrelated.txt"), "keep this file").unwrap();
    fs::write(blob.join(".blobui-managed-files.json"), b"{\"managedFiles\":[\"assets/fixture.png\"]}").unwrap();
    fs::write(blob.join("assets/fixture.png"), b"installed asset").unwrap();

    let settings_source = runtime.join("Dev");
    let source_file = fs::read_dir(&settings_source)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("css")))
        .expect("runtime fixture has CSS sources");
    let source_bytes = fs::read(&source_file).unwrap();
    let managed_manifest = fs::read(blob.join(".blobui-managed-files.json")).unwrap();
    let managed_asset = fs::read(blob.join("assets/fixture.png")).unwrap();

    let _environment_lock = ENVIRONMENT_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = EnvironmentGuard::set(&[
        ("APPDATA", &fixture.join("roaming")),
        ("BLOBUI_INSTALL_DIRECTORY", &install),
        ("BLOBUI_SOURCE_DIRECTORY", &runtime),
    ]);
    let draft = SettingsDraft {
        font_family: "Inter".into(),
        code_font_family: "Cascadia Code".into(),
        accent_color: "#27A879".into(),
        accent_mode: "custom".into(),
        accent_preset: String::new(),
        include_selections: true,
        banner_image: "data:image/png;base64,iVBORw0KGgo=".into(),
        reduced_motion: false,
        tab_hover_effect: true,
        link_hover_effect: true,
        object_mention_hover_effect: true,
        kanban_view_style: "compact".into(),
        collapsed_categories: true,
        calendar_colors_enabled: true,
        settings_v21_enabled: false,
        sidebar_resize_mode: SidebarResizeMode::Off,
    };
    let result = save_settings_blocking(draft.clone()).expect("native Save must compile locally");
    assert_eq!(result.calendar_status, "cached");
    let saved_data = read_json_file(&data_path).unwrap();
    assert_eq!(saved_data["settingsV2"]["version"], 2);
    assert_eq!(saved_data["settingsV2"]["settings"]["accentColor"], "#27A879");
    assert_eq!(saved_data["settingsV2"]["settings"]["reducedMotion"], false);
    assert_eq!(saved_data["settingsV2"]["settings"]["kanbanViewStyle"], "compact");
    assert_eq!(saved_data["settingsV2"]["settings"]["collapsedCategories"], true);
    let restored = settings_from_data(&saved_data);
    assert_eq!(restored.kanban_view_style, "compact");
    assert!(restored.collapsed_categories);
    assert_eq!(saved_data["settingsV2"]["settings"]["tabHoverEffect"], true);
    assert_eq!(saved_data["settingsV2"]["settings"]["bannerImage"], draft.banner_image);
    assert_eq!(saved_data["settings"]["fontFamily"], "Inter");
    assert_eq!(saved_data["calendarObjectColors"]["fixture-event"], "Green");
    let saved_css = fs::read_to_string(&css_path).unwrap();
    assert!(saved_css.contains("#27A879"));
    assert!(saved_css.contains("Cascadia Code"));
    assert!(saved_css.contains("url(\"data:image/png;base64,iVBORw0KGgo=\")"));
    assert!(saved_css.contains("fixture-event"));
    let experimental = SettingsDraft { settings_v21_enabled: true, sidebar_resize_mode: SidebarResizeMode::Pro, ..draft.clone() };
    save_settings_blocking(experimental).unwrap();
    assert!(settings_from_data(&read_json_file(&data_path).unwrap()).settings_v21_enabled);
    assert_eq!(settings_from_data(&read_json_file(&data_path).unwrap()).sidebar_resize_mode, SidebarResizeMode::Pro);
    assert_eq!(saved_css, fs::read_to_string(&css_path).unwrap());
    save_settings_blocking(draft.clone()).unwrap();
    assert!(!settings_from_data(&read_json_file(&data_path).unwrap()).settings_v21_enabled);

    assert!(saved_css.contains("--blobui-selection-fill:"));
    let selections_off = SettingsDraft { include_selections: false, ..draft.clone() };
    save_settings_blocking(selections_off).unwrap();
    assert!(!fs::read_to_string(&css_path).unwrap().contains("--blobui-selection-fill:"));
    assert!(!settings_from_data(&read_json_file(&data_path).unwrap()).include_selections);

    let off = SettingsDraft { calendar_colors_enabled: false, ..draft.clone() };
    assert_eq!(save_settings_blocking(off).unwrap().calendar_status, "off");
    assert!(!fs::read_to_string(&css_path).unwrap().contains("record-fixture-event"));
    let off_data = read_json_file(&data_path).unwrap();
    assert!(!settings_from_data(&off_data).calendar_colors_enabled);
    assert_eq!(off_data["calendarObjectColors"]["fixture-event"], "Green");
    save_settings_blocking(draft.clone()).unwrap();
    assert!(fs::read_to_string(&css_path).unwrap().contains("record-fixture-event"));

    // White passes settings validation but the native CSS compiler rejects it.
    // No installed file may change when compilation fails before commit.
    let previous_data_bytes = fs::read(&data_path).unwrap();
    let previous_css_bytes = fs::read(&css_path).unwrap();
    let invalid = SettingsDraft { accent_color: "#FFFFFF".into(), ..draft.clone() };
    assert!(save_settings_blocking(invalid).is_err());
    assert_eq!(fs::read(&data_path).unwrap(), previous_data_bytes);
    assert_eq!(fs::read(&css_path).unwrap(), previous_css_bytes);
    assert_eq!(fs::read(fixture.join("unrelated.txt")).unwrap(), b"keep this file");
    assert_eq!(fs::read(blob.join(".blobui-managed-files.json")).unwrap(), managed_manifest);
    assert_eq!(fs::read(blob.join("assets/fixture.png")).unwrap(), managed_asset);
    assert_eq!(fs::read(source_file).unwrap(), source_bytes);
    assert!(!blob.join("staging").exists(), "native Save does not create a staging/runtime deployment folder");

    configure_connection_blocking(ConnectionDraft {
        enabled: false,
        api_key: String::new(),
        keep_existing_key: false,
        ip: None,
        port: None,
    })
    .expect("No API mode saves without contacting Anytype");
    let disabled = read_json_file(&data_path).unwrap();
    assert_eq!(disabled["apiConnection"]["enabled"], false);
    assert_eq!(disabled["apiConnection"]["port"], 31009);
    assert_eq!(disabled["calendarObjectColors"], json!({}));
    assert!(!fs::read_to_string(&css_path).unwrap().contains("fixture-event"));
    assert!(load_app_state().unwrap().onboarding_complete);
    let disabled_settings = load_app_state().unwrap().settings;
    let disabled_save = save_settings_blocking(disabled_settings).unwrap();
    assert_eq!(disabled_save.calendar_status, "disabled");

    let credential = blob.join("credentials/anytype-api-key.dpapi");
    fs::create_dir_all(credential.parent().unwrap()).unwrap();
    let protected = connection::encrypt_api_key("fixture-reveal-key").unwrap();
    fs::write(&credential, format!("{protected}\n")).unwrap();
    let reveal_started = std::time::Instant::now();
    assert_eq!(read_saved_api_key().unwrap(), "fixture-reveal-key");
    eprintln!("Native saved-key reveal took {:?}", reveal_started.elapsed());
    fs::write(&credential, "not-a-valid-credential").unwrap();
    assert!(read_saved_api_key().is_err());
    fs::remove_file(&credential).unwrap();

    let before_invalid_connection = fs::read(&data_path).unwrap();
    assert!(configure_connection_blocking(ConnectionDraft {
        enabled: true,
        api_key: "test-key".into(),
        keep_existing_key: false,
        ip: None,
        port: Some(0),
    })
    .is_err());
    assert_eq!(fs::read(&data_path).unwrap(), before_invalid_connection);

    let canonical_fixture = fixture.canonicalize().unwrap();
    assert!(canonical_fixture.starts_with(fixture_parent.canonicalize().unwrap()));
    fs::remove_dir_all(canonical_fixture).unwrap();
}

#[test]
#[ignore = "Retired credential-reset flow. Use profile-isolation regressions; never run against a user profile."]
fn native_connection_uses_custom_ip_and_port_and_rolls_back_failures() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let fixture = source.join(".test-run").join(format!("native-connection-{}-{nonce}", std::process::id()));
    let runtime = fixture.join("runtime");
    copy_tree(&source.join("Dev"), &runtime.join("Dev"));
    copy_tree(&source.join("assets"), &runtime.join("assets"));
    let install = fixture.join("Anytype data");
    let blob = install.join("blob");
    fs::create_dir_all(&blob).unwrap();
    let data_path = blob.join("data.json");
    let css_path = install.join("custom.css");
    let data = json!({
        "installDirectory": install,
        "settings": { "fontFamily": "Libron", "codeFontFamily": "Consolas", "accentColor": "#DE50DE" },
        "calendarObjectColors": { "old-event": "Blue" }
    });
    fs::write(&data_path, serde_json::to_vec_pretty(&data).unwrap()).unwrap();
    fs::write(&css_path, b"old css").unwrap();
    let _environment_lock = ENVIRONMENT_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = EnvironmentGuard::set(&[
        ("BLOBUI_INSTALL_DIRECTORY", &install),
        ("BLOBUI_SOURCE_DIRECTORY", &runtime),
    ]);

    let (port, server) = mock_api_server("127.0.0.2", "fixture-key-one", successful_api_responses());
    let configured = configure_connection_blocking(ConnectionDraft {
        enabled: true,
        api_key: "fixture-key-one".into(),
        keep_existing_key: false,
        ip: Some("127.0.0.2".into()),
        port: Some(port),
    }).expect("valid API v2 key should be saved");
    assert_eq!(configured.calendar_status, "refreshed");
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests.iter().all(|request| request.contains(&format!("Host: 127.0.0.2:{port}"))));
    let saved = read_json_file(&data_path).unwrap();
    assert_eq!(saved["apiConnection"]["ip"], "127.0.0.2");
    assert_eq!(saved["apiConnection"]["port"], port);
    assert_eq!(saved["calendarObjectColors"]["event-1"], "Green");
    assert!(!fs::read_to_string(&data_path).unwrap().contains("fixture-key-one"));
    let credential_path = blob.join("credentials/anytype-api-key.dpapi");
    let first_credential = fs::read(&credential_path).unwrap();
    assert_eq!(read_saved_api_key_at(&install).unwrap(), "fixture-key-one");

    // A rotated, authenticated key remains usable when the optional calendar
    // endpoint fails, and old-account calendar values are cleared.
    let (port, server) = mock_api_server("127.0.0.2", "fixture-key-two", vec![
        (200, json!({"data": []})),
        (500, json!({"message": "optional calendar failure"})),
    ]);
    let rotated = configure_connection_blocking(ConnectionDraft {
        enabled: true,
        api_key: "fixture-key-two".into(),
        keep_existing_key: false,
        ip: Some("127.0.0.2".into()),
        port: Some(port),
    }).expect("calendar failure must not reject a valid replacement key");
    assert_eq!(rotated.calendar_status, "cached");
    assert_eq!(server.join().unwrap().len(), 2);
    assert_ne!(fs::read(&credential_path).unwrap(), first_credential);
    assert_eq!(read_saved_api_key_at(&install).unwrap(), "fixture-key-two");
    assert_eq!(read_json_file(&data_path).unwrap()["calendarObjectColors"], json!({}));

    let saved_data = fs::read(&data_path).unwrap();
    let saved_css = fs::read(&css_path).unwrap();
    let saved_key = fs::read(&credential_path).unwrap();
    let (port, server) = mock_api_server("127.0.0.2", "invalid-key", vec![
        (401, json!({"message": "do not include this response in the UI"})),
    ]);
    let rejected = configure_connection_blocking(ConnectionDraft {
        enabled: true,
        api_key: "invalid-key".into(),
        keep_existing_key: false,
        ip: Some("127.0.0.2".into()),
        port: Some(port),
    }).unwrap_err();
    assert!(rejected.contains("401"));
    assert!(!rejected.contains("do not include"));
    assert_eq!(server.join().unwrap().len(), 1);
    assert_eq!(fs::read(&data_path).unwrap(), saved_data);
    assert_eq!(fs::read(&css_path).unwrap(), saved_css);
    assert_eq!(fs::read(&credential_path).unwrap(), saved_key);

    // CSS compilation failure after successful authentication still leaves
    // settings and the currently encrypted credential untouched.
    let broken_css = runtime.join("Dev/zz-native-invalid.css");
    fs::write(&broken_css, "body { color: {{undeclared}}; }").unwrap();
    let (port, server) = mock_api_server("127.0.0.2", "fixture-key-one", successful_api_responses());
    assert!(configure_connection_blocking(ConnectionDraft {
        enabled: true,
        api_key: "fixture-key-one".into(),
        keep_existing_key: false,
        ip: Some("127.0.0.2".into()),
        port: Some(port),
    }).is_err());
    assert_eq!(server.join().unwrap().len(), 4);
    fs::remove_file(broken_css).unwrap();
    assert_eq!(fs::read(&data_path).unwrap(), saved_data);
    assert_eq!(fs::read(&css_path).unwrap(), saved_css);
    assert_eq!(fs::read(&credential_path).unwrap(), saved_key);

    // Calendar opt-out still verifies a connection but never fetches colors.
    let mut off_data = read_json_file(&data_path).unwrap();
    off_data["settingsV2"]["settings"]["calendarColorsEnabled"] = json!(false);
    fs::write(&data_path, serde_json::to_vec(&off_data).unwrap()).unwrap();
    let (port, server) = mock_api_server("127.0.0.2", "fixture-key-two", vec![(200, json!({"data": []}))]);
    let off_result = configure_connection_blocking(ConnectionDraft {
        enabled: true, api_key: String::new(), keep_existing_key: true,
        ip: Some("127.0.0.2".into()), port: Some(port),
    }).unwrap();
    assert_eq!(off_result.calendar_status, "off");
    assert_eq!(server.join().unwrap().len(), 1);
    let reconnected = read_json_file(&data_path).unwrap();
    assert!(!settings_from_data(&reconnected).calendar_colors_enabled);
    assert_eq!(reconnected["calendarObjectColors"], off_data["calendarObjectColors"]);
    assert!(!fs::read_to_string(&css_path).unwrap().contains("record-event-1"));

    // Opt-out does not make an API request and removes both cache and key.
    let disabled = configure_connection_blocking(ConnectionDraft {
        enabled: false,
        api_key: String::new(),
        keep_existing_key: false,
        ip: None,
        port: None,
    }).unwrap();
    assert_eq!(disabled.calendar_status, "disabled");
    assert!(!credential_path.exists());
    assert_eq!(read_json_file(&data_path).unwrap()["calendarObjectColors"], json!({}));
    let canonical = fixture.canonicalize().unwrap();
    assert!(canonical.starts_with(source.join(".test-run").canonicalize().unwrap()));
    fs::remove_dir_all(canonical).unwrap();
}

#[test]
fn calendar_api_availability_checks_saved_credentials_and_endpoint() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let fixture = source.join(".test-run").join(format!("calendar-api-{}-{nonce}", std::process::id()));
    let credentials = fixture.join("blob/credentials");
    fs::create_dir_all(&credentials).unwrap();
    let data_path = fixture.join("blob/data.json");
    fs::write(&data_path, b"{}").unwrap();
    assert_eq!(calendar_api_availability_at(&fixture), "missing");
    let credential = credentials.join("anytype-api-key.dpapi");
    fs::write(&credential, b"invalid credential").unwrap();
    assert_eq!(calendar_api_availability_at(&fixture), "unavailable");
    fs::write(&credential, connection::encrypt_api_key("availability-key").unwrap()).unwrap();
    for (http_status, body, expected) in [
        (200, json!({"data": []}), "available"),
        (401, json!({"error": "invalid key"}), "unavailable"),
        (200, json!({"unexpected": []}), "unavailable"),
    ] {
        let (port, server) = mock_api_server("127.0.0.1", "availability-key", vec![(http_status, body)]);
        fs::write(&data_path, serde_json::to_vec(&json!({"apiConnection": {"enabled": true, "port": port}})).unwrap()).unwrap();
        assert_eq!(calendar_api_availability_at(&fixture), expected);
        server.join().unwrap();
    }
    fs::write(&data_path, b"{\"apiConnection\":{\"enabled\":false}}").unwrap();
    assert_eq!(calendar_api_availability_at(&fixture), "missing");
    fs::remove_dir_all(fixture).unwrap();
}
