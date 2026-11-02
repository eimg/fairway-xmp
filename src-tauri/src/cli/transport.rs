//! User-local IPC. No TCP listener or remote management endpoint.
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(super) use unix::*;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(super) use windows::*;
