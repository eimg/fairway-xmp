use super::{failure, Logs, Result};
use std::{
    collections::VecDeque,
    fs::File,
    io::{Read, Write},
    os::fd::AsRawFd,
    process::{ChildStderr, ChildStdout},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
pub struct Capture {
    file: File,
    disk: usize,
    recent: VecDeque<u8>,
    truncated: bool,
}
impl Capture {
    pub fn new(file: File) -> Self {
        Self {
            file,
            disk: 0,
            recent: VecDeque::new(),
            truncated: false,
        }
    }
    fn append(&mut self, bytes: &[u8]) {
        let take = bytes
            .len()
            .min((1024 * 1024usize).saturating_sub(self.disk));
        match self.file.write_all(&bytes[..take]) {
            Ok(()) => self.disk += take,
            Err(_) => {
                self.disk = 1024 * 1024;
                self.truncated = true;
            }
        }
        self.recent.extend(bytes);
        while self.recent.len() > 64 * 1024 {
            self.recent.pop_front();
            self.truncated = true;
        }
        if take < bytes.len() {
            self.truncated = true;
        }
    }
    #[cfg(test)]
    pub fn test_append(&mut self, bytes: &[u8]) {
        self.append(bytes)
    }
    pub fn read(&self) -> Logs {
        Logs {
            text: String::from_utf8_lossy(&self.recent.iter().copied().collect::<Vec<_>>())
                .into_owned(),
            truncated: self.truncated,
        }
    }
}
pub struct Pump {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Pump {
    pub fn new(
        mut out: ChildStdout,
        mut err: ChildStderr,
        capture: Arc<Mutex<Capture>>,
    ) -> Result<Self> {
        for fd in [out.as_raw_fd(), err.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                return Err(failure("Cannot prepare bounded PostgreSQL output"));
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let thread = thread::spawn(move || {
            fn drain(pipe: &mut impl Read, capture: &Arc<Mutex<Capture>>) {
                let mut b = [0; 4096];
                for _ in 0..32 {
                    match pipe.read(&mut b) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut c) = capture.lock() {
                                c.append(&b[..n]);
                            }
                        }
                    }
                }
            }
            loop {
                drain(&mut out, &capture);
                drain(&mut err, &capture);
                if done.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
        });
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for Pump {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
