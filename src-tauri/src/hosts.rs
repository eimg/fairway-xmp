//! Manage only Fairway's marked entries in the system hosts file.
use crate::config::{valid_project_name, Result};
use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};

const BEGIN: &str = "# >>> Fairway XMP project domains >>>";
const END: &str = "# <<< Fairway XMP project domains <<<";

fn managed_names_from(text: &str) -> Result<BTreeSet<String>> {
    let Some(start) = text.find(BEGIN) else {
        if text.contains(END) {
            return Err("The Fairway hosts block is incomplete.".into());
        }
        return Ok(BTreeSet::new());
    };
    if start != 0 && text.as_bytes()[start - 1] != b'\n' {
        return Err("The Fairway hosts marker is not on its own line.".into());
    }
    let body = &text[start + BEGIN.len()..];
    let end = body
        .find(END)
        .ok_or("The Fairway hosts block is incomplete.")?;
    if !body[..end].starts_with('\n') && !body[..end].starts_with("\r\n") {
        return Err("The Fairway hosts block is malformed.".into());
    }
    let mut names = BTreeSet::new();
    for line in body[..end].lines().filter(|line| !line.trim().is_empty()) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 2 || fields[0] != "127.0.0.1" {
            return Err("The Fairway hosts block contains an unexpected entry.".into());
        }
        let name = fields[1]
            .strip_suffix(".test")
            .ok_or("The Fairway hosts block contains an unexpected domain.")?;
        if !valid_project_name(name) || !names.insert(name.to_string()) {
            return Err("The Fairway hosts block contains an invalid project name.".into());
        }
    }
    Ok(names)
}

pub fn managed_names() -> Result<BTreeSet<String>> {
    let text = fs::read_to_string(hosts_path()?).map_err(|e| e.to_string())?;
    managed_names_from(&text)
}

fn hosts_path() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetWindowsDirectoryW(buffer: *mut u16, size: u32) -> u32;
        }
        let mut buffer = [0u16; 32768];
        let length = unsafe { GetWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 || length as usize >= buffer.len() {
            return Err("Cannot locate the Windows hosts file.".into());
        }
        Ok(
            PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length as usize]))
                .join("System32/drivers/etc/hosts"),
        )
    }
    #[cfg(not(windows))]
    {
        Ok(PathBuf::from("/etc/hosts"))
    }
}

fn updated(previous: &str, names: &[String]) -> Result<String> {
    if names.iter().any(|name| !valid_project_name(name)) {
        return Err("Invalid project domain name.".into());
    }
    let mut text = previous.to_string();
    if let Some(start) = text.find(BEGIN) {
        if start != 0 && text.as_bytes()[start - 1] != b'\n' {
            return Err("The Fairway hosts marker is not on its own line.".into());
        }
        let end = text[start..]
            .find(END)
            .ok_or("The Fairway hosts block is incomplete.")?
            + start
            + END.len();
        if !matches!(text.as_bytes().get(end), None | Some(b'\r' | b'\n')) {
            return Err("The Fairway hosts marker is not on its own line.".into());
        }
        let end = if text[end..].starts_with("\r\n") {
            end + 2
        } else if text[end..].starts_with('\n') {
            end + 1
        } else {
            end
        };
        text.replace_range(start..end, "");
    }
    if text.contains(BEGIN) || text.contains(END) {
        return Err("Multiple or incomplete Fairway hosts blocks were found.".into());
    }
    let mut names = names.to_vec();
    names.sort();
    names.dedup();
    for name in &names {
        let domain = format!("{name}.test");
        for line in text.lines() {
            let mut fields = line.split('#').next().unwrap_or("").split_whitespace();
            let Some(address) = fields.next() else {
                continue;
            };
            if fields.any(|alias| alias.eq_ignore_ascii_case(&domain)) {
                return Err(format!("{domain} already has an entry outside Fairway's managed block. Remove that entry before enabling this domain (currently {address})."));
            }
        }
    }
    if names.is_empty() {
        return Ok(text);
    }
    let newline = if previous.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push_str(newline);
    }
    text.push_str(BEGIN);
    text.push_str(newline);
    for name in names {
        text.push_str(&format!("127.0.0.1 {name}.test{newline}"));
    }
    text.push_str(END);
    text.push_str(newline);
    Ok(text)
}

pub fn current(names: &[String]) -> Result<bool> {
    let text = fs::read_to_string(hosts_path()?).map_err(|e| e.to_string())?;
    Ok(updated(&text, names)? == text)
}

// Called by the short-lived elevated helper, never by the long-running service manager.
pub fn apply(names: &[String]) -> Result<()> {
    let path = fs::canonicalize(hosts_path()?).map_err(|e| e.to_string())?;
    let previous = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let next = updated(&previous, names)?;
    if next == previous {
        return Ok(());
    }
    #[cfg(windows)]
    {
        use std::{io::Write, os::windows::ffi::OsStrExt};
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn ReplaceFileW(
                replaced: *const u16,
                replacement: *const u16,
                backup: *const u16,
                flags: u32,
                exclude: *const u8,
                reserved: *const u8,
            ) -> i32;
        }
        let temporary = path.with_extension("fairway-tmp");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(next.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        let target: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let source: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            ReplaceFileW(
                target.as_ptr(),
                source.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        } == 0
        {
            let error = std::io::Error::last_os_error().to_string();
            let _ = fs::remove_file(temporary);
            return Err(error);
        }
    }
    #[cfg(not(windows))]
    crate::config::atomic_write(&path, next.as_bytes())?;
    Ok(())
}

pub fn ensure(names: &[String], helper_flag: &str) -> Result<()> {
    if names.iter().any(|name| !valid_project_name(name)) {
        return Err("Invalid project domain name.".into());
    }
    if current(names)? {
        return Ok(());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let mut command = crate::integration::shell_quote(&executable.to_string_lossy());
        command.push(' ');
        command.push_str(helper_flag);
        for name in names {
            command.push(' ');
            command.push_str(name);
        }
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            command.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let status = Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Local domain setup was not approved or failed.".into());
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::IsTerminal;
        let program = if std::io::stdin().is_terminal() {
            "sudo"
        } else {
            "pkexec"
        };
        let status = Command::new(program)
            .arg(&executable)
            .arg(helper_flag)
            .args(names)
            .status()
            .map_err(|e| format!("Could not start {program}: {e}"))?;
        if !status.success() {
            return Err("Local domain setup was not approved or failed.".into());
        }
    }
    #[cfg(windows)]
    {
        let quoted = |value: &str| format!("'{}'", value.replace('\'', "''"));
        let args = std::iter::once(quoted(helper_flag))
            .chain(names.iter().map(|name| quoted(name)))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!("$p = Start-Process -FilePath {} -ArgumentList @({args}) -Verb RunAs -Wait -PassThru; if ($p.ExitCode -ne 0) {{ exit $p.ExitCode }}", quoted(&executable.to_string_lossy()));
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Local domain setup was not approved or failed.".into());
        }
    }
    if current(names)? {
        Ok(())
    } else {
        Err("Local domain setup did not update the system hosts file.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_entries_preserve_other_hosts_and_remove_deleted_projects() {
        let original = "127.0.0.1 localhost\n# my own entry\n";
        let first = updated(original, &["zoo".into(), "alpha".into()]).unwrap();
        assert!(first.contains("127.0.0.1 alpha.test\n127.0.0.1 zoo.test"));
        assert_eq!(
            managed_names_from(&first).unwrap(),
            BTreeSet::from(["alpha".into(), "zoo".into()])
        );
        assert_eq!(
            updated(&first, &["alpha".into()])
                .unwrap()
                .matches("zoo.test")
                .count(),
            0
        );
        assert_eq!(updated(&first, &[]).unwrap(), original);
        assert!(updated(original, &["bad.name".into()]).is_err());
        let own = "127.0.0.1 localhost alpha.test\n";
        assert!(updated(own, &["alpha".into()]).is_err());
        assert!(updated("192.0.2.1 alpha.test\n", &["alpha".into()]).is_err());
        assert!(managed_names_from("# >>> Fairway XMP project domains >>>\n192.0.2.1 alpha.test\n# <<< Fairway XMP project domains <<<\n").is_err());
    }
}
