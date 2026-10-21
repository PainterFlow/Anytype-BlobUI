#[cfg(windows)]
pub fn select_anytype_folder(
    current_directory: Option<String>,
    owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    std::thread::Builder::new()
        .name("blobui-folder-dialog".into())
        .spawn(move || select_anytype_folder_sta(current_directory, owner_handle))
        .map_err(|_| "Windows could not start the Anytype folder chooser.".to_string())?
        .join()
        .map_err(|_| "The Anytype folder chooser stopped unexpectedly.".to_string())?
}

#[cfg(not(windows))]
pub fn select_anytype_folder(
    _current_directory: Option<String>,
    _owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    Err("The Anytype folder chooser is supported on Windows.".into())
}

#[cfg(windows)]
fn select_anytype_folder_sta(
    current_directory: Option<String>,
    owner_handle: Option<isize>,
) -> Result<Option<String>, String> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{HWND, RPC_E_CHANGED_MODE};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName,
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

    let title = HSTRING::from("Select Anytype Folder");
    let button = HSTRING::from("Select folder");
    unsafe { dialog.SetTitle(&title) }
        .map_err(|error| format!("Windows could not title the folder chooser: {error}"))?;
    unsafe { dialog.SetOkButtonLabel(&button) }
        .map_err(|error| format!("Windows could not configure the folder chooser: {error}"))?;

    let initial_directory = current_directory
        .filter(|value| !value.trim().is_empty() && !value.contains('\0'))
        .and_then(|value| PathBuf::from(value).canonicalize().ok())
        .filter(|path| path.is_dir());
    if let Some(path) = initial_directory {
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let item: IShellItem = unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) }
            .map_err(|error| {
                format!("Windows could not open the current Anytype folder: {error}")
            })?;
        unsafe { dialog.SetFolder(&item) }.map_err(|error| {
            format!("Windows could not open the current Anytype folder: {error}")
        })?;
    }

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
