//! Publish completed backups only after their contents and directory entries are flushed.
use crate::config::Result;
use std::{fs, path::Path};

pub fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| error.to_string())?;
    // Windows publication uses MOVEFILE_WRITE_THROUGH below. Opening a
    // directory with File::open/FlushFileBuffers is not supported there.
    #[cfg(windows)]
    let _ = path;
    Ok(())
}

pub fn sync_tree(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            sync_tree(&entry.path())?;
        } else if kind.is_file() {
            #[cfg(unix)]
            let file = fs::File::open(entry.path());
            #[cfg(windows)]
            let file = fs::OpenOptions::new().write(true).open(entry.path());
            file.and_then(|file| file.sync_all())
                .map_err(|error| error.to_string())?;
        } else {
            return Err("Unexpected link or special file in the backup.".into());
        }
    }
    sync_directory(path)
}

pub fn publish(from: &Path, to: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::rename(from, to).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        unsafe extern "system" {
            fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
        }
        let from: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x8) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    sync_directory(to.parent().ok_or("The backup directory is unavailable.")?)
}

pub fn publish_file(file: fs::File, from: &Path, to: &Path) -> Result<()> {
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    publish(from, to)
}

/// Publish an export without replacing a destination created by another caller.
pub fn publish_new_file(file: fs::File, from: &Path, to: &Path) -> Result<()> {
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    #[cfg(unix)]
    {
        fs::hard_link(from, to).map_err(|error| error.to_string())?;
        sync_directory(to.parent().ok_or("The backup directory is unavailable.")?)
    }
    #[cfg(windows)]
    {
        publish(from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_exports_never_replace_existing_destinations() {
        let root =
            std::env::temp_dir().join(format!("fairway-new-export-{}", rand::random::<u64>()));
        fs::create_dir(&root).unwrap();
        let partial = root.join("dump.partial");
        let destination = root.join("dump.sql");
        fs::write(&partial, b"new database").unwrap();
        fs::write(&destination, b"previous backup").unwrap();
        let open = || fs::OpenOptions::new().write(true).open(&partial).unwrap();
        assert!(publish_new_file(open(), &partial, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"previous backup");
        assert_eq!(fs::read(&partial).unwrap(), b"new database");
        fs::remove_file(&destination).unwrap();
        publish_new_file(open(), &partial, &destination).unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"new database");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_publication_keeps_the_unpublished_file() {
        let root = std::env::temp_dir().join(format!("fairway-publish-{}", rand::random::<u64>()));
        fs::create_dir(&root).unwrap();
        let partial = root.join("backup.partial");
        fs::write(&partial, b"student database").unwrap();
        let file = fs::OpenOptions::new().write(true).open(&partial).unwrap();
        assert!(publish_file(file, &partial, &root.join("missing/backup.sql")).is_err());
        assert_eq!(fs::read(&partial).unwrap(), b"student database");
        assert!(!root.join("backup.sql").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
