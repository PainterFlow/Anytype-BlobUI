//! Windows transport is separate from stdio MCP and the typed operations.
use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
pub const CALL_TIMEOUT: Duration = Duration::from_secs(180);
fn stopped(cancel: &AtomicBool, deadline: Instant) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Request cancelled or timed out.",
        ))
    } else {
        Ok(())
    }
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::{Pipes::*, Threading::*},
    };
    pub struct Pipe(HANDLE);
    unsafe impl Send for Pipe {}
    impl Drop for Pipe {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn sid() -> io::Result<String> {
        unsafe {
            let mut token = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            let token = Pipe(token);
            let mut size = 0;
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size);
            let mut data = vec![0usize; (size as usize + 7) / 8];
            if GetTokenInformation(
                token.0,
                TokenUser,
                data.as_mut_ptr().cast(),
                size,
                &mut size,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let mut text = ptr::null_mut();
            if ConvertSidToStringSidW((*(data.as_ptr().cast::<TOKEN_USER>())).User.Sid, &mut text)
                == 0
            {
                return Err(io::Error::last_os_error());
            }
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            Ok(result)
        }
    }
    fn name() -> io::Result<Vec<u16>> {
        // An explicit instance keeps automated fixtures away from a user's bridge.
        let instance =
            std::env::var("ANYBLOB_BRIDGE_INSTANCE").unwrap_or_else(|_| "default".into());
        if instance.is_empty()
            || instance.len() > 64
            || !instance
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid bridge instance.",
            ));
        }
        Ok(wide(&format!(
            r"\\.\pipe\BlobUI.Anyblob.{}.{}",
            sid()?,
            instance
        )))
    }
    impl Pipe {
        pub fn listen(first: bool) -> io::Result<Self> {
            unsafe {
                let acl = wide(&format!("D:P(A;;GA;;;{})", sid()?));
                let mut descriptor = ptr::null_mut();
                if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    acl.as_ptr(),
                    1,
                    &mut descriptor,
                    ptr::null_mut(),
                ) == 0
                {
                    return Err(io::Error::last_os_error());
                }
                let security = SECURITY_ATTRIBUTES {
                    nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: descriptor,
                    bInheritHandle: 0,
                };
                let handle = CreateNamedPipeW(
                    name()?.as_ptr(),
                    PIPE_ACCESS_DUPLEX
                        | if first {
                            FILE_FLAG_FIRST_PIPE_INSTANCE
                        } else {
                            0
                        },
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    16,
                    65536,
                    65536,
                    0,
                    &security,
                );
                LocalFree(descriptor);
                if handle == INVALID_HANDLE_VALUE {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(Self(handle))
                }
            }
        }
        pub fn accept(&self) -> io::Result<bool> {
            unsafe {
                if ConnectNamedPipe(self.0, ptr::null_mut()) != 0 {
                    return Ok(true);
                }
                match GetLastError() {
                    ERROR_PIPE_CONNECTED => Ok(true),
                    ERROR_PIPE_LISTENING => Ok(false),
                    _ => Err(io::Error::last_os_error()),
                }
            }
        }
        pub fn connect() -> io::Result<Self> {
            unsafe {
                let handle = CreateFileW(
                    name()?.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    ptr::null(),
                    OPEN_EXISTING,
                    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    ptr::null_mut(),
                );
                if handle == INVALID_HANDLE_VALUE {
                    return Err(io::Error::new(io::ErrorKind::NotConnected,"BlobUI is unavailable. Start BlobUI and enable Anyblob in Integrations > Anyblob."));
                }
                let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
                if SetNamedPipeHandleState(handle, &mode, ptr::null(), ptr::null()) == 0 {
                    CloseHandle(handle);
                    return Err(io::Error::last_os_error());
                }
                Ok(Self(handle))
            }
        }
        pub fn connected(&self) -> bool {
            unsafe {
                PeekNamedPipe(
                    self.0,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ) != 0
            }
        }
        fn transfer(
            &self,
            bytes: &mut [u8],
            write: bool,
            cancel: &AtomicBool,
            deadline: Instant,
        ) -> io::Result<()> {
            let mut offset = 0;
            while offset < bytes.len() {
                stopped(cancel, deadline)?;
                let mut count = 0;
                let ok = unsafe {
                    if write {
                        WriteFile(
                            self.0,
                            bytes[offset..].as_ptr(),
                            (bytes.len() - offset).min(65536) as u32,
                            &mut count,
                            ptr::null_mut(),
                        )
                    } else {
                        ReadFile(
                            self.0,
                            bytes[offset..].as_mut_ptr(),
                            (bytes.len() - offset).min(65536) as u32,
                            &mut count,
                            ptr::null_mut(),
                        )
                    }
                };
                if ok == 0 {
                    let error = unsafe { GetLastError() };
                    if error != ERROR_NO_DATA && error != ERROR_PIPE_BUSY {
                        return Err(io::Error::last_os_error());
                    }
                    if !self.connected() {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "Bridge disconnected.",
                        ));
                    }
                }
                offset += count as usize;
                if count == 0 {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            Ok(())
        }
        pub fn send(&self, bytes: &[u8], cancel: &AtomicBool, deadline: Instant) -> io::Result<()> {
            if bytes.len() > MAX_FRAME {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Bridge response too large.",
                ));
            }
            self.transfer(
                &mut (bytes.len() as u32).to_le_bytes(),
                true,
                cancel,
                deadline,
            )?;
            self.transfer(&mut bytes.to_vec(), true, cancel, deadline)
        }
        pub fn receive(&self, cancel: &AtomicBool, deadline: Instant) -> io::Result<Vec<u8>> {
            let mut size = [0; 4];
            self.transfer(&mut size, false, cancel, deadline)?;
            let size = u32::from_le_bytes(size) as usize;
            if size > MAX_FRAME {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Bridge frame too large.",
                ));
            }
            let mut data = vec![0; size];
            self.transfer(&mut data, false, cancel, deadline)?;
            Ok(data)
        }
    }
}
#[cfg(windows)]
pub use windows::Pipe;
pub fn call(request: &crate::Request, cancel: &AtomicBool) -> Result<crate::Response, String> {
    #[cfg(windows)]
    {
        let deadline = Instant::now() + CALL_TIMEOUT;
        let pipe = Pipe::connect().map_err(|e| e.to_string())?;
        pipe.send(
            &serde_json::to_vec(request).map_err(|_| "Invalid bridge request.")?,
            cancel,
            deadline,
        )
        .map_err(|e| e.to_string())?;
        serde_json::from_slice(&pipe.receive(cancel, deadline).map_err(|e| e.to_string())?)
            .map_err(|_| "Invalid BlobUI response.".into())
    }
    #[cfg(not(windows))]
    {
        let _ = (request, cancel);
        Err("Anyblob requires local Windows execution alongside BlobUI.".into())
    }
}
