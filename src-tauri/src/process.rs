use crate::config::Result;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn command(path: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd
}

pub(crate) const BACKUP_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub fn output(cmd: Command) -> Result<String> {
    crate::command_output::output(cmd, 30, &|| false)
}

pub fn sql(cmd: Command, input: &str) -> Result<()> {
    crate::command_output::run(
        cmd,
        Duration::from_secs(30),
        Some(input.as_bytes()),
        None,
        &|| false,
    )
    .map(|_| ())
}

pub fn available(port: u16) -> bool {
    probe_port(port).is_ok()
}

fn probe_port(port: u16) -> std::io::Result<()> {
    // Drop our temporary listener before probing for an existing one. macOS
    // and Windows can permit a loopback bind alongside a wildcard listener.
    drop(TcpListener::bind(("127.0.0.1", port))?);
    #[cfg(any(target_os = "macos", windows))]
    if connected(port) {
        return Err(std::io::Error::from(std::io::ErrorKind::AddrInUse));
    }
    Ok(())
}

pub fn connected(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(250),
    )
    .is_ok()
}

pub fn health(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(250),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    if stream
        .write_all(b"GET /__fairway_health HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut bytes = Vec::new();
    if stream.take(1024).read_to_end(&mut bytes).is_err() {
        return false;
    }
    let response = String::from_utf8_lossy(&bytes);
    response.contains("200 OK") && response.ends_with("fairway")
}

pub fn ensure_port(port: u16, label: &str, excluded: &[u16]) -> Result<()> {
    match probe_port(port) {
        Ok(_) => Ok(()),
        Err(error) => {
            let suggestion = (port.saturating_add(1)..=port.saturating_add(100))
                .find(|p| *p >= 1024 && !excluded.contains(p) && available(*p));
            let hint = suggestion
                .map(|p| format!(" Try port {p} in Settings."))
                .unwrap_or_default();
            Err(format!(
                "{label} port {port} is unavailable ({error}).{hint} {}",
                port_owner(port)
            ))
        }
    }
}

fn port_owner(port: u16) -> String {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = command("/usr/sbin/lsof");
        cmd.args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-F", "pc"]);
        if let Ok(out) = output(cmd) {
            let names: Vec<_> = out
                .lines()
                .filter_map(|line| line.strip_prefix('c'))
                .collect();
            if !names.is_empty() {
                return format!("Used by: {}.", names.join(", "));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let mut cmd = command("ss");
        cmd.args(["-ltnp", &format!("sport = :{port}")]);
        if let Ok(out) = output(cmd) {
            if let Some(line) = out.lines().find(|line| line.contains("users:")) {
                return format!("Used by: {line}.");
            }
        }
    }
    #[cfg(windows)]
    {
        let mut cmd = command("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &format!(
            "Get-NetTCPConnection -State Listen -LocalPort {port} -ErrorAction SilentlyContinue | ForEach-Object {{ (Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName }}")]);
        if let Ok(out) = output(cmd) {
            if !out.is_empty() {
                return format!("Used by: {}.", out.lines().collect::<Vec<_>>().join(", "));
            }
        }
    }
    String::new()
}

pub struct OwnedProcess {
    child: Option<Child>,
    pid: u32,
    #[cfg(unix)]
    start_time: Option<u64>,
    #[cfg(windows)]
    adopted_handle: Option<std::os::windows::io::OwnedHandle>,
    #[cfg(windows)]
    job: isize,
    #[cfg(windows)]
    job_preserved: bool,
}

impl OwnedProcess {
    pub fn spawn(mut cmd: Command, log: &Path, protect_database: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|e| e.to_string())?;
        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(file))
            .spawn()
            .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::JobObjects::*;
            let mut job = 0;
            if !protect_database {
                unsafe {
                    let name = windows_job_name(log);
                    let handle = CreateJobObjectW(std::ptr::null(), name.as_ptr());
                    if !handle.is_null() {
                        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                        if SetInformationJobObject(
                            handle,
                            JobObjectExtendedLimitInformation,
                            &info as *const _ as *const _,
                            std::mem::size_of_val(&info) as u32,
                        ) != 0
                            && AssignProcessToJobObject(handle, child.as_raw_handle() as _) != 0
                        {
                            job = handle as isize;
                        } else {
                            windows_sys::Win32::Foundation::CloseHandle(handle);
                        }
                    }
                }
            }
            let pid = child.id();
            Ok(Self {
                child: Some(child),
                pid,
                job,
                job_preserved: false,
                adopted_handle: None,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = protect_database;
            let pid = child.id();
            Ok(Self {
                child: Some(child),
                pid,
                start_time: None,
            })
        }
    }

    pub fn alive(&mut self) -> bool {
        match &mut self.child {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => {
                #[cfg(unix)]
                {
                    if self.start_time.is_none() || process_start_time(self.pid) != self.start_time
                    {
                        return false;
                    }
                    pid_alive(self.pid)
                }
                #[cfg(windows)]
                {
                    use std::os::windows::io::AsRawHandle;
                    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, GetProcessId};
                    self.adopted_handle.as_ref().is_some_and(|handle| unsafe {
                        let handle = handle.as_raw_handle() as _;
                        let mut code = 0;
                        GetProcessId(handle) == self.pid
                            && GetExitCodeProcess(handle, &mut code) != 0
                            && code == 259
                    })
                }
            }
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn adopt_database(pid: u32) -> Self {
        Self {
            child: None,
            pid,
            #[cfg(unix)]
            start_time: process_start_time(pid),
            #[cfg(windows)]
            adopted_handle: open_process(pid),
            #[cfg(windows)]
            job: 0,
            #[cfg(windows)]
            job_preserved: false,
        }
    }

    pub(crate) fn adopt_verified(pid: u32, matches: impl FnOnce() -> bool) -> Option<Self> {
        // Capture identity before inspecting executable/configuration, then
        // recheck it so a PID recycled during those checks cannot be adopted.
        let mut owned = Self::adopt_database(pid);
        (owned.alive() && matches() && owned.alive()).then_some(owned)
    }

    #[cfg(windows)]
    pub(crate) fn adopt_job(pid: u32, log: &Path) -> Option<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        const JOB_OBJECT_QUERY: u32 = 0x0004;
        const JOB_OBJECT_SET_ATTRIBUTES: u32 = 0x0002;
        let mut owned = Self::adopt_database(pid);
        let name = windows_job_name(log);
        let job = unsafe {
            OpenJobObjectW(
                JOB_OBJECT_QUERY | JOB_OBJECT_SET_ATTRIBUTES,
                0,
                name.as_ptr(),
            )
        };
        if job.is_null() {
            return None;
        }
        owned.job = job as isize;
        owned.job_preserved = true;
        let mut member = 0;
        let handle = owned.adopted_handle.as_ref()?;
        if unsafe { IsProcessInJob(handle.as_raw_handle() as _, job, &mut member) } == 0
            || member == 0
            || !owned.alive()
        {
            return None;
        }
        Some(owned)
    }

    pub(crate) fn leave_running(&mut self) -> Result<()> {
        #[cfg(windows)]
        if self.job != 0 {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS};
            use windows_sys::Win32::System::JobObjects::*;
            use windows_sys::Win32::System::Threading::GetCurrentProcess;
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe {
                SetInformationJobObject(
                    self.job as _,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&info) as u32,
                )
            } == 0
            {
                return Err(format!(
                    "Could not preserve a service job: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if !self.job_preserved {
                let child = self
                    .child
                    .as_ref()
                    .ok_or("The service process is unavailable.")?;
                let mut remote = std::ptr::null_mut();
                // A job's name disappears when its last handle closes, even
                // while members remain alive. Let the explicitly retained
                // service hold a handle until exit so a new controller can
                // reopen the same job and verify process membership.
                if unsafe {
                    DuplicateHandle(
                        GetCurrentProcess(),
                        self.job as _,
                        child.as_raw_handle() as _,
                        &mut remote,
                        0,
                        0,
                        DUPLICATE_SAME_ACCESS,
                    )
                } == 0
                {
                    let error = std::io::Error::last_os_error();
                    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                    let restored = unsafe {
                        SetInformationJobObject(
                            self.job as _,
                            JobObjectExtendedLimitInformation,
                            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                            std::mem::size_of_val(&info) as u32,
                        )
                    };
                    return Err(if restored == 0 {
                        format!("Could not preserve the service job ({error}) or restore its exit policy: {}", std::io::Error::last_os_error())
                    } else {
                        format!("Could not preserve the service job: {error}")
                    });
                }
                self.job_preserved = true;
            }
        }
        Ok(())
    }

    pub fn wait(&mut self, seconds: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while self.alive() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        !self.alive()
    }

    pub fn graceful_signal(&mut self) -> Result<()> {
        if !self.alive() {
            return Ok(());
        }
        #[cfg(unix)]
        {
            if unsafe { libc::kill(self.pid as i32, libc::SIGQUIT) } != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        Ok(())
    }

    // Only used for web/PHP processes. MySQL is never forcibly terminated.
    pub fn terminate_web(&mut self) -> Result<()> {
        if !self.alive() {
            return Ok(());
        }
        #[cfg(unix)]
        {
            unsafe {
                libc::kill(-(self.pid as i32), libc::SIGTERM);
            }
        }
        #[cfg(windows)]
        {
            let mut cmd = command("taskkill.exe");
            cmd.args(["/PID", &self.pid.to_string(), "/T", "/F"]);
            output(cmd)?;
        }
        if !self.wait(5) {
            return Err("The web service did not stop. Please retry.".into());
        }
        Ok(())
    }
}

pub fn pid_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    #[cfg(unix)]
    {
        // A recovered child can still be a zombie when its previous controller
        // lived in this same process. Reap it before treating kill(0) as liveness.
        let mut status = 0;
        if unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) } == pid as i32 {
            return false;
        }
        #[cfg(target_os = "linux")]
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            if stat
                .rsplit_once(')')
                .is_some_and(|(_, tail)| matches!(tail.split_whitespace().next(), Some("Z" | "X")))
            {
                return false;
            }
        }
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::*;
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut code = 0;
            let alive = GetExitCodeProcess(handle, &mut code) != 0 && code == 259;
            windows_sys::Win32::Foundation::CloseHandle(handle);
            alive
        }
    }
}

pub fn matches_database(pid: u32, executable: &Path, config: &Path) -> bool {
    if !pid_alive(pid) {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        if linux_executable_matches(pid, executable) {
            if let Ok(bytes) = std::fs::read(format!("/proc/{pid}/cmdline")) {
                return bytes
                    .split(|b| *b == 0)
                    .any(|arg| arg == format!("--defaults-file={}", config.display()).as_bytes());
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = command("/bin/ps");
        cmd.args(["-ww", "-p", &pid.to_string(), "-o", "command="]);
        if let Ok(line) = output(cmd) {
            return line
                == format!(
                    "{} --defaults-file={}",
                    executable.display(),
                    config.display()
                );
        }
    }
    #[cfg(windows)]
    {
        let mut cmd = command("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &format!(
            "Get-CimInstance Win32_Process -Filter 'ProcessId = {pid}' | Select-Object ExecutablePath,CommandLine | ConvertTo-Json -Compress")]);
        if let Ok(text) = output(cmd) {
            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&text) {
                let exe = info["ExecutablePath"].as_str().unwrap_or("");
                let line = info["CommandLine"].as_str().unwrap_or("");
                return same_windows_executable(Path::new(exe), executable)
                    && line.contains(&format!("--defaults-file={}", config.display()));
            }
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn process_start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

#[cfg(target_os = "macos")]
fn process_start_time(pid: u32) -> Option<u64> {
    if pid == 0 || pid > i32::MAX as u32 {
        return None;
    }
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let read = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    info.pbi_start_tvsec
        .checked_mul(1_000_000)?
        .checked_add(info.pbi_start_tvusec)
}

#[cfg(windows)]
fn open_process(pid: u32) -> Option<std::os::windows::io::OwnedHandle> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        None
    } else {
        // Keeping the process object open pins its identity through exit and
        // prevents its PID being reused while a PID-based command is pending.
        Some(unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(handle as _) })
    }
}

#[cfg(windows)]
fn windows_job_name(log: &Path) -> Vec<u16> {
    // A stable per-installation/service name permits recovery after an explicit
    // leave-running exit. Membership is checked against a pinned process handle.
    let path = std::fs::canonicalize(log)
        .unwrap_or_else(|_| log.into())
        .to_string_lossy()
        .to_lowercase();
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.as_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("Local\\FairwayXMP-{hash:016x}")
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

#[cfg(target_os = "linux")]
fn linux_executable_matches(pid: u32, expected: &Path) -> bool {
    match (
        std::fs::read_link(format!("/proc/{pid}/exe")),
        std::fs::canonicalize(expected),
    ) {
        (Ok(actual), Ok(expected)) => actual == expected,
        _ => false,
    }
}

#[cfg(target_os = "macos")]
fn macos_executable_matches(pid: u32, expected: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let mut buffer = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let length =
        unsafe { libc::proc_pidpath(pid as i32, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if length <= 0 || length as usize > buffer.len() {
        return false;
    }
    let bytes = &buffer[..length as usize];
    let actual = Path::new(std::ffi::OsStr::from_bytes(
        bytes.strip_suffix(&[0]).unwrap_or(bytes),
    ));
    match (
        std::fs::canonicalize(actual),
        std::fs::canonicalize(expected),
    ) {
        (Ok(actual), Ok(expected)) => actual == expected,
        _ => false,
    }
}

/// Whether `pid` runs `executable` with exactly `args`, for services that keep their command line.
#[cfg(unix)]
pub fn matches_command(pid: u32, executable: &Path, args: &[String]) -> bool {
    #[cfg(target_os = "linux")]
    let executable_matches = linux_executable_matches(pid, executable);
    #[cfg(target_os = "macos")]
    let executable_matches = macos_executable_matches(pid, executable);
    if !pid_alive(pid) || !executable_matches {
        return false;
    }
    let expected = std::iter::once(executable.to_string_lossy().into_owned())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    #[cfg(target_os = "linux")]
    {
        std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|bytes| {
            bytes
                .strip_suffix(b"\0")
                .unwrap_or(&bytes)
                .split(|b| *b == 0)
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .eq(expected)
        })
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = command("/bin/ps");
        cmd.args(["-ww", "-p", &pid.to_string(), "-o", "command="]);
        output(cmd).is_ok_and(|line| line == expected.join(" "))
    }
}

#[cfg(unix)]
pub fn matches_web(pid: u32, id: &str, layout: &crate::config::Layout) -> bool {
    let exe = match id {
        "php" => match layout.php_exe("php-fpm") {
            Ok(exe) => exe,
            Err(_) => return false,
        },
        "nginx" => layout.exe("nginx", "nginx"),
        _ => return false,
    };
    if !pid_alive(pid) {
        return false;
    }
    #[cfg(target_os = "linux")]
    let title = {
        if !linux_executable_matches(pid, &exe) {
            return false;
        }
        let Ok(bytes) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
            return false;
        };
        String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_owned()
    };
    #[cfg(target_os = "macos")]
    let title = {
        if !macos_executable_matches(pid, &exe) {
            return false;
        }
        let mut cmd = command("/bin/ps");
        cmd.args(["-ww", "-p", &pid.to_string(), "-o", "command="]);
        let Ok(title) = output(cmd) else {
            return false;
        };
        title
    };
    if id == "php" {
        title
            == format!(
                "php-fpm: master process ({})",
                layout.data.join("config/php-fpm.conf").display()
            )
    } else {
        title
            == format!(
                "nginx: master process {} -e {} -p {} -c {} -g daemon off;",
                exe.display(),
                layout.data.join("logs/nginx.log").display(),
                layout.data.display(),
                layout.data.join("config/nginx.conf").display()
            )
    }
}

#[cfg(windows)]
fn same_windows_executable(actual: &Path, expected: &Path) -> bool {
    // CIM reports normalized paths; Rust paths may contain mixed separators or
    // junctions. Compare resolved files rather than their original spelling.
    match (
        std::fs::canonicalize(actual),
        std::fs::canonicalize(expected),
    ) {
        (Ok(actual), Ok(expected)) => actual
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.to_string_lossy()),
        _ => false,
    }
}

#[cfg(windows)]
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if self.job != 0 {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job as _) };
        }
    }
}

pub fn tail(path: &Path) -> Result<String> {
    use std::io::{Seek, SeekFrom};
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok("No log entries yet.".into())
        }
        Err(e) => return Err(e.to_string()),
    };
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Expected a regular log file.".into());
    }
    let size = metadata.len();
    let offset = size.saturating_sub(64 * 1024);
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(size - offset)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_tail_bounds_large_files_and_rejects_special_files() {
        let root = std::env::temp_dir().join(format!("fairway-tail-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("large.log");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(4 * 1024 * 1024).unwrap();
        drop(file);
        assert_eq!(tail(&path).unwrap().len(), 64 * 1024);
        assert!(tail(&root).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let fifo = root.join("pipe.log");
            let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            assert!(tail(&fifo).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn silent_command_failure_retains_the_exit_status() {
        #[cfg(windows)]
        let mut cmd = command(
            std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32")
                .join("cmd.exe"),
        );
        #[cfg(windows)]
        cmd.args(["/D", "/C", "exit", "7"]);
        #[cfg(unix)]
        let mut cmd = command("/bin/sh");
        #[cfg(unix)]
        cmd.args(["-c", "exit 7"]);
        let error = output(cmd).unwrap_err();
        assert!(error.contains("Command failed"), "{error}");
        assert!(error.contains('7'), "{error}");
    }
    #[cfg(unix)]
    #[test]
    fn adopted_process_does_not_accept_a_reused_pid() {
        let mut owned = OwnedProcess::adopt_database(std::process::id());
        assert!(owned.alive());
        assert!(owned.start_time.is_some());
        owned.start_time = owned.start_time.map(|stamp| stamp + 1);
        assert!(!owned.alive());
    }

    struct TestChild(Child);

    impl TestChild {
        fn new() -> Self {
            #[cfg(unix)]
            let child = command("/bin/sleep").arg("30").spawn().unwrap();
            #[cfg(windows)]
            let child = command("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 30",
                ])
                .spawn()
                .unwrap();
            Self(child)
        }
    }

    impl Drop for TestChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn verified_adoption_rechecks_identity_after_executable_validation() {
        let mut child = TestChild::new();
        let pid = child.0.id();
        assert!(OwnedProcess::adopt_verified(pid, || false).is_none());
        assert!(OwnedProcess::adopt_verified(pid, || true).is_some());
        assert!(OwnedProcess::adopt_verified(pid, || {
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            true
        })
        .is_none());
        assert!(OwnedProcess::adopt_verified(u32::MAX, || panic!(
            "invalid PID must fail before validation"
        ))
        .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn stale_or_unverifiable_identity_never_receives_a_signal() {
        let mut child = TestChild::new();
        let mut owned = OwnedProcess::adopt_database(child.0.id());
        assert!(owned.alive());
        let start_time = owned.start_time.unwrap();
        for identity in [Some(start_time + 1), None] {
            owned.start_time = identity;
            assert!(!owned.alive());
            owned.graceful_signal().unwrap();
            owned.terminate_web().unwrap();
            assert!(child.0.try_wait().unwrap().is_none());
        }
    }

    #[cfg(windows)]
    #[test]
    fn adopted_windows_handle_rejects_exit_and_another_live_pid() {
        let mut original = TestChild::new();
        let mut other = TestChild::new();
        let mut owned = OwnedProcess::adopt_database(original.0.id());
        assert!(owned.alive());
        original.0.kill().unwrap();
        original.0.wait().unwrap();
        assert!(!owned.alive());
        owned.pid = other.0.id();
        assert!(!owned.alive());
        owned.terminate_web().unwrap();
        assert!(other.0.try_wait().unwrap().is_none());
        let mut unverified = OwnedProcess::adopt_database(other.0.id());
        unverified.adopted_handle = None;
        assert!(!unverified.alive());
        unverified.terminate_web().unwrap();
        assert!(other.0.try_wait().unwrap().is_none());
    }

    #[cfg(windows)]
    #[test]
    fn named_service_job_survives_explicit_leave_and_rejects_foreign_membership() {
        let root =
            std::env::temp_dir().join(format!("fairway-job-recovery-{}", rand::random::<u64>()));
        std::fs::create_dir(&root).unwrap();
        let log = root.join("php-process.log");
        let mut cmd = command("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 30",
        ]);
        let mut owned = OwnedProcess::spawn(cmd, &log, false).unwrap();
        assert_ne!(owned.job, 0);
        let pid = owned.pid();
        owned.leave_running().unwrap();
        assert!(owned.job_preserved);
        owned.leave_running().unwrap();
        drop(owned);
        let mut recovered =
            OwnedProcess::adopt_job(pid, &log).expect("left-running service was not recovered");
        assert!(recovered.alive());
        assert!(recovered.job_preserved);
        assert!(OwnedProcess::adopt_job(std::process::id(), &log).is_none());
        assert!(OwnedProcess::adopt_job(pid, &root.join("nginx-process.log")).is_none());
        recovered.terminate_web().unwrap();
        drop(recovered);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn recovered_web_process_requires_the_actual_executable() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "fairway-process-identity-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let actual = std::env::current_exe().unwrap();
        let alias = root.join("executable alias");
        symlink(&actual, &alias).unwrap();
        let different = root.join("different executable");
        std::fs::write(&different, b"not the running executable").unwrap();
        assert!(macos_executable_matches(std::process::id(), &actual));
        assert!(macos_executable_matches(std::process::id(), &alias));
        assert!(!macos_executable_matches(std::process::id(), &different));
        assert!(!macos_executable_matches(0, &actual));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn command_identity_requires_the_exact_arguments() {
        let mut child = command("/bin/sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        // Pass the path as launched; the binary check resolves symlinks itself.
        let sleep = Path::new("/bin/sleep");
        // Linux can briefly report the pre-exec image right after spawn returns.
        assert!((0..100).any(|_| {
            matches_command(pid, sleep, &["30".into()]) || {
                std::thread::sleep(Duration::from_millis(10));
                false
            }
        }));
        assert!(!matches_command(pid, sleep, &["31".into()]));
        assert!(!matches_command(pid, Path::new("/bin/sh"), &["30".into()]));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!matches_command(pid, sleep, &["30".into()]));
    }

    #[cfg(windows)]
    #[test]
    fn background_commands_have_no_console_and_keep_captured_errors() {
        let result = command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", r#"
$ErrorActionPreference = 'Stop'
Add-Type -Namespace Fairway -Name ConsoleProbe -MemberDefinition '[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow();'
if ([Fairway.ConsoleProbe]::GetConsoleWindow() -ne [IntPtr]::Zero) { exit 9 }
[Console]::Out.Write('no-console')
[Console]::Error.Write('captured-error')
exit 7
"#])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(7));
        assert_eq!(String::from_utf8_lossy(&result.stdout), "no-console");
        assert_eq!(String::from_utf8_lossy(&result.stderr), "captured-error");
    }

    #[cfg(windows)]
    #[test]
    fn database_executable_identity_accepts_normalized_windows_paths() {
        let exe = std::env::current_exe().unwrap();
        let mixed = exe.to_string_lossy().replace('\\', "/").to_lowercase();
        assert!(same_windows_executable(Path::new(&mixed), &exe));
        assert!(!same_windows_executable(&exe, exe.parent().unwrap()));
        assert!(!same_windows_executable(
            &exe.with_file_name("different-database.exe"),
            &exe
        ));
    }

    #[test]
    fn occupied_ports_are_detected_without_stopping_the_owner() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let error = ensure_port(port, "Website", &[port]).unwrap_err();
        assert!(error.contains("unavailable"));
        assert!(connected(port));
        // The successful connection proves ensure_port left the owner alive.
        // Immediate rebinding after close is OS-dependent (pending TCP teardown).
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }

    #[test]
    fn wildcard_listener_is_detected_by_preflight_and_port_suggestions() {
        let listener = TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!available(port));
        assert!(ensure_port(port, "MySQL", &[port])
            .unwrap_err()
            .contains("unavailable"));
        assert!(connected(port));
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }

    #[cfg(target_os = "macos")]
    fn ipv6_listener(only_v6: bool) -> TcpListener {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

        let fd = unsafe { libc::socket(libc::AF_INET6, libc::SOCK_STREAM, 0) };
        assert!(fd >= 0, "{}", std::io::Error::last_os_error());
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };
        for (level, name, value) in [
            (libc::SOL_SOCKET, libc::SO_REUSEADDR, 1i32),
            (libc::IPPROTO_IPV6, libc::IPV6_V6ONLY, i32::from(only_v6)),
        ] {
            assert_eq!(
                unsafe {
                    libc::setsockopt(
                        socket.as_raw_fd(),
                        level,
                        name,
                        (&value as *const i32).cast(),
                        std::mem::size_of_val(&value) as _,
                    )
                },
                0
            );
        }
        let mut address: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
        address.sin6_len = std::mem::size_of_val(&address) as u8;
        address.sin6_family = libc::AF_INET6 as _;
        assert_eq!(
            unsafe {
                libc::bind(
                    socket.as_raw_fd(),
                    (&address as *const libc::sockaddr_in6).cast(),
                    std::mem::size_of_val(&address) as _,
                )
            },
            0
        );
        assert_eq!(unsafe { libc::listen(socket.as_raw_fd(), 16) }, 0);
        TcpListener::from(socket)
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn dual_stack_wildcard_blocks_loopback_but_ipv6_only_does_not() {
        let dual_stack = ipv6_listener(false);
        let port = dual_stack.local_addr().unwrap().port();
        assert!(!available(port));
        assert!(ensure_port(port, "Website", &[port]).is_err());
        assert!(connected(port));
        let ipv6_only = ipv6_listener(true);
        let port = ipv6_only.local_addr().unwrap().port();
        assert!(available(port));
        assert!(ensure_port(port, "Website", &[port]).is_ok());
    }
}
