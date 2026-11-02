use crate::config::{Layout, Result};
use std::{
    cell::Cell,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, LocalFree, ERROR_NO_DATA, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING,
        INVALID_HANDLE_VALUE,
    },
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    },
    Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
    System::{
        Pipes::*,
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

fn sid() -> io::Result<String> {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = (|| {
            let mut size = 0;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size);
            // usize storage keeps TOKEN_USER correctly aligned.
            let mut storage = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(
                token,
                TokenUser,
                storage.as_mut_ptr().cast(),
                size,
                &mut size,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let user = &*storage.as_ptr().cast::<TOKEN_USER>();
            let mut text = std::ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let value = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            Ok(value)
        })();
        CloseHandle(token);
        result
    }
}

fn name(layout: &Layout) -> io::Result<Vec<u16>> {
    let digest = ring::digest::digest(
        &ring::digest::SHA256,
        layout.data.to_string_lossy().to_lowercase().as_bytes(),
    );
    let hash: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(r"\\.\pipe\fairway-{}-{hash}", sid()?)
        .encode_utf16()
        .chain(Some(0))
        .collect())
}

pub(in crate::cli) struct Stream {
    file: File,
    read_timeout: Cell<Option<Duration>>,
    write_timeout: Cell<Option<Duration>>,
}
impl Stream {
    fn new(file: File) -> Self {
        Self {
            file,
            read_timeout: Cell::new(Some(Duration::from_secs(2))),
            write_timeout: Cell::new(Some(Duration::from_secs(5))),
        }
    }
    pub fn set_read_timeout(&self, value: Option<Duration>) -> io::Result<()> {
        self.read_timeout.set(value);
        Ok(())
    }
    pub fn set_write_timeout(&self, value: Option<Duration>) -> io::Result<()> {
        self.write_timeout.set(value);
        Ok(())
    }
}
fn timed_out(start: Instant, timeout: Option<Duration>) -> io::Result<()> {
    if timeout.is_some_and(|timeout| start.elapsed() >= timeout) {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Supervisor pipe timed out.",
        ));
    }
    std::thread::sleep(Duration::from_millis(5));
    Ok(())
}
impl Read for &Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let start = Instant::now();
        loop {
            // Peek distinguishes an idle connected pipe from a disconnected peer.
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    self.file.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if available > 0 {
                return (&self.file).read(bytes);
            }
            timed_out(start, self.read_timeout.get())?;
        }
    }
}
impl Write for &Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let start = Instant::now();
        loop {
            match (&self.file).write(bytes) {
                Ok(0) => (),
                result => return result,
            }
            timed_out(start, self.write_timeout.get())?;
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Read for Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        (&*self).read(bytes)
    }
}
impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        (&*self).write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(in crate::cli) struct Listener {
    name: Vec<u16>,
    pending: File,
}
impl Listener {
    fn instance(name: &[u16], first: bool) -> io::Result<File> {
        let descriptor: Vec<u16> = format!("D:P(A;;GA;;;{})", sid()?)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let mut security = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                descriptor.as_ptr(),
                1,
                &mut security,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: security,
                bInheritHandle: 0,
            };
            let handle = CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX
                    | if first {
                        FILE_FLAG_FIRST_PIPE_INSTANCE
                    } else {
                        0
                    },
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                32,
                65536,
                65536,
                0,
                &attributes,
            );
            let error = io::Error::last_os_error();
            LocalFree(security);
            if handle == INVALID_HANDLE_VALUE {
                return Err(error);
            }
            Ok(File::from_raw_handle(handle))
        }
    }
    pub fn bind(layout: &Layout) -> Result<Self> {
        let name = name(layout).map_err(|e| e.to_string())?;
        let pending = Self::instance(&name, true).map_err(|e| e.to_string())?;
        Ok(Self { name, pending })
    }
    pub fn accept(&mut self) -> io::Result<Stream> {
        let connected =
            unsafe { ConnectNamedPipe(self.pending.as_raw_handle(), std::ptr::null_mut()) };
        if connected == 0 {
            let error = io::Error::last_os_error();
            match error.raw_os_error().map(|e| e as u32) {
                Some(ERROR_PIPE_CONNECTED) => (),
                Some(ERROR_PIPE_LISTENING) => return Err(io::ErrorKind::WouldBlock.into()),
                // A readiness probe may disconnect before accept. Retire that instance.
                Some(ERROR_NO_DATA) => {
                    self.pending = Self::instance(&self.name, false)?;
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                _ => return Err(error),
            }
        }
        let next = Self::instance(&self.name, false)?;
        Ok(Stream::new(std::mem::replace(&mut self.pending, next)))
    }
}
pub(in crate::cli) fn connect(layout: &Layout) -> io::Result<Stream> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let name = name(layout)?;
    let path = OsString::from_wide(&name[..name.len() - 1]);
    let start = Instant::now();
    let file = loop {
        match OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0x00100000 | 0x00010000)
            .open(&path)
        {
            Ok(file) => break file,
            Err(e) if e.raw_os_error() == Some(231) && start.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(e),
        }
    };
    let mode = PIPE_NOWAIT | PIPE_READMODE_BYTE;
    if unsafe {
        SetNamedPipeHandleState(
            file.as_raw_handle(),
            &mode,
            std::ptr::null(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(Stream::new(file))
}
pub(in crate::cli) fn lock(layout: &Layout) -> Result<Option<File>> {
    lock_path(&layout.data.join("supervisor.lock"))
}
pub(in crate::cli) fn lock_path(path: &std::path::Path) -> Result<Option<File>> {
    file_lock(path, 0)
}
pub(in crate::cli) fn shared_lock(path: &std::path::Path) -> Result<Option<File>> {
    file_lock(path, 3) // FILE_SHARE_READ | FILE_SHARE_WRITE: probes can coexist.
}
fn file_lock(path: &std::path::Path, share_mode: u32) -> Result<Option<File>> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(share_mode)
        .open(path)
    {
        Ok(file) => Ok(Some(file)),
        Err(e) if e.raw_os_error() == Some(32) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(test))]
pub(in crate::cli) fn detach(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000 | 0x00000200);
}

#[cfg(not(test))]
pub(in crate::cli) fn spawn(
    command: &mut std::process::Command,
) -> io::Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    // Preserve the controller when an agent's disposable job permits breakaway.
    command.creation_flags(0x08000000 | 0x00000200 | 0x01000000);
    match command.spawn() {
        Err(error) if error.raw_os_error() == Some(5) => {
            detach(command);
            command.spawn()
        }
        result => result,
    }
}
