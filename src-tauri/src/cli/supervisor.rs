use super::transport::{self, Listener, Stream};
use super::{Envelope, Request, Response};
use crate::{
    config::{Layout, Result},
    integration,
    manager::Manager,
};
use std::{
    fs::{self},
    io::{BufRead, BufReader, Read, Write},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::{Arc, Mutex, TryLockError},
    thread,
    time::{Duration, Instant},
};

static STOP: AtomicBool = AtomicBool::new(false);
const MAX_REQUEST: u64 = 3 * 1024 * 1024;
const MAX_RESPONSE: u64 = 9 * 1024 * 1024;
#[cfg(unix)]
extern "C" fn signal_handler(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

pub(crate) fn available(layout: &Layout) -> bool {
    transport::connect(layout).is_ok()
}

pub(crate) fn ensure(layout: &Layout) -> Result<()> {
    layout.initialize()?;
    if transport::shared_lock(&layout.data.join("installation.lock"))?.is_none() {
        return Err("Fairway is being updated. Retry when installation finishes.".into());
    }
    if available(layout) {
        return Ok(());
    }
    #[cfg(test)]
    {
        ensure_test(layout)
    }
    #[cfg(not(test))]
    ensure_process(layout)
}

#[cfg(test)]
fn ensure_test(layout: &Layout) -> Result<()> {
    let layout = layout.clone();
    let check = layout.clone();
    let (send, receive) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let _ = send.send(serve(&layout));
    });
    let start = Instant::now();
    while !available(&check) {
        if let Ok(result) = receive.try_recv() {
            return result.and_then(|_| Err("Supervisor stopped before becoming ready.".into()));
        }
        if start.elapsed() > Duration::from_secs(5) {
            return Err("Test supervisor did not become ready.".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

#[cfg(not(test))]
fn ensure_process(layout: &Layout) -> Result<()> {
    use std::{
        fs::OpenOptions,
        process::{Command, Stdio},
    };
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let log = options
        .open(layout.data.join("logs/supervisor.log"))
        .map_err(|e| e.to_string())?;
    let mut command = Command::new(executable()?);
    command
        .arg("__serve")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log));
    transport::detach(&mut command);
    let mut child = transport::spawn(&mut command).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if available(layout) {
            // A desktop can outlive an externally requested shutdown. Reap its
            // controller child even when another CLI initiated that shutdown.
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Ok(());
        }
        // A losing concurrent launch can exit before the winner publishes its socket.
        // Wait for readiness instead of treating that normal race as a startup failure.
        if child.try_wait().map_err(|e| e.to_string())?.is_some()
            && transport::lock(layout)?.is_some()
            && !available(layout)
        {
            return Err(format!(
                "The supervisor could not start. {}",
                crate::process::tail(&layout.data.join("logs/supervisor.log")).unwrap_or_default()
            ));
        }
        if Instant::now() > deadline {
            return Err("The supervisor did not become ready. Check its log.".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn response_timeout(request: &Request) -> Option<Duration> {
    match request {
        Request::Backup
        | Request::Shutdown
        | Request::PrepareUpdate
        | Request::PrepareDesktopUpdate => None,
        Request::Database { args }
            if args.first().is_some_and(|s| s == "import" || s == "export") =>
        {
            None
        }
        _ => Some(Duration::from_secs(240)),
    }
}

pub(crate) fn send_request(layout: &Layout, request: &Request) -> Result<Stream> {
    let socket =
        transport::connect(layout).map_err(|e| format!("Cannot connect to supervisor: {e}"))?;
    socket
        .set_read_timeout(response_timeout(request))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    write_request(layout, &socket, request)?;
    Ok(socket)
}

pub(crate) fn write_request(layout: &Layout, mut socket: &Stream, request: &Request) -> Result<()> {
    let mut bytes =
        serde_json::to_vec(&serde_json::json!({"runtime": layout.runtime, "request": request}))
            .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_REQUEST {
        return Err("Supervisor request exceeds the limit.".into());
    }
    bytes.push(b'\n');
    socket.write_all(&bytes).map_err(|e| e.to_string())
}

pub(crate) fn receive_response(layout: &Layout, socket: &Stream) -> Result<serde_json::Value> {
    let response: Response =
        serde_json::from_slice(&read_line(socket, MAX_RESPONSE)?).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        let mut writer = socket;
        writer.write_all(b"!").map_err(|e| e.to_string())?;
    }
    if response.runtime != layout.runtime {
        return Err("Another Fairway installation owns the services. Shut it down using its original command before switching installations.".into());
    }
    match response.error {
        Some(error) => Err(error),
        None => Ok(response.data),
    }
}

pub(crate) fn call(layout: &Layout, request: &Request) -> Result<serde_json::Value> {
    receive_response(layout, &send_request(layout, request)?)
}

pub(crate) fn pid(layout: &Layout) -> Option<u32> {
    fs::read_to_string(layout.data.join("supervisor.pid"))
        .ok()?
        .trim()
        .parse()
        .ok()
}
pub(crate) fn wait_shutdown(layout: &Layout) -> Result<()> {
    wait_shutdown_pid(layout, pid(layout))
}
pub(crate) fn wait_shutdown_pid(layout: &Layout, pid: Option<u32>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while transport::lock(layout)?.is_none()
        || pid.is_some_and(|pid| pid != std::process::id() && crate::process::pid_alive(pid))
    {
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

fn read_line(stream: &Stream, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    BufReader::new(stream.take(limit + 1))
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit || !bytes.ends_with(b"\n") {
        return Err("Invalid or oversized supervisor message.".into());
    }
    Ok(bytes)
}

/// Desktop bundles run their own executable in controller mode. Console and test
/// clients locate the installed desktop binary; Linux runs the terminal binary.
#[cfg(not(test))]
fn executable() -> Result<std::path::PathBuf> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    if current.file_name().is_some_and(|n| n == "fairway-cli.exe") {
        let desktop = current
            .parent()
            .and_then(|p| p.parent())
            .ok_or("Cannot locate Fairway.")?
            .join("fairway.exe");
        if desktop.is_file() {
            return Ok(desktop);
        }
    }
    Ok(current)
}

struct PidGuard(std::path::PathBuf);
impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn serve(layout: &Layout) -> Result<()> {
    layout.initialize()?;
    let Some(mut lock) = transport::lock(layout)? else {
        return Ok(());
    };
    lock.set_len(0).map_err(|e| e.to_string())?;
    write!(lock, "{}", std::process::id()).map_err(|e| e.to_string())?;
    // Refuse an older desktop that still owns services; never adopt its children.
    if let Ok(text) = fs::read_to_string(layout.data.join("app.pid")) {
        if let Ok(pid) = text.trim().parse::<u32>() {
            if crate::process::pid_alive(pid) {
                // New desktops mark the supervisor protocol beside their PID.
                if fs::read_to_string(layout.data.join("desktop-supervisor"))
                    .ok()
                    .and_then(|text| text.trim().parse::<u32>().ok())
                    != Some(pid)
                {
                    return Err(
                        "Quit the older Fairway desktop before using the shared supervisor.".into(),
                    );
                }
            }
        }
    }
    if transport::shared_lock(&layout.data.join("installation.lock"))?.is_none() {
        return Err("Fairway is being updated. Retry when installation finishes.".into());
    }
    let manager = Arc::new(Mutex::new(Manager::new(layout.clone())?));
    crate::config::atomic_write(
        &layout.data.join("supervisor.pid"),
        std::process::id().to_string().as_bytes(),
    )?;
    let _pid_guard = PidGuard(layout.data.join("supervisor.pid"));
    let quitting = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let active = Arc::new(AtomicUsize::new(0));
    let mut listener = Listener::bind(layout)?;
    #[cfg(unix)]
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
            Ok(stream) => {
                if active.load(Ordering::Acquire) >= 16 {
                    let mut stream = stream;
                    let _ = send_response(
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
    // Let the shutdown response finish before dropping the owning process.
    while active.load(Ordering::Acquire) != 0 {
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn send_response(
    stream: &mut Stream,
    layout: &Layout,
    result: Result<serde_json::Value>,
) -> Result<()> {
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
    let mut bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_RESPONSE {
        return Err("Supervisor response exceeds the limit.".into());
    }
    bytes.push(b'\n');
    stream.write_all(&bytes).map_err(|e| e.to_string())?;
    // Windows discards unread pipe data when the last server handle closes.
    // A bounded acknowledgement avoids blocking FlushFileBuffers indefinitely.
    #[cfg(windows)]
    {
        let mut ack = [0];
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        let read = stream.read_exact(&mut ack).map_err(|e| e.to_string());
        stream.set_read_timeout(None).map_err(|e| e.to_string())?;
        read?;
        if ack != *b"!" {
            return Err("Invalid supervisor acknowledgement.".into());
        }
    }
    Ok(())
}

fn handle_connection(
    mut stream: Stream,
    layout: &Layout,
    manager: &Mutex<Manager>,
    quitting: &AtomicBool,
    finished: &AtomicBool,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let first = read_request(&stream, layout);
    let shutdown_requested = first
        .as_ref()
        .is_ok_and(|r| matches!(r, Request::Shutdown | Request::PrepareUpdate));
    let mut shutdown_reserved = false;
    let result = first.and_then(|request| {
        // Discovery/identity must stay available while an operation owns Manager.
        if matches!(request, Request::Hello) {
            return Ok(identity());
        }
        if shutdown_requested
            && quitting
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err("Fairway is already shutting down.".into());
        }
        shutdown_reserved = shutdown_requested;
        let read_only = matches!(
            request,
            Request::Status
                | Request::Diagnostics
                | Request::Log { .. }
                | Request::PhpSettings { .. }
        );
        let deadline = Instant::now() + Duration::from_millis(if read_only { 250 } else { 0 });
        let mut guard = if shutdown_requested {
            manager.lock().map_err(|_| "Service manager failed.")?
        } else {
            loop {
                match manager.try_lock() {
                    Ok(guard) => break guard,
                    Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(TryLockError::WouldBlock) => {
                        return Err("A service operation is in progress. Retry shortly.".into())
                    }
                    Err(_) => return Err("Service manager failed.".into()),
                }
            }
        };
        if transport::shared_lock(&layout.data.join("installation.lock"))?.is_none() {
            return Err("Fairway is being updated. Retry when installation finishes.".into());
        }
        if !shutdown_requested && quitting.load(Ordering::Acquire) {
            return Err("Fairway is shutting down. Retry after it stops.".into());
        }
        if matches!(request, Request::Acquire) {
            // An operation reservation is scoped to this connection. Client exit,
            // cancellation or a broken pipe releases it without stale lease files.
            send_response(&mut stream, layout, Ok(serde_json::json!("Reserved")))?;
            stream.set_read_timeout(None).map_err(|e| e.to_string())?;
            loop {
                let request = read_request(&stream, layout)?;
                if matches!(request, Request::Release) {
                    return Ok(serde_json::json!("Released"));
                }
                let shutdown = matches!(request, Request::Shutdown | Request::PrepareUpdate);
                let response = dispatch(&mut guard, request, quitting);
                let done = shutdown && response.is_ok();
                if done {
                    finished.store(true, Ordering::Release);
                }
                send_response(&mut stream, layout, response)?;
                if done {
                    return Ok(serde_json::Value::Null);
                }
            }
        }
        let shutdown = matches!(request, Request::Shutdown | Request::PrepareUpdate);
        let response = dispatch(&mut guard, request, quitting);
        if shutdown && response.is_ok() {
            finished.store(true, Ordering::Release);
            send_response(&mut stream, layout, response)?;
            return Ok(serde_json::Value::Null);
        }
        response
    });
    if shutdown_reserved && result.is_err() && !finished.load(Ordering::Acquire) {
        quitting.store(false, Ordering::Release);
    }
    // Successful shutdown already wrote its response.
    if !finished.load(Ordering::Acquire) {
        let _ = send_response(&mut stream, layout, result);
    }
}

fn read_request(stream: &Stream, layout: &Layout) -> Result<Request> {
    let envelope: Envelope =
        serde_json::from_slice(&read_line(stream, MAX_REQUEST)?).map_err(|e| e.to_string())?;
    if envelope.runtime != layout.runtime {
        return Err("Another Fairway installation owns the services. Shut it down before switching installations.".into());
    }
    Ok(envelope.request)
}

fn dispatch(
    manager: &mut Manager,
    request: Request,
    quitting: &AtomicBool,
) -> Result<serde_json::Value> {
    if matches!(request, Request::Shutdown | Request::PrepareUpdate) {
        quitting.store(true, Ordering::Release);
        let result = (|| {
            if matches!(request, Request::PrepareUpdate) {
                crate::updates::prepare(manager)?;
            }
            manager.stop_all()?;
            Ok(serde_json::json!("Stopped"))
        })();
        if result.is_err() {
            quitting.store(false, Ordering::Release);
        }
        return result;
    }
    execute(manager, request)
}

fn identity() -> serde_json::Value {
    serde_json::json!({"protocolVersion":1,"version":env!("CARGO_PKG_VERSION")})
}

fn execute(manager: &mut Manager, request: Request) -> Result<serde_json::Value> {
    match request {
        Request::Hello => return Ok(identity()),
        Request::Acquire | Request::Release => return Err("Invalid operation reservation.".into()),
        Request::Settings { ports, version } => manager.save_settings(ports, version)?,
        Request::PhpSettings { version } => {
            return serde_json::to_value(manager.php_settings(&version)?).map_err(|e| e.to_string())
        }
        Request::SavePhpSettings { version, settings } => {
            return serde_json::to_value(manager.save_php_settings(&version, settings)?)
                .map_err(|e| e.to_string())
        }
        Request::RememberAddressApproval { pending } => {
            manager.remember_address_approval(pending)?
        }
        Request::PrepareDesktopUpdate => {
            crate::updates::prepare(manager)?;
        }
        Request::Database { args } => {
            return crate::data_browser::cli_value(&manager.layout, &args)
        }
        Request::Status => {
            return serde_json::to_value(manager.snapshot()).map_err(|e| e.to_string())
        }
        Request::Diagnostics => return Ok(crate::diagnostics::report(&manager.snapshot())),
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
        Request::SaveProject { previous, project } => {
            manager.save_project(previous.as_deref(), project)?
        }
        Request::RemoveProject { domain } => manager.remove_project(&domain)?,
        Request::ProjectHttps { domain, enabled } => manager.set_project_https(&domain, enabled)?,
        Request::InstallCli => {
            integration::install(&manager.layout)?;
            #[cfg(target_os = "linux")]
            {
                let path = manager.layout.bin.join("fairway");
                if path.exists() || path.is_symlink() {
                    fs::remove_file(&path).map_err(|e| e.to_string())?;
                }
                std::os::unix::fs::symlink(
                    std::env::current_exe().map_err(|e| e.to_string())?,
                    path,
                )
                .map_err(|e| e.to_string())?;
            }
            return Ok(serde_json::json!(
                "Command-line tools enabled. Reopen your shell to refresh PATH."
            ));
        }
        Request::ShellTools => {
            #[cfg(unix)]
            integration::write_unix_launchers(&manager.layout)?;
            #[cfg(windows)]
            integration::write_windows_launchers(&manager.layout)?;
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
    if !["all", "nginx", "php", "mysql", "mail"].contains(&service) {
        return Err("Choose all, nginx, mysql or mail.".into());
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;
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
