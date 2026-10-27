#[cfg(windows)]
pub fn select_anytype_folder(
    current_directory: Option<String>,
    owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    select_folder(
        current_directory,
        "Select Anytype Folder".into(),
        "Select folder".into(),
        owner_handle,
    )
}

#[cfg(windows)]
pub fn select_folder(
    current_directory: Option<String>,
    title: String,
    button_label: String,
    owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    std::thread::Builder::new()
        .name("blobui-folder-dialog".into())
        .spawn(move || select_folder_sta(current_directory, &title, &button_label, owner_handle))
        .map_err(|_| "Windows could not start the folder chooser.".to_string())?
        .join()
        .map_err(|_| "The folder chooser stopped unexpectedly.".to_string())?
}

#[cfg(not(windows))]
pub fn select_anytype_folder(
    _current_directory: Option<String>,
    _owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    Err("The Anytype folder chooser is supported on Windows.".into())
}

#[cfg(not(windows))]
pub fn select_folder(
    _current_directory: Option<String>,
    _title: String,
    _button_label: String,
    _owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    Err("The folder chooser is supported on Windows.".into())
}

#[cfg(windows)]
fn shell_parsing_path(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(&path).to_string()
    }
}

#[cfg(windows)]
fn set_initial_folder(
    dialog: &windows::Win32::UI::Shell::IFileOpenDialog,
    current_directory: Option<&str>,
) -> windows::core::Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName};

    let Some(path) = current_directory
        .filter(|value| !value.trim().is_empty() && !value.contains('\0'))
        .and_then(|value| std::path::Path::new(value).canonicalize().ok())
        .filter(|path| path.is_dir())
    else {
        return Ok(());
    };
    // The Shell parser rejects the verbatim prefixes returned by canonicalize.
    let parsing_name = HSTRING::from(shell_parsing_path(&path));
    let item: IShellItem = unsafe { SHCreateItemFromParsingName(&parsing_name, None) }?;
    unsafe { dialog.SetFolder(&item) }
}

#[cfg(windows)]
fn select_folder_sta(
    current_directory: Option<String>,
    title: &str,
    button_label: &str,
    owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    use std::path::PathBuf;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{HWND, RPC_E_CHANGED_MODE};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog,
        FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH,
    };

    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    struct ShellPath(windows::core::PWSTR);
    impl Drop for ShellPath {
        fn drop(&mut self) {
            unsafe { CoTaskMemFree(Some(self.0 .0.cast())) };
        }
    }

    let init_result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if init_result == RPC_E_CHANGED_MODE {
        return Err("Windows could not prepare a folder chooser on this thread.".into());
    }
    init_result
        .ok()
        .map_err(|error| format!("Windows could not initialize the folder chooser: {error}"))?;
    let _apartment = ComApartment;

    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| format!("Windows could not create the folder chooser: {error}"))?;

    let options = unsafe { dialog.GetOptions() }
        .map_err(|error| format!("Windows could not configure the folder chooser: {error}"))?;
    let options = options.0
        | FOS_PICKFOLDERS.0
        | FOS_FORCEFILESYSTEM.0
        | FOS_PATHMUSTEXIST.0
        | FOS_NOCHANGEDIR.0;
    unsafe { dialog.SetOptions(windows::Win32::UI::Shell::FILEOPENDIALOGOPTIONS(options)) }
        .map_err(|error| format!("Windows could not configure the folder chooser: {error}"))?;

    let title = HSTRING::from(title);
    let button = HSTRING::from(button_label);
    unsafe { dialog.SetTitle(&title) }
        .map_err(|error| format!("Windows could not title the folder chooser: {error}"))?;
    unsafe { dialog.SetOkButtonLabel(&button) }
        .map_err(|error| format!("Windows could not configure the folder chooser: {error}"))?;

    // An unavailable starting location must not prevent choosing another folder.
    let _ = set_initial_folder(&dialog, current_directory.as_deref());

    let owner = owner_handle.map(|handle| HWND(handle as *mut _));
    if let Err(error) = unsafe { dialog.Show(owner) } {
        if error.code().0 as u32 == 0x8007_04C7 {
            return Ok(None);
        }
        return Err(format!("The Anytype folder chooser failed: {error}"));
    }

    let item = unsafe { dialog.GetResult() }
        .map_err(|error| format!("Windows could not read the selected Anytype folder: {error}"))?;
    let display_name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }
        .map_err(|error| format!("Windows could not read the selected Anytype folder: {error}"))?;
    if display_name.is_null() {
        return Err("Windows returned an empty Anytype folder path.".into());
    }
    let display_name = ShellPath(display_name);
    let selected = unsafe { display_name.0.to_string() }
        .map_err(|_| "Windows returned an invalid Anytype folder path.".to_string())?;
    let selected_path = PathBuf::from(&selected)
        .canonicalize()
        .map_err(|_| "The selected Anytype folder is no longer available.".to_string())?;
    if !selected_path.is_dir() {
        return Err("The selected Anytype path is not a folder.".into());
    }
    Ok(Some(selected))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH};

    #[test]
    fn shell_paths_preserve_drive_and_unc_locations() {
        assert_eq!(shell_parsing_path(Path::new(r"\\?\C:\Anytype folder")), r"C:\Anytype folder");
        assert_eq!(shell_parsing_path(Path::new(r"\\?\UNC\server\share\Anytype")), r"\\server\share\Anytype");
        assert_eq!(shell_parsing_path(Path::new(r"C:\Anytype")), r"C:\Anytype");
    }

    #[test]
    fn native_shell_accepts_canonical_fixture_folder_without_showing_a_dialog() {
        std::thread::spawn(|| {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok().unwrap();
            struct Apartment;
            impl Drop for Apartment {
                fn drop(&mut self) { unsafe { CoUninitialize() }; }
            }
            let _apartment = Apartment;
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent().unwrap().join(".test-run")
                .join(format!("folder-picker-{}-Anytype ü", std::process::id()));
            std::fs::create_dir_all(&fixture).unwrap();
            let dialog: IFileOpenDialog = unsafe {
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            }.unwrap();
            let canonical = fixture.canonicalize().unwrap();
            set_initial_folder(&dialog, canonical.to_str()).unwrap();
            let selected = unsafe { dialog.GetFolder() }.unwrap();
            let name = unsafe { selected.GetDisplayName(SIGDN_FILESYSPATH) }.unwrap();
            let actual = unsafe { name.to_string() }.unwrap();
            unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(name.0.cast())) };
            assert_eq!(PathBuf::from(actual).canonicalize().unwrap(), canonical);
            set_initial_folder(&dialog, Some("missing folder\0")).unwrap();
            std::fs::remove_dir_all(fixture).unwrap();
        }).join().unwrap();
    }
}
