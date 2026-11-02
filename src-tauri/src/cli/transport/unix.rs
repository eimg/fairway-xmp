use crate::config::{Layout, Result};
pub(in crate::cli) use std::os::unix::net::UnixStream as Stream;
use std::{
    fs::{self, File, OpenOptions},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::PathBuf,
};

pub(in crate::cli) struct Listener {
    socket: std::os::unix::net::UnixListener,
    path: PathBuf,
    temporary_directory: bool,
}

impl Listener {
    pub fn bind(layout: &Layout) -> Result<Self> {
        let path = socket_path(layout, true).map_err(|e| e.to_string())?;
        match fs::remove_file(&path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
        let socket = std::os::unix::net::UnixListener::bind(&path).map_err(|e| {
            format!("Cannot create supervisor socket (the data path may be too long): {e}")
        })?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        let temporary_directory = path != layout.data.join("supervisor.sock");
        Ok(Self {
            socket,
            path,
            temporary_directory,
        })
    }
    pub fn accept(&mut self) -> std::io::Result<Stream> {
        let (stream, _) = self.socket.accept()?;
        // BSD/macOS accepts inherit O_NONBLOCK from the listener.
        stream.set_nonblocking(false)?;
        Ok(stream)
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        if self.temporary_directory {
            if let Some(directory) = self.path.parent() {
                // Remove only our empty private fallback directory, preserving
                // any unexpected contents rather than recursively deleting them.
                let _ = fs::remove_dir(directory);
            }
        }
    }
}
pub(in crate::cli) fn connect(layout: &Layout) -> std::io::Result<Stream> {
    Stream::connect(socket_path(layout, false)?)
}

fn socket_path(layout: &Layout, create_directory: bool) -> std::io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let path = layout.data.join("supervisor.sock");
    if path.as_os_str().as_bytes().len() < 100 {
        return Ok(path);
    }
    // macOS sockaddr_un has a short path limit. A private, verified directory
    // keeps long and Unicode user-data paths usable without exposing the socket.
    let digest = ring::digest::digest(&ring::digest::SHA256, layout.data.as_os_str().as_bytes());
    let hash: String = digest.as_ref()[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let uid = unsafe { libc::geteuid() };
    let directory = PathBuf::from(format!("/tmp/fairway-{uid}-{hash}"));
    if create_directory {
        match fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
    }
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Supervisor socket directory is not private.",
        ));
    }
    Ok(directory.join("controller.sock"))
}
pub(in crate::cli) fn lock(layout: &Layout) -> Result<Option<File>> {
    lock_path(&layout.data.join("supervisor.lock"))
}
pub(in crate::cli) fn lock_path(path: &std::path::Path) -> Result<Option<File>> {
    file_lock(path, libc::LOCK_EX)
}
pub(in crate::cli) fn shared_lock(path: &std::path::Path) -> Result<Option<File>> {
    file_lock(path, libc::LOCK_SH)
}
fn file_lock(path: &std::path::Path, mode: libc::c_int) -> Result<Option<File>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(file.as_raw_fd(), mode | libc::LOCK_NB) } != 0 {
        let error = std::io::Error::last_os_error();
        return if error.kind() == std::io::ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(error.to_string())
        };
    }
    Ok(Some(file))
}
#[cfg(not(test))]
pub(in crate::cli) fn detach(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

#[cfg(not(test))]
pub(in crate::cli) fn spawn(
    command: &mut std::process::Command,
) -> std::io::Result<std::process::Child> {
    command.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_path_socket_is_private_and_its_directory_is_removed_on_shutdown() {
        let root = std::env::temp_dir().join(format!("fw-socket-{}", rand::random::<u64>()));
        let layout = Layout::at(
            root.join("home"),
            root.join("long student data path ".repeat(8)),
            root.join("runtime"),
        );
        fs::create_dir_all(&layout.data).unwrap();
        let listener = Listener::bind(&layout).unwrap();
        assert!(listener.temporary_directory);
        let directory = listener.path.parent().unwrap().to_owned();
        let metadata = fs::symlink_metadata(&directory).unwrap();
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.mode() & 0o777, 0o700);
        drop(connect(&layout).unwrap());
        drop(listener);
        assert!(!directory.exists());
        assert!(connect(&layout).is_err());
        assert!(
            !directory.exists(),
            "A readiness probe must not recreate the directory"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
