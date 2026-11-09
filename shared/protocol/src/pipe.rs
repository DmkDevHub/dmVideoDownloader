//! Local Windows transport. Both ends verify the peer before exchanging data.
use crate::{frame, Code, Message, HOST_EXE, MAX_BYTES};
use sha2::{Digest, Sha256};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        LocalFree, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_NO_DATA, ERROR_PIPE_BUSY,
        ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, HANDLE, INVALID_HANDLE_VALUE,
    },
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    },
    Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_GENERIC_READ,
        FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
    },
    System::{
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
            GetNamedPipeServerProcessId, SetNamedPipeHandleState, PIPE_NOWAIT,
            PIPE_REJECT_REMOTE_CLIENTS,
        },
        Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};

pub const APP_EXES: [&str; 2] = ["VideoDownloader.exe", "dm-video-desktop.exe"];
const IO_LIMIT: Duration = Duration::from_secs(3);
const PAUSE: Duration = Duration::from_millis(15);

fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

pub fn canonical(path: &Path) -> io::Result<String> {
    Ok(std::fs::canonicalize(path)?
        .to_string_lossy()
        .to_lowercase())
}
fn pipe_name(root: &Path) -> io::Result<Vec<u16>> {
    let key = Sha256::digest(canonical(root)?.as_bytes());
    Ok(wide(std::ffi::OsStr::new(&format!(
        r"\\.\pipe\dm-video-downloader-{key:x}"
    ))))
}

fn user_sid(process: HANDLE) -> io::Result<String> {
    let mut token = null_mut();
    // Token user needs an aligned variable-sized buffer.
    unsafe {
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle::from_raw_handle(token);
        let mut size = 0;
        GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut size);
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut length = 0;
        while *sid.add(length) != 0 {
            length += 1;
        }
        let value = String::from_utf16_lossy(std::slice::from_raw_parts(sid, length));
        LocalFree(sid.cast());
        Ok(value)
    }
}

fn same_peer(pipe: &File, server: bool, sid: &str, expected: &[String]) -> bool {
    unsafe {
        let mut pid = 0;
        let result = if server {
            GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut pid)
        } else {
            GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut pid)
        };
        if result == 0 || pid == 0 {
            return false;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }
        let process = OwnedHandle::from_raw_handle(process);
        let mut path = vec![0u16; 32_768];
        let mut length = path.len() as u32;
        if QueryFullProcessImageNameW(process.as_raw_handle(), 0, path.as_mut_ptr(), &mut length)
            == 0
        {
            return false;
        }
        let actual = PathBuf::from(std::ffi::OsString::from_wide(&path[..length as usize]));
        user_sid(process.as_raw_handle()).is_ok_and(|value| value == sid)
            && canonical(&actual).is_ok_and(|value| expected.contains(&value))
    }
}

fn listener(name: &[u16], sid: &str) -> io::Result<File> {
    let descriptor = wide(std::ffi::OsStr::new(&format!("D:P(A;;GA;;;{sid})")));
    unsafe {
        let mut security = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            1,
            &mut security,
            null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let attrs = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security,
            bInheritHandle: 0,
        };
        let handle = CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            MAX_BYTES as u32 + 4,
            MAX_BYTES as u32 + 4,
            0,
            &attrs,
        );
        let error = io::Error::last_os_error();
        LocalFree(security);
        if handle == INVALID_HANDLE_VALUE {
            return Err(error);
        }
        Ok(File::from_raw_handle(handle))
    }
}

struct Timed<'a> {
    file: &'a mut File,
    until: Instant,
}
impl<'a> Timed<'a> {
    fn new(file: &'a mut File) -> Self {
        Self {
            file,
            until: Instant::now() + IO_LIMIT,
        }
    }
    fn pause(&self) -> io::Result<()> {
        if Instant::now() >= self.until {
            return Err(io::ErrorKind::TimedOut.into());
        }
        thread::sleep(PAUSE);
        Ok(())
    }
}
impl Read for Timed<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            if Instant::now() >= self.until {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let mut count = 0;
            let result = unsafe {
                ReadFile(
                    self.file.as_raw_handle(),
                    bytes.as_mut_ptr(),
                    bytes.len() as u32,
                    &mut count,
                    null_mut(),
                )
            };
            let result = if result == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(count as usize)
            };
            match result {
                Err(error) if error.raw_os_error() == Some(ERROR_NO_DATA as i32) => self.pause()?,
                Err(error) if error.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) => {
                    return Ok(0)
                }
                result => return result,
            }
        }
    }
}
impl Write for Timed<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            if Instant::now() >= self.until {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let mut count = 0;
            let result = unsafe {
                WriteFile(
                    self.file.as_raw_handle(),
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    &mut count,
                    null_mut(),
                )
            };
            let result = if result == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(count as usize)
            };
            match result {
                Ok(0) => self.pause()?,
                Err(error) if error.raw_os_error() == Some(ERROR_NO_DATA as i32) => self.pause()?,
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Server {
    file: File,
    sid: String,
    expected_host: PathBuf,
}
impl Server {
    pub fn bind(root: &Path) -> io::Result<Self> {
        let sid = user_sid(unsafe { GetCurrentProcess() })?;
        let file = listener(&pipe_name(root)?, &sid)?;
        Ok(Self {
            file,
            sid,
            expected_host: root.join(HOST_EXE),
        })
    }
    pub fn run(
        mut self,
        mut handle: impl FnMut(u64, &str, Message) -> Message,
        mut closed: impl FnMut(u64),
    ) {
        let mut connection = 0u64;
        loop {
            if unsafe { ConnectNamedPipe(self.file.as_raw_handle(), null_mut()) } != 0 {
                // In PIPE_NOWAIT this first success only makes the instance available.
                thread::sleep(PAUSE);
                continue;
            } else {
                match io::Error::last_os_error()
                    .raw_os_error()
                    .map(|code| code as u32)
                {
                    Some(ERROR_PIPE_CONNECTED) => {}
                    Some(ERROR_PIPE_LISTENING) => {
                        thread::sleep(PAUSE);
                        continue;
                    }
                    Some(ERROR_NO_DATA) => {
                        unsafe { DisconnectNamedPipe(self.file.as_raw_handle()) };
                        continue;
                    }
                    _ => break,
                }
            }
            connection += 1;
            if canonical(&self.expected_host)
                .is_ok_and(|path| same_peer(&self.file, true, &self.sid, &[path]))
            {
                let _ = self.session(connection, &mut handle);
                closed(connection);
            }
            unsafe { DisconnectNamedPipe(self.file.as_raw_handle()) };
        }
    }
    fn session(
        &mut self,
        connection: u64,
        handle: &mut impl FnMut(u64, &str, Message) -> Message,
    ) -> io::Result<()> {
        let mut origin = [0; 32];
        Timed::new(&mut self.file).read_exact(&mut origin)?;
        let id = std::str::from_utf8(&origin)
            .ok()
            .filter(|id| crate::extension_id(id))
            .ok_or(io::ErrorKind::InvalidData)?;
        while let Some(bytes) = frame::read(&mut Timed::new(&mut self.file))? {
            let reply = match Message::decode(&bytes) {
                Ok(message) => match frame::request_only(&message) {
                    Ok(()) => handle(connection, id, message),
                    Err(code) => Message::error(message.request_id, code),
                },
                Err(code) => Message::error(
                    crate::request_id_hint(&bytes).unwrap_or_else(|| "invalid".into()),
                    code,
                ),
            };
            frame::write(&mut Timed::new(&mut self.file), &reply)?;
        }
        Ok(())
    }
}

pub struct Client(File);
impl Client {
    pub fn connect(root: &Path, id: &str, wait: Duration) -> Result<Self, Code> {
        if !crate::extension_id(id) {
            return Err(Code::ExtensionIdMismatch);
        }
        let name = pipe_name(root).map_err(|_| Code::HostPathMismatch)?;
        let expected: Vec<_> = APP_EXES
            .iter()
            .filter_map(|name| canonical(&root.join(name)).ok())
            .collect();
        let sid = user_sid(unsafe { GetCurrentProcess() }).map_err(|_| Code::HostStartFailed)?;
        let until = Instant::now() + wait;
        let mut file = loop {
            // Avoid FILE_APPEND_DATA, which also grants creation of a pipe instance.
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    FILE_GENERIC_READ | FILE_WRITE_DATA | FILE_WRITE_ATTRIBUTES,
                    0,
                    null(),
                    OPEN_EXISTING,
                    0,
                    null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                break unsafe { File::from_raw_handle(handle) };
            }
            let code = io::Error::last_os_error()
                .raw_os_error()
                .map(|value| value as u32);
            if !matches!(code, Some(ERROR_PIPE_BUSY | ERROR_FILE_NOT_FOUND)) {
                return Err(Code::HostStartFailed);
            }
            if Instant::now() >= until {
                return Err(if code == Some(ERROR_FILE_NOT_FOUND) {
                    Code::AppNotRunning
                } else {
                    Code::IpcTimeout
                });
            }
            thread::sleep(PAUSE);
        };
        if !same_peer(&file, false, &sid, &expected) {
            return Err(Code::HostPathMismatch);
        }
        if unsafe { SetNamedPipeHandleState(file.as_raw_handle(), &PIPE_NOWAIT, null(), null()) }
            == 0
        {
            return Err(Code::HostStartFailed);
        }
        Timed::new(&mut file)
            .write_all(id.as_bytes())
            .map_err(|_| Code::IpcTimeout)?;
        Ok(Self(file))
    }
    pub fn exchange(&mut self, message: &Message) -> Result<Message, Code> {
        frame::write(&mut Timed::new(&mut self.0), message).map_err(|_| Code::IpcTimeout)?;
        let bytes = frame::read(&mut Timed::new(&mut self.0))
            .map_err(|_| Code::IpcTimeout)?
            .ok_or(Code::IpcTimeout)?;
        let reply = Message::decode(&bytes)?;
        if reply.request_id != message.request_id || frame::request_only(&reply).is_ok() {
            return Err(Code::InvalidPayload);
        }
        Ok(reply)
    }
}
