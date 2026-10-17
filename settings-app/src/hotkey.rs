//! Foreground-scoped shortcuts for BlobUI Settings and Anytype.
use std::sync::OnceLock;

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

fn is_alt_r_key(key: u32) -> bool {
    key == b'R' as u32
}

fn alt_r_modifiers_match(alt_down: bool, other_modifier_down: bool) -> bool {
    alt_down && !other_modifier_down
}

#[cfg(test)]
mod tests {
    use super::{alt_r_modifiers_match, is_alt_r_key, KeyGate};

    #[test]
    fn alt_r_requires_r_and_only_alt() {
        assert!(is_alt_r_key(b'R' as u32));
        assert!(!is_alt_r_key(b'1' as u32));
        assert!(alt_r_modifiers_match(true, false));
        assert!(!alt_r_modifiers_match(false, false));
        assert!(!alt_r_modifiers_match(true, true));
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

pub fn start(app: tauri::AppHandle) {
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
    use super::{alt_r_modifiers_match, is_alt_r_key, KeyGate};
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
        GetAsyncKeyState, VK_CONTROL, VK_END, VK_LWIN, VK_MENU, VK_NUMPAD1, VK_RWIN, VK_SHIFT,
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

    fn is_numpad_one(key: u32, scan: u32, flags: u32) -> bool {
        flags & LLKHF_EXTENDED == 0
            && (key == u32::from(VK_NUMPAD1) || (key == u32::from(VK_END) && scan == 0x4f))
    }

    fn is_anytype_executable(path: &str) -> bool {
        Path::new(path)
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Anytype.exe"))
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
            if key.flags & LLKHF_INJECTED == 0
                && matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP)
            {
                let down = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
                if is_numpad_one(key.vkCode, key.scanCode, key.flags) {
                    let modified = [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
                        .iter()
                        .any(|key| GetAsyncKeyState(i32::from(*key)) < 0);
                    let (consume, open) = NUMPAD_GATE.with(|gate| {
                        gate.borrow_mut()
                            .handle(down, down && anytype_is_foreground(), !modified)
                    });
                    if open {
                        if let Some(sender) = SHORTCUT_ACTIONS.get() {
                            let _ = sender.try_send(ShortcutAction::OpenSettings);
                        }
                    }
                    if consume {
                        return 1;
                    }
                } else if is_alt_r_key(key.vkCode) {
                    let alt_down = GetAsyncKeyState(i32::from(VK_MENU)) < 0;
                    let other_modifier_down = [VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN]
                        .iter()
                        .any(|key| GetAsyncKeyState(i32::from(*key)) < 0);
                    let in_scope = down && settings_or_anytype_is_foreground();
                    let (consume, rebuild) = REBUILD_GATE.with(|gate| {
                        gate.borrow_mut().handle(
                            down,
                            in_scope,
                            alt_r_modifiers_match(alt_down, other_modifier_down),
                        )
                    });
                    if rebuild {
                        if let Some(sender) = SHORTCUT_ACTIONS.get() {
                            let _ = sender.try_send(ShortcutAction::RebuildCustomCss);
                        }
                    }
                    if consume {
                        return 1;
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
            assert!(is_numpad_one(u32::from(VK_NUMPAD1), 0x4f, 0));
            assert!(is_numpad_one(u32::from(VK_END), 0x4f, 0));
            assert!(!is_numpad_one(u32::from(VK_END), 0x4f, LLKHF_EXTENDED));
            assert!(!is_numpad_one(0x31, 0x02, 0));
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
