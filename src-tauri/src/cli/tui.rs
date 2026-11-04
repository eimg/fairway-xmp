use super::{supervisor, Request};
use crate::{
    config::{Layout, Ports, Result},
    manager::Snapshot,
};
use std::{
    io::{self, Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

static EXIT: AtomicBool = AtomicBool::new(false);
extern "C" fn signal_handler(_: libc::c_int) {
    EXIT.store(true, Ordering::Relaxed);
}

struct Terminal {
    original: libc::termios,
}
impl Terminal {
    fn enter() -> Result<Self> {
        let mut original = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(0, &mut original) } != 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        let terminal = Self { original };
        terminal.resume()?;
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = signal_handler as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
            libc::sigaction(libc::SIGHUP, &action, std::ptr::null_mut());
        }
        Ok(terminal)
    }
    fn resume(&self) -> Result<()> {
        let mut raw = self.original;
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        if unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        print!("\x1b[?1049h\x1b[?25l");
        io::stdout().flush().map_err(|e| e.to_string())
    }
    fn pause(&self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.original);
        }
        print!("\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.pause();
    }
}

const SERVICES: [&str; 3] = ["nginx", "mysql", "mail"];

fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

pub(super) fn run(layout: &Layout) -> Result<()> {
    if std::env::var("TERM").is_ok_and(|s| s == "dumb") {
        return Err("Use fairway status in a terminal without ANSI support.".into());
    }
    let terminal = Terminal::enter()?;
    let (sender, receiver) = mpsc::channel::<(bool, Result<serde_json::Value>)>();
    let mut snapshot: Option<Snapshot> = None;
    let mut selected = 0;
    let mut message =
        "q closes this dashboard and leaves services running. Q shuts everything down.".to_string();
    let mut log: Option<String> = None;
    let mut action_pending = false;
    let mut suppress_action_reply = false;
    let mut input = std::collections::VecDeque::new();
    let mut shutting_down = false;
    let mut refresh = Instant::now() - Duration::from_secs(3);
    while !EXIT.load(Ordering::Relaxed) {
        if let Ok((shutdown_reply, result)) = receiver.try_recv() {
            if shutdown_reply {
                shutting_down = false;
                match result {
                    Ok(_) => {
                        supervisor::wait_shutdown(layout)?;
                        return Ok(());
                    }
                    Err(error) => message = clean(&error),
                }
            } else {
                action_pending = false;
                if suppress_action_reply {
                    suppress_action_reply = false;
                } else if !shutting_down {
                    message = match result {
                        Ok(value) => clean(value.as_str().unwrap_or("Done.")),
                        Err(error) => clean(&error),
                    };
                }
            }
            refresh = Instant::now() - Duration::from_secs(3);
        }
        let busy = action_pending || shutting_down;
        if !busy && refresh.elapsed() >= Duration::from_secs(2) {
            match supervisor::call(layout, &Request::Status)
                .and_then(|data| serde_json::from_value(data).map_err(|e| e.to_string()))
            {
                Ok(value) => snapshot = Some(value),
                Err(error) => message = clean(&error),
            }
            refresh = Instant::now();
        }
        draw(snapshot.as_ref(), selected, busy, &message, log.as_deref())?;
        let Some(key) = next_key(&mut input)? else {
            continue;
        };
        if key.is_empty() {
            break;
        }
        let key = key.as_slice();
        if matches!(key, b"q" | b"\x03") {
            break;
        }
        if matches!(key, b"\x1b[A" | b"k") {
            selected = selected.saturating_sub(1);
            continue;
        }
        if matches!(key, b"\x1b[B" | b"j") {
            selected = (selected + 1).min(SERVICES.len() - 1);
            continue;
        }
        if key == b"Q" {
            if !shutting_down {
                match spawn_command(layout, Request::Shutdown, &sender, true) {
                    Ok(()) => {
                        shutting_down = true;
                        suppress_action_reply = action_pending;
                        message =
                            "Shutting down… Waiting for any current operation to finish.".into();
                    }
                    Err(error) => message = clean(&error),
                }
            }
            continue;
        }
        if busy {
            continue;
        }
        let service = SERVICES[selected].to_string();
        let request = match key {
            b"s" => Request::Start { service },
            b"x" => Request::Stop { service },
            b"r" => Request::Restart { service },
            b"a" => Request::Start {
                service: "all".into(),
            },
            b"z" => Request::Stop {
                service: "all".into(),
            },
            b"b" => Request::Backup { id: None },
            b"l" | b"f" => {
                let id = if key == b"f" { "php" } else { &service };
                log = Some(
                    match supervisor::call(layout, &Request::Log { service: id.into() }) {
                        Ok(value) => clean(value.as_str().unwrap_or("")),
                        Err(error) => error,
                    },
                );
                continue;
            }
            b"\x1b" => {
                log = None;
                continue;
            }
            b"p" => {
                terminal.pause();
                println!("Enter ports: website phpMyAdmin MySQL PHP-FPM [mail-inbox SMTP] (blank cancels).");
                let mut line = String::new();
                let result = io::stdin().read_line(&mut line);
                terminal.resume()?;
                result.map_err(|e| e.to_string())?;
                if line.trim().is_empty() {
                    continue;
                }
                let values = line
                    .split_whitespace()
                    .map(str::parse::<u16>)
                    .collect::<std::result::Result<Vec<_>, _>>();
                match values {
                    Ok(values) if values.len() == 4 || values.len() == 6 => Request::Ports {
                        ports: Ports {
                            website: values[0],
                            phpmyadmin: values[1],
                            mysql: values[2],
                            php: values[3],
                            mail: values.get(4).copied().unwrap_or(Ports::default().mail),
                            smtp: values.get(5).copied().unwrap_or(Ports::default().smtp),
                        },
                    },
                    _ => {
                        message = "Enter four or six valid port numbers.".into();
                        continue;
                    }
                }
            }
            _ => continue,
        };
        match spawn_command(layout, request, &sender, false) {
            Ok(()) => {
                action_pending = true;
                message = "Working… q closes the dashboard; Q queues graceful shutdown.".into();
            }
            Err(error) => message = clean(&error),
        }
    }
    Ok(())
}

fn spawn_command(
    layout: &Layout,
    request: Request,
    sender: &mpsc::Sender<(bool, Result<serde_json::Value>)>,
    shutdown: bool,
) -> Result<()> {
    // Deliver the request before acknowledging the key. Closing the dashboard
    // immediately afterwards must not cancel an accepted service action.
    let stream = supervisor::send_request(layout, &request)?;
    let layout = layout.clone();
    let sender = sender.clone();
    thread::spawn(move || {
        let _ = sender.send((shutdown, supervisor::receive_response(&layout, &stream)));
    });
    Ok(())
}

fn read_input(pending: &mut std::collections::VecDeque<u8>, timeout: i32) -> Result<Option<bool>> {
    let mut descriptor = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    if unsafe { libc::poll(&mut descriptor, 1, timeout) } <= 0 {
        return Ok(None);
    }
    let mut bytes = [0; 32];
    let count = io::stdin().read(&mut bytes).map_err(|e| e.to_string())?;
    pending.extend(&bytes[..count]);
    Ok(Some(count != 0))
}

fn next_key(pending: &mut std::collections::VecDeque<u8>) -> Result<Option<Vec<u8>>> {
    if pending.is_empty() {
        match read_input(pending, 200)? {
            None => return Ok(None),
            Some(false) => return Ok(Some(Vec::new())),
            Some(true) => (),
        }
    }
    let first = pending.pop_front().unwrap();
    if first != 27 {
        return Ok(Some(vec![first]));
    }
    // Allow split ANSI arrow sequences, while retaining following ordinary keys.
    if pending.len() < 2 {
        let _ = read_input(pending, 50)?;
    }
    if pending.len() >= 2 && pending.front() == Some(&b'[') {
        let middle = pending.pop_front().unwrap();
        let last = pending.pop_front().unwrap();
        return Ok(Some(vec![first, middle, last]));
    }
    Ok(Some(vec![first]))
}

fn draw(
    snapshot: Option<&Snapshot>,
    selected: usize,
    busy: bool,
    message: &str,
    log: Option<&str>,
) -> Result<()> {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    unsafe {
        libc::ioctl(1, libc::TIOCGWINSZ, &mut size);
    }
    let rows = if size.ws_row == 0 {
        24
    } else {
        size.ws_row as usize
    };
    let columns = if size.ws_col == 0 {
        80
    } else {
        size.ws_col as usize
    };
    let mut lines = vec![
        format!(
            "\x1b[1mFairway XMP {}\x1b[0m  •  Linux terminal edition",
            env!("CARGO_PKG_VERSION")
        ),
        String::new(),
    ];
    if let Some(snapshot) = snapshot {
        for (index, id) in SERVICES.iter().enumerate() {
            if let Some(service) = snapshot.services.iter().find(|s| s.id == *id) {
                let (color, label) = if service.ready {
                    (32, "Ready")
                } else if service.running {
                    (33, "Starting / not ready")
                } else {
                    (31, "Stopped")
                };
                lines.push(format!(
                    "{} \x1b[{color}m●\x1b[0m {:<7} {:<22} port {}",
                    if index == selected { ">" } else { " " },
                    id,
                    label,
                    service.port
                ));
            }
        }
        lines.push("  PHP-FPM follows Nginx; PHP CLI is always available.".into());
        lines.push(format!(
            "  Website:    http://127.0.0.1:{}/",
            snapshot.ports.website
        ));
        lines.push(format!(
            "  phpMyAdmin: http://127.0.0.1:{}/",
            snapshot.ports.phpmyadmin
        ));
        lines.push(format!(
            "  Mail inbox: http://127.0.0.1:{}/  (SMTP 127.0.0.1:{})",
            snapshot.ports.mail, snapshot.ports.smtp
        ));
        lines.push(format!(
            "  Document root: {}",
            clean(&snapshot.document_root)
        ));
        lines.push(format!("  Data:       {}", clean(&snapshot.data_dir)));
        if let Some(error) = &snapshot.sites_error {
            lines.push(format!(
                "  Saved sites could not be loaded: {}",
                clean(error)
            ));
        }
    }
    lines.push(String::new());
    lines.push("↑/↓ Select  s Start  x Stop  r Restart  a Start all  z Stop all".into());
    lines.push("l Logs  f PHP logs  p Ports  b Backup  q Close  Q Shutdown".into());
    lines.push(String::new());
    for line in message.lines().take(3) {
        lines.push(line.chars().take(columns).collect());
    }
    if let Some(log) = log {
        lines.push("── Recent log entries (Esc hides) ──".into());
        let room = rows.saturating_sub(lines.len() + 1);
        let entries: Vec<_> = log.lines().collect();
        for line in entries.iter().skip(entries.len().saturating_sub(room)) {
            lines.push(line.chars().take(columns).collect());
        }
    }
    print!("\x1b[H\x1b[2J");
    for line in lines.into_iter().take(rows.saturating_sub(1)) {
        print!("{line}\r\n");
    }
    if busy {
        print!("\x1b[33mOperation in progress\x1b[0m");
    }
    io::stdout().flush().map_err(|e| e.to_string())
}
