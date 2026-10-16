use crate::config::Result;
use std::{
    fs::{File, OpenOptions},
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

pub fn output(mut cmd: Command) -> Result<String> {
    let result = cmd.output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&result.stdout).trim().into())
}

pub fn sql(mut cmd: Command, input: &str) -> Result<()> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("Cannot open database command input.")?
        .write_all(input.as_bytes())
        .map_err(|e| e.to_string())?;
    let result = child.wait_with_output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into());
    }
    Ok(())
}

pub fn available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
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
    match TcpListener::bind(("127.0.0.1", port)) {
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
    #[cfg(target_os = "linux")]
    start_time: Option<u64>,
    #[cfg(windows)]
    job: isize,
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
                    let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
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
            })
        }
        #[cfg(not(windows))]
        {
            let _ = protect_database;
            let pid = child.id();
            Ok(Self {
                child: Some(child),
                pid,
                #[cfg(target_os = "linux")]
                start_time: linux_start_time(pid),
            })
        }
    }

    pub fn alive(&mut self) -> bool {
        match &mut self.child {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => {
                #[cfg(target_os = "linux")]
                if self.start_time.is_none() || linux_start_time(self.pid) != self.start_time {
                    return false;
                }
                pid_alive(self.pid)
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
            #[cfg(target_os = "linux")]
            start_time: linux_start_time(pid),
            #[cfg(windows)]
            job: 0,
        }
    }

    pub fn wait(&mut self, seconds: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while self.alive() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        !self.alive()
    }

    pub fn graceful_signal(&self) -> Result<()> {
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
fn linux_start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
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

#[cfg(target_os = "linux")]
pub fn matches_web(pid: u32, id: &str, layout: &crate::config::Layout) -> bool {
    let exe = if id == "php" {
        let Ok(exe) = layout.php_exe("php-fpm") else {
            return false;
        };
        exe
    } else {
        layout.exe(id, "nginx")
    };
    if !pid_alive(pid) || !linux_executable_matches(pid, &exe) {
        return false;
    }
    let Ok(bytes) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    let title = String::from_utf8_lossy(&bytes)
        .trim_end_matches('\0')
        .to_owned();
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
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok("No log entries yet.".into())
        }
        Err(e) => return Err(e.to_string()),
    };
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(64 * 1024)))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn adopted_process_does_not_accept_a_reused_pid() {
        let mut owned = OwnedProcess::adopt_database(std::process::id());
        assert!(owned.alive());
        owned.start_time = owned.start_time.map(|stamp| stamp + 1);
        assert!(!owned.alive());
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
}
