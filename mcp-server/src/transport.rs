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
                    ERROR_NO_DATA | ERROR_BROKEN_PIPE => {
                        // A canceled typeahead can close its handle before our polling accept.
                        // Reset this instance instead of terminating the entire bridge listener.
                        if DisconnectNamedPipe(self.0)==0 && GetLastError()!=ERROR_PIPE_NOT_CONNECTED {
                            return Err(io::Error::last_os_error());
                        }
                        Ok(false)
                    },
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
                    return Err(io::Error::last_os_error());
                }
                let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
                if SetNamedPipeHandleState(handle, &mode, ptr::null(), ptr::null()) == 0 {
                    CloseHandle(handle);
                    return Err(io::Error::last_os_error());
                }
                Ok(Self(handle))
            }
        }
        pub fn connect_wait(cancel:&AtomicBool, deadline:Instant)->io::Result<Self> {
            let startup_deadline = deadline.min(Instant::now() + Duration::from_secs(2));
            loop {
                stopped(cancel,deadline)?;
                match Self::connect() {
                    Ok(pipe)=>return Ok(pipe),
                    Err(error) if error.raw_os_error()==Some(ERROR_PIPE_BUSY as i32)=>{
                        // The listener creates another instance after accepting the previous one.
                        std::thread::sleep(Duration::from_millis(10));
                    },
                    Err(error) if error.raw_os_error()==Some(ERROR_FILE_NOT_FOUND as i32)
                        && Instant::now() < startup_deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    },
                    Err(_)=>return Err(io::Error::new(io::ErrorKind::NotConnected,"BlobUI is unavailable. Start BlobUI and enable Anyblob in Integrations > Anyblob.")),
                }
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
#[cfg(all(test,windows))]
mod tests {
    use super::*;
    #[test]
    fn aborted_typeahead_connections_do_not_poison_the_listener() {
        std::env::set_var("ANYBLOB_BRIDGE_INSTANCE",format!("fixture-aborted-{}",std::process::id()));
        let listener=Pipe::listen(true).unwrap();
        for _ in 0..3 {
            let client=Pipe::connect().unwrap();drop(client);
            assert!(!listener.accept().expect("A client that leaves before accept must reset the listener"));
            assert!(!listener.accept().unwrap()); // Re-arm the disconnected instance on the next poll.
        }
        let client=Pipe::connect().unwrap();assert!(listener.accept().unwrap());
        let cancel=AtomicBool::new(false);
        assert_eq!(Pipe::connect_wait(&cancel,Instant::now()+Duration::from_millis(20)).err().unwrap().kind(),io::ErrorKind::TimedOut);
        let waiting=std::thread::spawn(||Pipe::connect_wait(&AtomicBool::new(false),Instant::now()+Duration::from_secs(5)));
        let next=Pipe::listen(false).unwrap();
        let second=waiting.join().unwrap().expect("Concurrent searches wait for the next pipe instance");
        assert!(next.accept().unwrap());
        cancel.store(true,Ordering::Release);
        assert_eq!(Pipe::connect_wait(&cancel,Instant::now()+Duration::from_secs(5)).err().unwrap().kind(),io::ErrorKind::TimedOut);
        drop((second,next,client,listener));
        // A temporarily absent listener may recover without replaying a request.
        let waiting = std::thread::spawn(|| Pipe::connect_wait(
            &AtomicBool::new(false), Instant::now() + Duration::from_secs(3)));
        std::thread::sleep(Duration::from_millis(50));
        let recovered = Pipe::listen(true).unwrap();
        let client = waiting.join().unwrap().expect("Wait for a restarting bridge");
        assert!(recovered.accept().unwrap());
        drop((client, recovered));
    }
}
pub fn call(request: &crate::Request, cancel: &AtomicBool) -> Result<crate::Response, String> {
    call_with_approval(request, cancel, &|_, _| {
        Err("Update Codex or use a client supporting form elicitation.".into())
    })
}
pub fn call_with_approval(
    request: &crate::Request,
    cancel: &AtomicBool,
    approve: &impl Fn(&str, Duration) -> Result<bool, String>,
) -> Result<crate::Response, String> {
    #[cfg(windows)]
    {
        let deadline = Instant::now() + CALL_TIMEOUT;
        let pipe = Pipe::connect_wait(cancel, deadline).map_err(|e| e.to_string())?;
        pipe.send(
            &serde_json::to_vec(
                &serde_json::json!({"version":crate::approval::VERSION,"request":request}),
            )
            .map_err(|_| "Invalid bridge request.")?,
            cancel,
            deadline,
        )
        .map_err(|e| e.to_string())?;
        loop {
            let frame: crate::approval::Frame =
                serde_json::from_slice(&pipe.receive(cancel, deadline).map_err(|e| e.to_string())?)
                    .map_err(|_| crate::approval::UPDATE)?;
            match frame {
                crate::approval::Frame::Response { version, response }
                    if version == crate::approval::VERSION =>
                {
                    if response.result.is_none() && response.error.is_none() {
                        return Err(crate::approval::UPDATE.into());
                    }
                    return Ok(response)
                }
                crate::approval::Frame::ApprovalRequest {
                    version,
                    operation_id,
                    approval_id,
                    message,
                } if version == crate::approval::VERSION && operation_id == request.id => {
                    let accepted =
                        approve(&message, deadline.saturating_duration_since(Instant::now()))?;
                    let decision = crate::approval::Frame::ApprovalDecision {
                        version,
                        operation_id,
                        approval_id,
                        accepted,
                    };
                    pipe.send(
                        &serde_json::to_vec(&decision).map_err(|_| "Invalid decision")?,
                        cancel,
                        deadline,
                    )
                    .map_err(|e| e.to_string())?;
                }
                _ => return Err(crate::approval::UPDATE.into()),
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (request, cancel, approve);
        Err("Anyblob requires local Windows execution alongside BlobUI.".into())
    }
}
