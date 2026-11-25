//! Owned helper process only. Cancellation never signals a PostgreSQL server PID.
use super::{
    super::explorer_contract::*,
    wire::{Envelope, PrivateBytes},
};
use std::{
    io::{Read, Write},
    os::unix::process::CommandExt,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
pub fn run(e: Envelope, cancel: Arc<AtomicBool>) -> Result<Output> {
    let writing = matches!(
        e.operation,
        Operation::Query {
            mode: QueryMode::ReadWrite,
            ..
        }
    );
    let mut bytes = PrivateBytes(
        serde_json::to_vec(&e)
            .map_err(|_| fail(Code::InvalidRequest, "Private request encoding failed"))?,
    );
    if bytes.0.len() > 1024 * 1024 - 1 {
        return Err(fail(Code::InvalidRequest, "Private request limit exceeded"));
    }
    bytes.0.push(b'\n');
    let executable = std::env::current_exe().map_err(|e| {
        #[cfg(test)]
        eprintln!(
            "Query worker spawn failed with OS code {:?}",
            e.raw_os_error()
        );
        #[cfg(not(test))]
        let _ = e;
        fail(Code::Unavailable, "Query worker unavailable")
    })?;
    #[cfg(test)]
    let executable = std::env::var_os("FAIRWAY_TEST_POSTGRES_WORKER")
        .map(std::path::PathBuf::from)
        .unwrap_or(executable);
    let (read, mut write) = std::os::unix::net::UnixStream::pair()
        .map_err(|_| fail(Code::Unavailable, "Private pipe unavailable"))?;
    write
        .set_nonblocking(true)
        .map_err(|_| fail(Code::Unavailable, "Private pipe unavailable"))?;
    let fd: std::os::fd::OwnedFd = read.into();
    let mut command = crate::process::command(executable);
    command
        .arg("--fairway-postgres-query-worker")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .current_dir(&e.runtime)
        .stdin(Stdio::from(fd))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().map_err(|e| {
        #[cfg(test)]
        eprintln!(
            "Query worker spawn failed with OS code {:?}",
            e.raw_os_error()
        );
        #[cfg(not(test))]
        let _ = e;
        fail(Code::Unavailable, "Query worker unavailable")
    })?;
    drop(command);
    drop(e);
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| fail(Code::Unavailable, "Query worker output unavailable"))?;
    let output = std::thread::spawn(move || {
        let mut b = Vec::new();
        let result = stdout.take(2 * 1024 * 1024 + 1).read_to_end(&mut b);
        (result, b)
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut sent = 0;
    let mut cancelled = false;
    let mut monitor_failures = 0;
    let mut memory_limited = false;
    let status = loop {
        if sent < bytes.0.len() {
            match write.write(&bytes.0[sent..]) {
                Ok(n) => sent += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                _ => break None,
            }
        } else if cancel.load(Ordering::SeqCst) && !cancelled {
            match write.write(b"!") {
                Ok(1) => cancelled = true,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                _ => break None,
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => {
                let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v0>::zeroed();
                if unsafe {
                    libc::proc_pid_rusage(
                        child.id() as i32,
                        libc::RUSAGE_INFO_V0,
                        usage.as_mut_ptr().cast(),
                    )
                } == 0
                {
                    monitor_failures = 0;
                    if unsafe { usage.assume_init() }.ri_phys_footprint > 256 * 1024 * 1024 {
                        memory_limited = true;
                        break None;
                    }
                } else {
                    monitor_failures += 1;
                    if monitor_failures >= 5 {
                        memory_limited = true;
                        break None;
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => break None,
        }
    };
    drop(bytes);
    drop(write);
    if status.is_none() {
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        };
        let _ = child.wait();
    }
    let (read, b) = output
        .join()
        .map_err(|_| fail(Code::Unavailable, "Query output unavailable"))?;
    let failure = || {
        let mut e = fail(
            if memory_limited {
                Code::ResultLimit
            } else if cancel.load(Ordering::SeqCst) {
                Code::Cancelled
            } else {
                Code::TimedOut
            },
            "Query worker ended without a verified outcome",
        );
        if writing {
            e.outcome = Outcome::Unknown;
        }
        e
    };
    if status.is_none_or(|s| !s.success()) || read.is_err() || b.len() > 2 * 1024 * 1024 {
        return Err(failure());
    }
    serde_json::from_slice::<Result<Output>>(&b).map_err(|_| failure())?
}
