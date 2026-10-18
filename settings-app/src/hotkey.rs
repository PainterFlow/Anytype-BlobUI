//! Foreground-scoped shortcuts for BlobUI Settings and Anytype.
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::{atomic::{AtomicBool, Ordering}, OnceLock, RwLock};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    pub action: String,
    pub code: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

pub fn default_shortcuts() -> Vec<Shortcut> {
    [
        ("openSettings", "Numpad1", false, false, false, false),
        ("rebuildCss", "KeyR", false, true, false, false),
        ("showOnboarding", "F8", false, false, false, false),
        ("simulateUpdate", "F9", false, false, false, false),
        ("quitSettings", "KeyQ", true, false, false, false),
    ].into_iter().map(|(action, code, ctrl, alt, shift, meta)| Shortcut {
        action: action.into(), code: code.into(), ctrl, alt, shift, meta,
    }).collect()
}

static SHORTCUTS: OnceLock<RwLock<Vec<Shortcut>>> = OnceLock::new();
static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

fn shortcut_store() -> &'static RwLock<Vec<Shortcut>> {
    SHORTCUTS.get_or_init(|| RwLock::new(default_shortcuts()))
}

pub fn get_shortcuts() -> Vec<Shortcut> {
    shortcut_store().read().map(|shortcuts| shortcuts.clone()).unwrap_or_else(|_| default_shortcuts())
}

pub fn set_capture_active(active: bool) {
    CAPTURE_ACTIVE.store(active, Ordering::Release);
}

pub fn save_shortcuts(path: &Path, shortcuts: Vec<Shortcut>) -> Result<Vec<Shortcut>, String> {
    validate_shortcuts(&shortcuts)?;
    if let Some(parent) = path.parent() {
        crate::storage::reject_reparse_path(parent)?;
        fs::create_dir_all(parent).map_err(|_| "Could not prepare the keyboard shortcut settings folder.".to_string())?;
    }
    let mut bytes = serde_json::to_vec_pretty(&shortcuts)
        .map_err(|_| "Could not encode the keyboard shortcuts.".to_string())?;
    bytes.push(b'\n');
    crate::storage::write_atomic(path, &bytes)?;
    *shortcut_store().write().map_err(|_| "The keyboard shortcut settings are unavailable.".to_string())? = shortcuts.clone();
    Ok(shortcuts)
}

fn load_shortcuts(path: &Path) -> Result<Vec<Shortcut>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let shortcuts: Vec<Shortcut> = serde_json::from_slice(&bytes)
                .map_err(|_| "The saved keyboard shortcuts are invalid.".to_string())?;
            validate_shortcuts(&shortcuts)?;
            Ok(shortcuts)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(default_shortcuts()),
        Err(_) => Err("The saved keyboard shortcuts could not be read.".into()),
    }
}

fn validate_shortcuts(shortcuts: &[Shortcut]) -> Result<(), String> {
    let expected = ["openSettings", "rebuildCss", "showOnboarding", "simulateUpdate", "quitSettings"];
    if shortcuts.len() != expected.len() {
        return Err("All five BlobUI keyboard shortcuts must be provided.".into());
    }
    for (index, shortcut) in shortcuts.iter().enumerate() {
        if !expected.contains(&shortcut.action.as_str())
            || shortcuts[..index].iter().any(|prior| prior.action == shortcut.action)
            || !valid_code(&shortcut.code)
        {
            return Err("A keyboard shortcut contains an unsupported action or key.".into());
        }
        if shortcuts[..index].iter().any(|prior| same_chord(prior, shortcut)) {
            return Err("Two BlobUI actions cannot use the same keyboard shortcut.".into());
        }
    }
    Ok(())
}

fn same_chord(left: &Shortcut, right: &Shortcut) -> bool {
    left.code == right.code && left.ctrl == right.ctrl && left.alt == right.alt
        && left.shift == right.shift && left.meta == right.meta
}

fn valid_code(code: &str) -> bool {
    matches!(code, "Numpad1" | "Numpad2" | "Numpad3" | "Numpad4" | "Numpad5" | "Numpad6" | "Numpad7" | "Numpad8" | "Numpad9" | "Numpad0" | "NumpadAdd" | "NumpadSubtract" | "NumpadMultiply" | "NumpadDivide" | "NumpadDecimal" | "Space" | "Enter" | "Escape" | "Tab" | "Backspace" | "Delete" | "Insert" | "Home" | "End" | "PageUp" | "PageDown" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Minus" | "Equal" | "BracketLeft" | "BracketRight" | "Backslash" | "Semicolon" | "Quote" | "Backquote" | "Comma" | "Period" | "Slash")
        || (code.strip_prefix("Key").is_some_and(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_uppercase()))
        || (code.strip_prefix("Digit").is_some_and(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_digit()))
        || (code.strip_prefix('F').is_some_and(|rest| rest.parse::<u8>().is_ok_and(|number| (1..=12).contains(&number)) && code == format!("F{}", rest.parse::<u8>().unwrap_or_default())))
}

#[derive(Default)]
struct KeyGate {
    captured: bool,
}

impl KeyGate {
    fn handle(&mut self, down: bool, in_scope: bool, modifiers_match: bool) -> (bool, bool) {
        if self.captured {
            if !down {
                self.captured = false;
            }
            return (true, false);
        }
        if down && in_scope && modifiers_match {
            self.captured = true;
            return (true, true);
        }
        (false, false)
    }
}

#[cfg(test)]
mod tests {
    use super::{default_shortcuts, get_shortcuts, load_shortcuts, save_shortcuts, validate_shortcuts, KeyGate};
    use std::{fs, path::Path};

    #[test]
    fn defaults_cover_every_blobui_shortcut_and_are_unique() {
        let shortcuts = default_shortcuts();
        assert_eq!(shortcuts.len(), 5);
        validate_shortcuts(&shortcuts).unwrap();
        for action in ["openSettings", "rebuildCss", "showOnboarding", "simulateUpdate", "quitSettings"] {
            assert!(shortcuts.iter().any(|shortcut| shortcut.action == action));
        }
    }

    #[test]
    fn rejects_duplicate_chords_and_noncanonical_function_keys() {
        let mut shortcuts = default_shortcuts();
        shortcuts[1].code = shortcuts[0].code.clone();
        shortcuts[1].ctrl = shortcuts[0].ctrl;
        shortcuts[1].alt = shortcuts[0].alt;
        shortcuts[1].shift = shortcuts[0].shift;
        shortcuts[1].meta = shortcuts[0].meta;
        assert!(validate_shortcuts(&shortcuts).is_err());
        shortcuts = default_shortcuts();
        shortcuts[2].code = "F01".into();
        assert!(validate_shortcuts(&shortcuts).is_err());
    }

    #[test]
    fn saves_valid_shortcuts_atomically_and_updates_the_runtime_bindings() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = workspace.join(".test-run").join(format!("shortcuts-{}-{nonce}", std::process::id()));
        let path = root.join("BlobUI/shortcuts.json");
        let mut shortcuts = default_shortcuts();
        shortcuts[2].code = "F10".into();
        assert_eq!(save_shortcuts(&path, shortcuts.clone()).unwrap(), shortcuts);
        assert_eq!(get_shortcuts(), shortcuts);
        assert_eq!(load_shortcuts(&path).unwrap(), shortcuts);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hotkey_gate_suppresses_repeat_and_consumes_release_after_focus_changes() {
        let mut gate = KeyGate::default();
        assert_eq!(gate.handle(true, false, true), (false, false));
        assert_eq!(gate.handle(true, true, false), (false, false));
        assert_eq!(gate.handle(true, true, true), (true, true));
        assert_eq!(gate.handle(true, true, true), (true, false));
        assert_eq!(gate.handle(false, false, false), (true, false));
        assert_eq!(gate.handle(true, false, false), (false, false));
    }
}

static STARTUP_ERROR: OnceLock<String> = OnceLock::new();

pub fn error() -> Option<String> {
    STARTUP_ERROR.get().cloned()
}

pub fn start(app: tauri::AppHandle, preferences_path: &Path) {
    match load_shortcuts(preferences_path) {
        Ok(shortcuts) => {
            if let Ok(mut current) = shortcut_store().write() { *current = shortcuts; }
        }
        Err(error) => { let _ = STARTUP_ERROR.set(error); }
    }
    #[cfg(windows)]
    if let Err(error) = windows::start(app) {
        let _ = STARTUP_ERROR.set(error);
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        let _ = STARTUP_ERROR.set("The BlobUI keyboard shortcuts are supported on Windows.".into());
    }
}

#[cfg(windows)]
mod windows {
    use super::{get_shortcuts, KeyGate, CAPTURE_ACTIVE, Ordering};
    use std::path::Path;
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::{sync_channel, SyncSender};
    use std::sync::OnceLock;
    use tauri::Emitter;
    use windows_sys::Win32::Foundation::{CloseHandle, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
        SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED,
        MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static SHORTCUT_ACTIONS: OnceLock<SyncSender<ShortcutAction>> = OnceLock::new();
    thread_local! {
        static NUMPAD_GATE: std::cell::RefCell<KeyGate> = std::cell::RefCell::new(KeyGate::default());
        static REBUILD_GATE: std::cell::RefCell<KeyGate> = std::cell::RefCell::new(KeyGate::default());
    }

    #[derive(Clone, Copy)]
    enum ShortcutAction {
        OpenSettings,
        RebuildCustomCss,
    }

    fn is_anytype_executable(path: &str) -> bool {
        Path::new(path)
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Anytype.exe"))
    }

    fn code_matches(code: &str, key: u32, scan: u32, flags: u32) -> bool {
        if code.starts_with("Numpad") {
            let (num_lock_vk, num_lock_off_vk, num_lock_off_scan) = match code {
                "Numpad0" => (0x60, 0x2d, 0x52), "Numpad1" => (0x61, 0x23, 0x4f),
                "Numpad2" => (0x62, 0x28, 0x50), "Numpad3" => (0x63, 0x22, 0x51),
                "Numpad4" => (0x64, 0x25, 0x4b), "Numpad5" => (0x65, 0x0c, 0x4c),
                "Numpad6" => (0x66, 0x27, 0x4d), "Numpad7" => (0x67, 0x24, 0x47),
                "Numpad8" => (0x68, 0x26, 0x48), "Numpad9" => (0x69, 0x21, 0x49),
                "NumpadDecimal" => (0x6e, 0x2e, 0x53),
                "NumpadAdd" => (0x6b, 0, 0), "NumpadSubtract" => (0x6d, 0, 0),
                "NumpadMultiply" => (0x6a, 0, 0), "NumpadDivide" => (0x6f, 0, 0),
                _ => return false,
            };
            return key == num_lock_vk || (num_lock_off_vk != 0 && flags & LLKHF_EXTENDED == 0
                && key == num_lock_off_vk && scan == num_lock_off_scan);
        }
        let vk = match code {
            "Space" => 0x20, "Enter" => 0x0d, "Escape" => 0x1b, "Tab" => 0x09,
            "Backspace" => 0x08, "Delete" => 0x2e, "Insert" => 0x2d,
            "Home" => 0x24, "End" => 0x23, "PageUp" => 0x21, "PageDown" => 0x22,
            "ArrowUp" => 0x26, "ArrowDown" => 0x28, "ArrowLeft" => 0x25, "ArrowRight" => 0x27,
            "Minus" => 0xbd, "Equal" => 0xbb, "BracketLeft" => 0xdb, "BracketRight" => 0xdd,
            "Backslash" => 0xdc, "Semicolon" => 0xba, "Quote" => 0xde, "Backquote" => 0xc0,
            "Comma" => 0xbc, "Period" => 0xbe, "Slash" => 0xbf,
            _ if code.starts_with("Key") => code.as_bytes()[3] as u32,
            _ if code.starts_with("Digit") => code.as_bytes()[5] as u32,
            _ if code.starts_with('F') => code[1..].parse::<u32>().ok().map(|number| 0x70 + number - 1).unwrap_or(0),
            _ => 0,
        };
        if matches!(code, "Delete" | "Insert" | "Home" | "End" | "PageUp" | "PageDown" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight") {
            key == vk && flags & LLKHF_EXTENDED != 0
        } else {
            key == vk
        }
    }

    unsafe fn modifiers_match(shortcut: &super::Shortcut) -> bool {
        let down = |key| GetAsyncKeyState(key) < 0;
        down(i32::from(VK_CONTROL)) == shortcut.ctrl
            && down(i32::from(VK_MENU)) == shortcut.alt
            && down(i32::from(VK_SHIFT)) == shortcut.shift
            && (down(i32::from(VK_LWIN)) || down(i32::from(VK_RWIN))) == shortcut.meta
    }

    unsafe fn foreground_process_id() -> u32 {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return 0;
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(foreground, &mut process_id);
        process_id
    }

    unsafe fn process_is_anytype(process_id: u32) -> bool {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return false;
        }
        let mut buffer = [0u16; 2048];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length);
        CloseHandle(process);
        result != 0 && is_anytype_executable(&String::from_utf16_lossy(&buffer[..length as usize]))
    }

    unsafe fn anytype_is_foreground() -> bool {
        let process_id = foreground_process_id();
        process_id != 0 && process_is_anytype(process_id)
    }

    unsafe fn settings_or_anytype_is_foreground() -> bool {
        let process_id = foreground_process_id();
        process_id == std::process::id()
            || (process_id != 0 && process_is_anytype(process_id))
    }

    unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
        if code >= 0 && data != 0 {
            let key = &*(data as *const KBDLLHOOKSTRUCT);
            let message = message as u32;
            if !CAPTURE_ACTIVE.load(Ordering::Acquire) && key.flags & LLKHF_INJECTED == 0
                && matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP)
            {
                let down = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
                let shortcuts = get_shortcuts();
                for shortcut in shortcuts.iter().filter(|shortcut| code_matches(&shortcut.code, key.vkCode, key.scanCode, key.flags)) {
                    let in_scope = match shortcut.action.as_str() {
                        "openSettings" => down && anytype_is_foreground(),
                        "rebuildCss" => down && settings_or_anytype_is_foreground(),
                        _ => false,
                    };
                    let matched = modifiers_match(shortcut);
                    let action = match shortcut.action.as_str() {
                        "openSettings" => Some((ShortcutAction::OpenSettings, &NUMPAD_GATE)),
                        "rebuildCss" => Some((ShortcutAction::RebuildCustomCss, &REBUILD_GATE)),
                        _ => None,
                    };
                    if let Some((action, gate)) = action {
                        let (consume, fire) = gate.with(|gate| gate.borrow_mut().handle(down, in_scope, matched));
                        if fire {
                            if let Some(sender) = SHORTCUT_ACTIONS.get() { let _ = sender.try_send(action); }
                        }
                        if consume { return 1; }
                    }
                }
            }
        }
        CallNextHookEx(null_mut(), code, message, data)
    }

    pub(super) fn start(app: tauri::AppHandle) -> Result<(), String> {
        let (action_sender, action_receiver) = sync_channel(1);
        SHORTCUT_ACTIONS
            .set(action_sender)
            .map_err(|_| "The keyboard listener is already running.")?;
        std::thread::Builder::new()
            .name("blobui-shortcut-actions".into())
            .spawn(move || {
                while let Ok(action) = action_receiver.recv() {
                    match action {
                        ShortcutAction::OpenSettings => super::super::show_settings_window(&app),
                        ShortcutAction::RebuildCustomCss => {
                            let _ = app.emit("blobui-rebuild-custom-css", ());
                        }
                    }
                }
            })
            .map_err(|error| format!("Could not start the settings shortcut worker: {error}"))?;
        let (ready_sender, ready_receiver) = sync_channel(1);
        std::thread::Builder::new().name("blobui-anytype-keyboard".into()).spawn(move || unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), GetModuleHandleW(null()), 0);
            if hook.is_null() {
                let _ = ready_sender.send(Err(format!("The BlobUI keyboard shortcuts could not be enabled: {}", std::io::Error::last_os_error())));
                return;
            }
            let _ = ready_sender.send(Ok(()));
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {}
            UnhookWindowsHookEx(hook);
            let _ = super::STARTUP_ERROR.set("The keyboard listener stopped. Quit and reopen BlobUI Settings to enable Alt+R and Numpad 1 again.".into());
        }).map_err(|error| format!("Could not start the Anytype keyboard listener: {error}"))?;
        ready_receiver
            .recv_timeout(std::time::Duration::from_secs(3))
            .map_err(|error| format!("The Anytype keyboard listener did not start: {error}"))?
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn accepts_numpad_one_with_numlock_on_or_off_but_not_regular_end_or_one() {
            assert!(code_matches("Numpad1", 0x61, 0x4f, 0));
            assert!(code_matches("Numpad1", 0x23, 0x4f, 0));
            assert!(!code_matches("Numpad1", 0x23, 0x4f, LLKHF_EXTENDED));
            assert!(!code_matches("Numpad1", 0x31, 0x02, 0));
            for (code, numlock_on, numlock_off, scan) in [
                ("Numpad0", 0x60, 0x2d, 0x52), ("Numpad1", 0x61, 0x23, 0x4f),
                ("Numpad2", 0x62, 0x28, 0x50), ("Numpad3", 0x63, 0x22, 0x51),
                ("Numpad4", 0x64, 0x25, 0x4b), ("Numpad5", 0x65, 0x0c, 0x4c),
                ("Numpad6", 0x66, 0x27, 0x4d), ("Numpad7", 0x67, 0x24, 0x47),
                ("Numpad8", 0x68, 0x26, 0x48), ("Numpad9", 0x69, 0x21, 0x49),
                ("NumpadDecimal", 0x6e, 0x2e, 0x53),
            ] {
                assert!(code_matches(code, numlock_on, scan, 0));
                assert!(code_matches(code, numlock_off, scan, 0));
                assert!(!code_matches(code, numlock_off, scan, LLKHF_EXTENDED));
            }
            for (code, vk, scan) in [("Home", 0x24, 0x47), ("End", 0x23, 0x4f), ("ArrowUp", 0x26, 0x48)] {
                assert!(!code_matches(code, vk, scan, 0));
                assert!(code_matches(code, vk, scan, LLKHF_EXTENDED));
            }
            assert!(code_matches("Numpad7", 0x67, 0x47, 0));
            assert!(code_matches("NumpadDecimal", 0x6e, 0x53, 0));
        }

        #[test]
        fn passes_other_apps_and_modified_keys_through() {
            let mut gate = KeyGate::default();
            assert_eq!(gate.handle(true, false, false), (false, false));
            assert_eq!(gate.handle(false, false, false), (false, false));
            assert_eq!(gate.handle(true, true, false), (false, false));
        }

        #[test]
        fn opens_once_and_consumes_release_after_focus_changes() {
            let mut gate = KeyGate::default();
            assert_eq!(gate.handle(true, true, true), (true, true));
            assert_eq!(gate.handle(true, false, false), (true, false));
            assert_eq!(gate.handle(false, false, false), (true, false));
            assert_eq!(gate.handle(true, false, false), (false, false));
        }

        #[test]
        fn matches_process_name_instead_of_window_title() {
            assert!(is_anytype_executable(r"C:\Apps\Anytype\Anytype.exe"));
            assert!(!is_anytype_executable(r"C:\Apps\Browser.exe"));
        }
    }
}
