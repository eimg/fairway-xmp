use super::{Envelope, Request, Response};
use crate::{
    config::{Layout, Result},
    integration,
    manager::Manager,
};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::{Arc, Mutex, TryLockError},
    thread,
    time::{Duration, Instant},
};

static STOP: AtomicBool = AtomicBool::new(false);
const MAX_REQUEST: u64 = 16 * 1024;
const MAX_RESPONSE: u64 = 1024 * 1024;
extern "C" fn signal_handler(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

pub(super) fn available(layout: &Layout) -> bool {
    UnixStream::connect(layout.data.join("supervisor.sock")).is_ok()
}

pub(super) fn ensure(layout: &Layout) -> Result<()> {
    if available(layout) {
        return Ok(());
    }
    layout.initialize()?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(layout.data.join("logs/supervisor.log"))
        .map_err(|e| e.to_string())?;
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .arg("__serve")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log));
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if available(layout) {
            return Ok(());
        }
        // A losing concurrent launch can exit before the winner publishes its socket.
        // Wait for readiness instead of treating that normal race as a startup failure.
        let _ = child.try_wait().map_err(|e| e.to_string())?;
        if Instant::now() > deadline {
            return Err("The supervisor did not become ready. Check its log.".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn send_request(layout: &Layout, request: &Request) -> Result<UnixStream> {
    let mut socket = UnixStream::connect(layout.data.join("supervisor.sock"))
        .map_err(|e| format!("Cannot connect to supervisor: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(240)))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut bytes =
        serde_json::to_vec(&serde_json::json!({"runtime": layout.runtime, "request": request}))
            .map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    socket.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(socket)
}

pub(super) fn receive_response(layout: &Layout, socket: &UnixStream) -> Result<serde_json::Value> {
    let response: Response =
        serde_json::from_slice(&read_line(socket, MAX_RESPONSE)?).map_err(|e| e.to_string())?;
    if response.runtime != layout.runtime {
        return Err("Another Fairway installation owns the services. Shut it down using its original command before switching installations.".into());
    }
    match response.error {
        Some(error) => Err(error),
        None => Ok(response.data),
    }
}

pub(super) fn call(layout: &Layout, request: &Request) -> Result<serde_json::Value> {
    receive_response(layout, &send_request(layout, request)?)
}

pub(super) fn wait_shutdown(layout: &Layout) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while layout.data.join("supervisor.sock").exists() {
        if Instant::now() >= deadline {
            return Err(
                "Shutdown completed but the supervisor socket has not closed. Check its log."
                    .into(),
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn read_line(stream: &UnixStream, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    BufReader::new(stream.take(limit + 1))
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit || !bytes.ends_with(b"\n") {
        return Err("Invalid or oversized supervisor message.".into());
    }
    Ok(bytes)
}

struct SocketGuard(std::path::PathBuf);
impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(super) fn serve(layout: &Layout) -> Result<()> {
    layout.initialize()?;
    let mut lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(layout.data.join("supervisor.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Ok(());
    }
    lock.set_len(0).map_err(|e| e.to_string())?;
    write!(lock, "{}", std::process::id()).map_err(|e| e.to_string())?;
    let socket_path = layout.data.join("supervisor.sock");
    match fs::remove_file(&socket_path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    // Initialize before publishing the socket, so readiness means commands can run.
    let manager = Arc::new(Mutex::new(Manager::new(layout.clone())?));
    let quitting = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let active = Arc::new(AtomicUsize::new(0));
    let listener = UnixListener::bind(&socket_path).map_err(|e| {
        format!("Cannot create supervisor socket (the data path may be too long): {e}")
    })?;
    let _socket_guard = SocketGuard(socket_path.clone());
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = signal_handler as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
    }
    while !finished.load(Ordering::Acquire) {
        if STOP.swap(false, Ordering::Relaxed) {
            quitting.store(true, Ordering::Release);
            match manager
                .lock()
                .map_err(|_| "Service manager failed.")?
                .stop_all()
            {
                Ok(()) => return Ok(()),
                Err(error) => {
                    quitting.store(false, Ordering::Release);
                    eprintln!("Shutdown failed; supervisor remains available: {error}");
                }
            }
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if active.load(Ordering::Acquire) >= 16 {
                    let mut stream = stream;
                    send_response(
                        &mut stream,
                        layout,
                        Err("Too many concurrent clients. Retry shortly.".into()),
                    );
                    continue;
                }
                active.fetch_add(1, Ordering::AcqRel);
                let active = active.clone();
                let manager = manager.clone();
                let quitting = quitting.clone();
                let finished = finished.clone();
                let layout = layout.clone();
                thread::spawn(move || {
                    handle_connection(stream, &layout, &manager, &quitting, &finished);
                    active.fetch_sub(1, Ordering::AcqRel);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50))
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
            Err(error) => {
                eprintln!("Supervisor socket: {error}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
    Ok(())
}

fn send_response(stream: &mut UnixStream, layout: &Layout, result: Result<serde_json::Value>) {
    let response = match result {
        Ok(data) => Response {
            data,
            error: None,
            runtime: layout.runtime.clone(),
        },
        Err(error) => Response {
            data: serde_json::Value::Null,
            error: Some(error),
            runtime: layout.runtime.clone(),
        },
    };
    if let Ok(mut bytes) = serde_json::to_vec(&response) {
        bytes.push(b'\n');
        let _ = stream.write_all(&bytes);
    }
}

fn handle_connection(
    mut stream: UnixStream,
    layout: &Layout,
    manager: &Mutex<Manager>,
    quitting: &AtomicBool,
    finished: &AtomicBool,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let request = read_line(&stream, MAX_REQUEST)
        .and_then(|bytes| serde_json::from_slice::<Envelope>(&bytes).map_err(|e| e.to_string()));
    let mut quit = false;
    let result = request.and_then(|envelope| {
        if envelope.runtime != layout.runtime {
            return Err("Another Fairway installation owns the services. Shut it down before switching installations.".into());
        }
        let request = envelope.request;
        // Read-only clients must not freeze a dashboard behind a long startup/backup.
        let mut manager = if matches!(request, Request::Status | Request::Log { .. }) {
            let deadline = Instant::now() + Duration::from_millis(250);
            loop {
                match manager.try_lock() {
                    Ok(guard) => break guard,
                    Err(TryLockError::WouldBlock) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                    Err(TryLockError::WouldBlock) => return Err("A service operation is in progress. Retry shortly.".into()),
                    Err(_) => return Err("Service manager failed.".into()),
                }
            }
        } else { manager.lock().map_err(|_| "Service manager failed.")? };
        if quitting.load(Ordering::Acquire) { return Err("Fairway is shutting down. Retry after it stops.".into()); }
        if matches!(request, Request::Shutdown | Request::PrepareUpdate) {
            quitting.store(true, Ordering::Release);
            let shutdown = (|| -> Result<()> {
                if matches!(request, Request::PrepareUpdate) && manager.layout.data.join("mysql/mysql").is_dir() {
                    manager.start("mysql")?;
                    println!("Update backup: {}", manager.backup()?);
                }
                manager.stop_all()
            })();
            if let Err(error) = shutdown { quitting.store(false, Ordering::Release); return Err(error); }
            quit = true;
            return Ok(serde_json::json!("Stopped"));
        }
        execute(&mut manager, request)
    });
    send_response(&mut stream, layout, result);
    if quit {
        finished.store(true, Ordering::Release);
    }
}

fn execute(manager: &mut Manager, request: Request) -> Result<serde_json::Value> {
    match request {
        Request::Status => {
            return serde_json::to_value(manager.snapshot()).map_err(|e| e.to_string())
        }
        Request::Start { service } => service_action(manager, &service, true)?,
        Request::Stop { service } => service_action(manager, &service, false)?,
        Request::Restart { service } => {
            service_action(manager, &service, false)?;
            service_action(manager, &service, true)?;
        }
        Request::Log { service } => return manager.log(&service).map(serde_json::Value::String),
        Request::Ports { ports } => {
            manager.save_ports(ports)?;
            return serde_json::to_value(&manager.ports).map_err(|e| e.to_string());
        }
        Request::PhpVersion { version } => {
            manager.save_php_version(version)?;
            return Ok(serde_json::json!(manager.layout.php_version()?));
        }
        Request::Backup => return manager.backup().map(serde_json::Value::String),
        Request::InstallCli => {
            integration::install(&manager.layout)?;
            let path = manager.layout.bin.join("fairway");
            if path.exists() || path.is_symlink() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            std::os::unix::fs::symlink(std::env::current_exe().map_err(|e| e.to_string())?, path)
                .map_err(|e| e.to_string())?;
            return Ok(serde_json::json!(
                "Command-line tools enabled. Reopen your shell to refresh PATH."
            ));
        }
        Request::ShellTools => {
            integration::write_unix_launchers(&manager.layout)?;
            return Ok(serde_json::json!("Done"));
        }
        Request::RemoveCli => {
            integration::remove(&manager.layout)?;
            let _ = fs::remove_file(manager.layout.bin.join("fairway"));
            return Ok(serde_json::json!(
                "Managed shell integration removed. Projects and data preserved."
            ));
        }
        Request::Shutdown | Request::PrepareUpdate => unreachable!(),
    }
    Ok(serde_json::json!("Done"))
}

fn service_action(manager: &mut Manager, service: &str, start: bool) -> Result<()> {
    if !["all", "nginx", "mysql"].contains(&service) {
        return Err("Choose all, nginx or mysql.".into());
    }
    if service == "all" {
        if start {
            manager.start_all()
        } else {
            manager.stop_all()
        }
    } else if start {
        manager.start(service)
    } else {
        manager.stop(service)?;
        if service == "nginx" {
            manager.stop("php")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_does_not_wait_for_an_active_operation() {
        let root = std::env::temp_dir().join(format!(
            "fairway-busy-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        let manager = Arc::new(Mutex::new(Manager::new(layout.clone()).unwrap()));
        let guard = manager.lock().unwrap();
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let manager_clone = manager.clone();
        let layout_clone = layout.clone();
        let worker = thread::spawn(move || {
            handle_connection(
                server,
                &layout_clone,
                &manager_clone,
                &AtomicBool::new(false),
                &AtomicBool::new(false),
            )
        });
        let bytes = serde_json::to_vec(
            &serde_json::json!({"runtime": layout.runtime, "request": {"command": "status"}}),
        )
        .unwrap();
        client.write_all(&bytes).unwrap();
        client.write_all(b"\n").unwrap();
        let response: Response =
            serde_json::from_slice(&read_line(&client, MAX_RESPONSE).unwrap()).unwrap();
        assert!(response.error.unwrap().contains("operation is in progress"));
        worker.join().unwrap();
        drop(guard);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn protocol_rejects_unterminated_and_oversized_messages() {
        for message in [b"{}".as_slice(), b"01234567890123456789\n".as_slice()] {
            let (mut client, server) = UnixStream::pair().unwrap();
            client.write_all(message).unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            assert!(read_line(&server, 16).is_err());
        }
    }
}
