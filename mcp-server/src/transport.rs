//! Windows transport is separate from stdio MCP and the typed operations.
use std::{
    io::{self, Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
pub const CALL_TIMEOUT: Duration = Duration::from_secs(180);

pub trait Bridge {
    fn connected(&self) -> bool;
    fn send(&self, bytes: &[u8], cancel: &AtomicBool, deadline: Instant) -> io::Result<()>;
    fn receive(&self, cancel: &AtomicBool, deadline: Instant) -> io::Result<Vec<u8>>;
}

fn tunnel_endpoint(value: &str) -> io::Result<String> {
    let mut url = reqwest::Url::parse(value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid remote Anyblob URL."))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Remote Anyblob requires an HTTPS tunnel URL without credentials, query, or fragment.",
        ));
    }
    if !url.path().ends_with("/anyblob") {
        let path = format!("{}/anyblob", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url.to_string())
}

fn valid_remote_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub struct TcpBridge {
    reader: Mutex<TcpStream>,
    writer: Mutex<TcpStream>,
    connected: AtomicBool,
}

impl TcpBridge {
    pub fn from_stream(stream: TcpStream) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_millis(50)))?;
        stream.set_write_timeout(Some(Duration::from_millis(50)))?;
        let reader = stream.try_clone()?;
        Ok(Self {
            reader: Mutex::new(reader),
            writer: Mutex::new(stream),
            connected: AtomicBool::new(true),
        })
    }
}

fn read_exact_until(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    cancel: &AtomicBool,
    deadline: Instant,
) -> io::Result<()> {
    let mut read = 0;
    while read < buffer.len() {
        stopped(cancel, deadline)?;
        match stream.read(&mut buffer[read..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Bridge disconnected.",
                ))
            }
            Ok(count) => read += count,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn write_all_until(
    stream: &mut TcpStream,
    buffer: &[u8],
    cancel: &AtomicBool,
    deadline: Instant,
) -> io::Result<()> {
    let mut written = 0;
    while written < buffer.len() {
        stopped(cancel, deadline)?;
        match stream.write(&buffer[written..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "Bridge disconnected.",
                ))
            }
            Ok(count) => written += count,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

impl Bridge for TcpBridge {
    fn connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    fn send(&self, bytes: &[u8], cancel: &AtomicBool, deadline: Instant) -> io::Result<()> {
        if bytes.len() > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Bridge frame too large.",
            ));
        }
        let result = (|| {
            let mut stream = self
                .writer
                .lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Bridge stream unavailable."))?;
            write_all_until(
                &mut stream,
                &(bytes.len() as u32).to_le_bytes(),
                cancel,
                deadline,
            )?;
            write_all_until(&mut stream, bytes, cancel, deadline)
        })();
        if result.is_err() {
            self.connected.store(false, Ordering::Release);
        }
        result
    }

    fn receive(&self, cancel: &AtomicBool, deadline: Instant) -> io::Result<Vec<u8>> {
        let result = (|| {
            let mut stream = self
                .reader
                .lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Bridge stream unavailable."))?;
            let mut header = [0; 4];
            read_exact_until(&mut stream, &mut header, cancel, deadline)?;
            let size = u32::from_le_bytes(header) as usize;
            if size > MAX_FRAME {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Bridge frame too large.",
                ));
            }
            let mut bytes = vec![0; size];
            read_exact_until(&mut stream, &mut bytes, cancel, deadline)?;
            Ok(bytes)
        })();
        if result.is_err() {
            self.connected.store(false, Ordering::Release);
        }
        result
    }
}
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
                        if DisconnectNamedPipe(self.0) == 0
                            && GetLastError() != ERROR_PIPE_NOT_CONNECTED
                        {
                            return Err(io::Error::last_os_error());
                        }
                        Ok(false)
                    }
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
                    let error = io::Error::last_os_error();
                    CloseHandle(handle);
                    return Err(error);
                }
                Ok(Self(handle))
            }
        }
        pub fn connect_wait(cancel: &AtomicBool, deadline: Instant) -> io::Result<Self> {
            let startup_deadline = deadline.min(Instant::now() + Duration::from_secs(2));
            loop {
                stopped(cancel, deadline)?;
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
#[cfg(windows)]
impl Bridge for Pipe {
    fn connected(&self) -> bool {
        Pipe::connected(self)
    }
    fn send(&self, bytes: &[u8], cancel: &AtomicBool, deadline: Instant) -> io::Result<()> {
        Pipe::send(self, bytes, cancel, deadline)
    }
    fn receive(&self, cancel: &AtomicBool, deadline: Instant) -> io::Result<Vec<u8>> {
        Pipe::receive(self, cancel, deadline)
    }
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn aborted_typeahead_connections_do_not_poison_the_listener() {
        std::env::set_var(
            "ANYBLOB_BRIDGE_INSTANCE",
            format!("fixture-aborted-{}", std::process::id()),
        );
        let listener = Pipe::listen(true).unwrap();
        for _ in 0..3 {
            let client = Pipe::connect().expect(
                "Connect to the same-user fixture pipe. On access denied, run the test in a normal Windows session outside the restricted sandbox.",
            );
            drop(client);
            assert!(!listener
                .accept()
                .expect("A client that leaves before accept must reset the listener"));
            assert!(!listener.accept().unwrap()); // Re-arm the disconnected instance on the next poll.
        }
        let client = Pipe::connect().unwrap();
        assert!(listener.accept().unwrap());
        let cancel = AtomicBool::new(false);
        assert_eq!(
            Pipe::connect_wait(&cancel, Instant::now() + Duration::from_millis(20))
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::TimedOut
        );
        let waiting = std::thread::spawn(|| {
            Pipe::connect_wait(
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(5),
            )
        });
        let next = Pipe::listen(false).unwrap();
        let second = waiting
            .join()
            .unwrap()
            .expect("Concurrent searches wait for the next pipe instance");
        assert!(next.accept().unwrap());
        cancel.store(true, Ordering::Release);
        assert_eq!(
            Pipe::connect_wait(&cancel, Instant::now() + Duration::from_secs(5))
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::TimedOut
        );
        drop((second, next, client, listener));
        // A temporarily absent listener may recover without replaying a request.
        let waiting = std::thread::spawn(|| {
            Pipe::connect_wait(
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(3),
            )
        });
        std::thread::sleep(Duration::from_millis(50));
        let recovered = Pipe::listen(true).unwrap();
        let client = waiting
            .join()
            .unwrap()
            .expect("Wait for a restarting bridge");
        assert!(recovered.accept().unwrap());
        drop((client, recovered));
    }
}
struct HttpBridge {
    client: reqwest::blocking::Client,
    endpoint: String,
    token: String,
    response: Mutex<Option<Vec<u8>>>,
    connected: AtomicBool,
}

fn decode_http_bridge_frame(bytes: &[u8]) -> io::Result<Vec<u8>> {
    if bytes.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Remote bridge returned a truncated frame.",
        ));
    }
    let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    if size == 0 || size > MAX_FRAME || bytes.len() != size + 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Remote bridge returned an invalid frame length.",
        ));
    }
    Ok(bytes[4..].to_vec())
}

impl HttpBridge {
    fn connect() -> io::Result<Self> {
        let address = std::env::var("ANYBLOB_REMOTE_ADDRESS").map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Remote Anyblob URL is not configured.",
            )
        })?;
        let endpoint = tunnel_endpoint(&address)?;
        let token = std::env::var("ANYBLOB_REMOTE_TOKEN").map_err(|_| {
            io::Error::new(io::ErrorKind::NotFound, "ANYBLOB_REMOTE_TOKEN is not set.")
        })?;
        if !valid_remote_token(&token) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ANYBLOB_REMOTE_TOKEN must be a 64-character hexadecimal key.",
            ));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(CALL_TIMEOUT)
            .build()
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
        Ok(Self {
            client,
            endpoint,
            token,
            response: Mutex::new(None),
            connected: AtomicBool::new(true),
        })
    }
}

impl Bridge for HttpBridge {
    fn connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    fn send(&self, bytes: &[u8], cancel: &AtomicBool, deadline: Instant) -> io::Result<()> {
        stopped(cancel, deadline)?;
        if bytes.len() > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Bridge frame too large.",
            ));
        }
        let timeout = deadline.saturating_duration_since(Instant::now());
        let result = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .timeout(timeout)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(bytes.to_vec())
            .send()
            .and_then(|response| {
                if !response.status().is_success() {
                    return Err(response.error_for_status().unwrap_err());
                }
                response.bytes()
            })
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
        let result = decode_http_bridge_frame(&result)?;
        *self
            .response
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Bridge response unavailable."))? =
            Some(result.to_vec());
        Ok(())
    }

    fn receive(&self, cancel: &AtomicBool, deadline: Instant) -> io::Result<Vec<u8>> {
        stopped(cancel, deadline)?;
        self.response
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Bridge response unavailable."))?
            .take()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Remote bridge returned no response.",
                )
            })
    }
}

pub fn sync_project_catalog(sync: crate::ProjectSync) -> Result<(), String> {
    let bridge = HttpBridge::connect().map_err(|error| error.to_string())?;
    let cancel = AtomicBool::new(false);
    sync_project_catalog_on_bridge(
        &bridge,
        &sync,
        &cancel,
        Instant::now() + Duration::from_secs(15),
    )
}

fn sync_project_catalog_on_bridge(
    bridge: &impl Bridge,
    sync: &crate::ProjectSync,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<(), String> {
    let request = serde_json::to_vec(&serde_json::json!({
        "version": crate::approval::VERSION,
        "projectSync": sync,
    }))
    .map_err(|_| "Could not encode the remote project catalog.")?;
    bridge
        .send(&request, cancel, deadline)
        .map_err(|_| "Could not send the remote project catalog.")?;
    let response = bridge
        .receive(cancel, deadline)
        .map_err(|_| "Could not receive the remote project catalog result.")?;
    let frame: crate::approval::Frame = serde_json::from_slice(&response)
        .map_err(|_| crate::approval::UPDATE)?;
    match frame {
        crate::approval::Frame::Response { version, response }
            if version == crate::approval::VERSION =>
        {
            if response.result.as_ref().is_some_and(|result| result["synced"] == true)
                && response.error.is_none()
            {
                Ok(())
            } else {
                Err(response.error.unwrap_or_else(|| "Cloud project catalog sync was not confirmed.".into()))
            }
        }
        _ => Err(crate::approval::UPDATE.into()),
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
    let deadline = Instant::now() + CALL_TIMEOUT;
    if std::env::var_os("ANYBLOB_REMOTE_ADDRESS").is_some() {
        let bridge = HttpBridge::connect().map_err(|e| e.to_string())?;
        return call_on_bridge(&bridge, request, cancel, deadline, approve);
    }
    #[cfg(windows)]
    {
        let pipe = Pipe::connect_wait(cancel, deadline).map_err(|e| e.to_string())?;
        call_on_bridge(&pipe, request, cancel, deadline, approve)
    }
    #[cfg(not(windows))]
    {
        let _ = (request, cancel, approve);
        Err(
            "Set ANYBLOB_REMOTE_ADDRESS to the HTTPS Cloudflare Tunnel URL before using remote Anyblob."
                .into(),
        )
    }
}

fn call_on_bridge(
    bridge: &impl Bridge,
    request: &crate::Request,
    cancel: &AtomicBool,
    deadline: Instant,
    approve: &impl Fn(&str, Duration) -> Result<bool, String>,
) -> Result<crate::Response, String> {
    bridge
        .send(
            &serde_json::to_vec(
                &serde_json::json!({"version":crate::approval::VERSION,"request":request}),
            )
            .map_err(|_| "Invalid bridge request.")?,
            cancel,
            deadline,
        )
        .map_err(|e| e.to_string())?;
    loop {
        let frame: crate::approval::Frame = serde_json::from_slice(
            &bridge
                .receive(cancel, deadline)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|_| crate::approval::UPDATE)?;
        match frame {
            crate::approval::Frame::Response { version, response }
                if version == crate::approval::VERSION =>
            {
                if response.result.is_none() && response.error.is_none() {
                    return Err(crate::approval::UPDATE.into());
                }
                return Ok(response);
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
                bridge
                    .send(
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

#[cfg(test)]
mod tcp_tests {
    use super::{decode_http_bridge_frame, sync_project_catalog_on_bridge, tunnel_endpoint, valid_remote_token, Bridge, TcpBridge, MAX_FRAME};
    use std::io::{self, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{atomic::AtomicBool, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    fn streams() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let client = TcpStream::connect(address).unwrap();
        let server = listener.accept().unwrap().0;
        (client, server)
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(3)
    }

    #[test]
    fn remote_http_response_decodes_the_tunnel_adapter_frame() {
        let payload = br#"{"version":2,"response":{"result":{"synced":true}}}"#;
        let mut response = (payload.len() as u32).to_le_bytes().to_vec();
        response.extend_from_slice(payload);
        assert_eq!(decode_http_bridge_frame(&response).unwrap(), payload);
        for invalid in [
            Vec::new(),
            vec![1, 0, 0, 0],
            [u32::MAX.to_le_bytes().as_slice(), b"x"].concat(),
            [((payload.len() + 1) as u32).to_le_bytes().as_slice(), payload].concat(),
            [((payload.len() - 1) as u32).to_le_bytes().as_slice(), payload].concat(),
        ] {
            assert!(decode_http_bridge_frame(&invalid).is_err());
        }
    }

    struct FixtureBridge {
        sent: Mutex<Option<Vec<u8>>>,
        response: Mutex<Option<Vec<u8>>>,
    }
    impl Bridge for FixtureBridge {
        fn connected(&self) -> bool { true }
        fn send(&self, bytes: &[u8], _: &AtomicBool, _: Instant) -> io::Result<()> {
            *self.sent.lock().unwrap() = Some(bytes.to_vec());
            Ok(())
        }
        fn receive(&self, _: &AtomicBool, _: Instant) -> io::Result<Vec<u8>> {
            self.response.lock().unwrap().take().ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))
        }
    }

    #[test]
    fn project_catalog_sync_uses_the_authenticated_versioned_bridge_envelope() {
        let response = serde_json::to_vec(&crate::approval::Frame::Response {
            version: crate::approval::VERSION,
            response: crate::Response { result: Some(serde_json::json!({"synced":true})), error: None },
        }).unwrap();
        let bridge = FixtureBridge { sent: Mutex::new(None), response: Mutex::new(Some(response)) };
        let sync = crate::ProjectSync {
            token: "a".repeat(64),
            host_id: "host_fixture".into(),
            computer_name: "Codex fixture".into(),
            projects: vec![crate::ProjectInfo { id: "project-id".into(), name: "Renamed".into() }],
        };
        assert!(sync_project_catalog_on_bridge(&bridge, &sync, &AtomicBool::new(false), deadline()).is_ok());
        let envelope: serde_json::Value = serde_json::from_slice(&bridge.sent.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(envelope["version"], crate::approval::VERSION);
        assert_eq!(envelope["projectSync"]["token"], sync.token);
        assert_eq!(envelope["projectSync"]["hostId"], "host_fixture");
        assert_eq!(envelope["projectSync"]["computerName"], "Codex fixture");
        assert_eq!(envelope["projectSync"]["projects"], serde_json::json!([{"id":"project-id","name":"Renamed"}]));
        assert!(envelope.get("request").is_none());
    }

    #[test]
    fn remote_url_requires_https_and_targets_only_the_bridge() {
        assert_eq!(
            tunnel_endpoint("https://example.trycloudflare.com").unwrap(),
            "https://example.trycloudflare.com/anyblob"
        );
        assert_eq!(
            tunnel_endpoint("https://anyblob.example.com/base").unwrap(),
            "https://anyblob.example.com/base/anyblob"
        );
        for invalid in [
            "http://example.trycloudflare.com",
            "https://user:pass@example.com",
            "https://example.com/?token=secret",
            "https://example.com/#secret",
        ] {
            assert_eq!(
                tunnel_endpoint(invalid).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn remote_token_requires_a_64_character_hex_key() {
        assert!(valid_remote_token(&"a".repeat(64)));
        assert!(!valid_remote_token("short"));
        assert!(!valid_remote_token(&"g".repeat(64)));
    }

    #[test]
    fn tcp_bridge_round_trips_a_length_prefixed_frame() {
        let (client_stream, server_stream) = streams();
        let client = TcpBridge::from_stream(client_stream).unwrap();
        let server = TcpBridge::from_stream(server_stream).unwrap();
        let server_task = thread::spawn(move || {
            let cancel = AtomicBool::new(false);
            let request = server.receive(&cancel, deadline()).unwrap();
            server.send(&request, &cancel, deadline()).unwrap();
            request
        });
        let cancel = AtomicBool::new(false);
        client.send(b"remote request", &cancel, deadline()).unwrap();
        assert_eq!(
            client.receive(&cancel, deadline()).unwrap(),
            b"remote request"
        );
        assert_eq!(server_task.join().unwrap(), b"remote request");
    }

    #[test]
    fn tcp_bridge_rejects_oversized_frames() {
        let (client_stream, _server_stream) = streams();
        let client = TcpBridge::from_stream(client_stream).unwrap();
        let cancel = AtomicBool::new(false);
        let oversized = vec![0; MAX_FRAME + 1];
        assert_eq!(
            client
                .send(&oversized, &cancel, deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );

        let (bridge_stream, mut peer) = streams();
        let bridge = TcpBridge::from_stream(bridge_stream).unwrap();
        peer.write_all(&((MAX_FRAME as u32 + 1).to_le_bytes()))
            .unwrap();
        assert_eq!(
            bridge.receive(&cancel, deadline()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
