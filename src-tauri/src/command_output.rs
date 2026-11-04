//! Bounded subprocess capture for service controls, backups and tool discovery.
use crate::config::Result;
#[cfg(unix)]
use std::io::Read;
use std::{
    fs::File,
    io::Write,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Drain both pipes without blocking the child or retaining unbounded output.
/// Quit/cancel and timeout terminate the process group, including download children.
pub(crate) fn output(cmd: Command, seconds: u64, cancelled: &dyn Fn() -> bool) -> Result<String> {
    run(cmd, Duration::from_secs(seconds), None, None, cancelled)
}

/// Dump output can stream directly to a file; SQL input is written separately so
/// a child that stops reading cannot prevent timeout/cancellation checks.
pub(crate) fn run(
    mut cmd: Command,
    timeout: Duration,
    input: Option<&[u8]>,
    destination: Option<File>,
    cancelled: &dyn Fn() -> bool,
) -> Result<String> {
    if cancelled() {
        return Err("Command cancelled.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(unix)]
    use std::os::fd::AsRawFd;
    #[cfg(unix)]
    fn nonblocking(pipe: &impl AsRawFd) -> Result<()> {
        let flags = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(unix)]
    fn drain(pipe: &mut impl Read, saved: &mut Vec<u8>) {
        let mut buf = [0; 8192];
        for _ in 0..64 {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let take = n.min((2 * 1024 * 1024usize).saturating_sub(saved.len()));
                    saved.extend_from_slice(&buf[..take]);
                }
            }
        }
    }
    #[cfg(windows)]
    use windows::drain;
    let program = cmd.get_program().to_string_lossy().into_owned();
    let capture_stdout = destination.is_none();
    let mut child = cmd
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(destination.map(Stdio::from).unwrap_or_else(Stdio::piped))
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not start {program}: {error}"))?;
    #[cfg(windows)]
    let _job = match windows::Job::new(&child) {
        Ok(job) => Some(job),
        // Fast commands may finish between spawn and job assignment. Their
        // cached exit status and pipes can still be consumed normally.
        Err(_) if child.try_wait().is_ok_and(|status| status.is_some()) => None,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Could not track command {program}: {error}"));
        }
    };
    let writer = input.map(|bytes| {
        let mut stdin = child.stdin.take().expect("command input is piped");
        let bytes = bytes.to_vec();
        let (sent, received) = mpsc::channel();
        thread::spawn(move || {
            let _ = sent.send(stdin.write_all(&bytes));
        });
        received
    });
    let mut stdout = child.stdout.take();
    debug_assert_eq!(stdout.is_some(), capture_stdout);
    let mut stderr = child.stderr.take().ok_or("Missing command errors")?;
    #[cfg(unix)]
    if let Err(error) = stdout
        .as_ref()
        .map_or(Ok(()), nonblocking)
        .and_then(|_| nonblocking(&stderr))
    {
        stop(&mut child);
        return Err(error);
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let start = Instant::now();
    let status = loop {
        if let Some(stdout) = &mut stdout {
            drain(stdout, &mut out);
        }
        drain(&mut stderr, &mut err);
        let cancel = cancelled();
        if cancel || start.elapsed() > timeout {
            stop(&mut child);
            return Err(if cancel {
                "Command cancelled."
            } else {
                "Command timed out."
            }
            .into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(stdout) = &mut stdout {
                    drain(stdout, &mut out);
                }
                drain(&mut stderr, &mut err);
                break status;
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                stop(&mut child);
                return Err(error.to_string());
            }
        }
    };
    let input_error = if let Some(writer) = writer {
        // Never join a writer while an inherited input handle could be open.
        match writer.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(())) => None,
            result => {
                stop(&mut child);
                Some(format!("Command input could not be sent: {result:?}"))
            }
        }
    } else {
        None
    };
    let stdout = String::from_utf8_lossy(&out).trim().to_string();
    let stderr = String::from_utf8_lossy(&err).trim().to_string();
    if !status.success() {
        return Err(format!("Command failed ({status}): {stdout}\n{stderr}")
            .trim()
            .chars()
            .take(4000)
            .collect());
    }
    if let Some(error) = input_error {
        return Err(error);
    }
    Ok(stdout)
}

fn stop(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn python(script: &str) -> Command {
        let mut cmd = crate::process::command(if cfg!(windows) { "python" } else { "python3" });
        cmd.args(["-c", script]);
        cmd
    }

    #[test]
    fn timeout_covers_blocked_input_and_drains_large_errors() {
        let start = Instant::now();
        let error = run(
            python("import time; time.sleep(30)"),
            Duration::from_millis(150),
            Some(&vec![b'x'; 2 * 1024 * 1024]),
            None,
            &|| false,
        )
        .unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(5));
        let error = output(
            python("import sys; sys.stderr.write('x'*3000000); sys.exit(7)"),
            10,
            &|| false,
        )
        .unwrap_err();
        assert!(
            error.contains('7') && error.len() <= 4000,
            "{}",
            error.len()
        );
    }

    #[test]
    fn cancelling_a_dump_stops_its_children_and_keeps_the_output_bounded() {
        let root = std::env::temp_dir().join(format!("fairway-command-{}", rand::random::<u64>()));
        fs::create_dir(&root).unwrap();
        let started = root.join("started");
        let late = root.join("late");
        let script = format!("import subprocess,sys,time,pathlib; subprocess.Popen([sys.executable,'-c',{}]); pathlib.Path({}).touch(); print('partial',flush=True); time.sleep(30)",
            serde_json::to_string(&format!("import time,pathlib; time.sleep(1); pathlib.Path({}).touch()", serde_json::to_string(&late.to_string_lossy()).unwrap())).unwrap(),
            serde_json::to_string(&started.to_string_lossy()).unwrap());
        let partial = root.join("dump.partial");
        let start = Instant::now();
        let error = run(
            python(&script),
            Duration::from_secs(5),
            None,
            Some(fs::File::create(&partial).unwrap()),
            &|| started.exists(),
        )
        .unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(1200));
        assert!(
            !late.exists(),
            "a cancelled command left its descendant running"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn input_and_streamed_output_preserve_bytes_and_failures_keep_diagnostics() {
        let root = std::env::temp_dir().join(format!("fairway-input-{}", rand::random::<u64>()));
        fs::create_dir(&root).unwrap();
        let path = root.join("dump.partial");
        let bytes = vec![b'x'; 256 * 1024];
        run(
            python("import sys; sys.stdout.buffer.write(sys.stdin.buffer.read())"),
            Duration::from_secs(10),
            Some(&bytes),
            Some(File::create(&path).unwrap()),
            &|| false,
        )
        .unwrap();
        assert_eq!(fs::read(path).unwrap(), bytes);
        let error = run(
            python("import sys; sys.stderr.write('fixture failure'); sys.exit(7)"),
            Duration::from_secs(10),
            Some(&bytes),
            None,
            &|| false,
        )
        .unwrap_err();
        assert!(
            error.contains("fixture failure") && error.contains('7'),
            "{error}"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(windows)]
mod windows {
    use crate::config::Result;
    use std::{
        io::Read,
        os::windows::io::{AsRawHandle, RawHandle},
        process::Child,
    };
    pub(super) fn drain(pipe: &mut (impl Read + AsRawHandle), saved: &mut Vec<u8>) {
        use windows_sys::Win32::System::Pipes::PeekNamedPipe;
        let mut buf = [0; 8192];
        for _ in 0..64 {
            let mut available = 0;
            let ok = unsafe {
                PeekNamedPipe(
                    pipe.as_raw_handle() as _,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || available == 0 {
                break;
            }
            let count = buf.len().min(available as usize);
            match pipe.read(&mut buf[..count]) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let count = n.min((2 * 1024 * 1024usize).saturating_sub(saved.len()));
                    saved.extend_from_slice(&buf[..count]);
                }
            }
        }
    }

    pub(super) struct Job(RawHandle);
    impl Job {
        pub(super) fn new(child: &Child) -> Result<Self> {
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as _,
                    std::mem::size_of_val(&info) as u32,
                ) == 0
                    || AssignProcessToJobObject(handle, child.as_raw_handle() as _) == 0
                {
                    let error = std::io::Error::last_os_error().to_string();
                    CloseHandle(handle);
                    return Err(error);
                }
                Ok(Self(handle as _))
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
            }
        }
    }
}
