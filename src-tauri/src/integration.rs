use crate::{
    config::{atomic_write, Layout, Result},
    process,
};
use std::{env, fs, path::Path};

const BEGIN: &str = "# >>> fairway environment >>>";
const END: &str = "# <<< fairway environment <<<";

#[cfg(unix)]
#[derive(serde::Serialize, serde::Deserialize)]
struct ProfileRecord {
    path: std::path::PathBuf,
    created: bool,
}

pub fn resolve(name: &str) -> Option<String> {
    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        #[cfg(windows)]
        let candidates = [
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.bat"),
        ];
        #[cfg(not(windows))]
        let candidates = [name.to_string()];
        for candidate in candidates {
            let file = directory.join(candidate);
            if file.is_file() {
                return Some(file.to_string_lossy().into());
            }
        }
    }
    None
}

pub fn installed(layout: &Layout) -> bool {
    layout.data.join("cli-installed").exists()
        && launcher(layout, "php").is_some()
        && launcher(layout, "composer").is_some()
}

// These paths describe Fairway's own commands, independently of the desktop
// process's inherited PATH (which often excludes shell profiles on macOS).
pub fn launcher(layout: &Layout, name: &str) -> Option<String> {
    let filename = if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_string()
    };
    let path = layout.bin.join(filename);
    path.is_file().then(|| path.to_string_lossy().into())
}

#[cfg(unix)]
pub fn write_unix_launchers(layout: &Layout) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let base = format!(
            "#!/bin/sh\nexport PHP_INI_SCAN_DIR=''\nexport MIBS=''\nexport PHPRC={}\nexport COMPOSER_HOME={}\n",
            shell_quote(&layout.data.join("config/php.ini").to_string_lossy()),
            shell_quote(&layout.data.join("composer").to_string_lossy())
        );
    let php = shell_quote(&layout.php_exe("php")?.to_string_lossy());
    let scripts = [
        ("php", format!("{base}exec {php} \"$@\"\n")),
        (
            "composer",
            format!(
                "{base}exec {php} {} \"$@\"\n",
                shell_quote(&layout.runtime.join("composer.phar").to_string_lossy())
            ),
        ),
    ];
    for (name, text) in scripts {
        let path = layout.bin.join(name);
        atomic_write(&path, text.as_bytes())?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        // Test/example binaries must not replace the desktop command launcher.
        if executable.file_name().is_some_and(|name| name == "fairway") {
            let path = layout.bin.join("fairway");
            atomic_write(
                &path,
                format!(
                    "#!/bin/sh\nexec {} \"$@\"\n",
                    shell_quote(&executable.to_string_lossy())
                )
                .as_bytes(),
            )?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn write_windows_launchers(layout: &Layout) -> Result<()> {
    // cmd expands percent signs in filenames; reject them rather than create broken launchers.
    if [&layout.data, &layout.runtime]
        .iter()
        .any(|p| p.to_string_lossy().contains('%'))
    {
        return Err("The installation path contains a percent sign, which Windows command launchers cannot support.".into());
    }
    // Batch sources are UTF-8 so Windows user/install paths can contain Unicode.
    let base = format!("@echo off\r\nchcp 65001 >nul\r\nsetlocal\r\nset \"PHP_INI_SCAN_DIR=\"\r\nset \"PHPRC={}\"\r\nset \"COMPOSER_HOME={}\"\r\n",
        crate::config::windows_ansi_path(&layout.data.join("config/php.ini")).display(), layout.data.join("composer").display());
    atomic_write(
        &layout.bin.join("php.cmd"),
        format!("{base}\"{}\" %*\r\n", layout.php_exe("php")?.display()).as_bytes(),
    )?;
    atomic_write(
        &layout.bin.join("composer.cmd"),
        format!(
            "{base}\"{}\" \"{}\" %*\r\n",
            layout.php_exe("php")?.display(),
            layout.runtime.join("composer.phar").display()
        )
        .as_bytes(),
    )?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    if executable
        .file_name()
        .is_some_and(|name| name == "fairway.exe")
    {
        if executable.to_string_lossy().contains('%') {
            return Err("The application path contains a percent sign, which Windows command launchers cannot support.".into());
        }
        atomic_write(
            &layout.bin.join("fairway.cmd"),
            format!(
                "@echo off\r\nchcp 65001 >nul\r\n\"{}\" %*\r\n",
                executable.display()
            )
            .as_bytes(),
        )?;
    }
    Ok(())
}

pub fn install(layout: &Layout) -> Result<()> {
    if !layout.php_exe("php")?.is_file() || !layout.runtime.join("composer.phar").is_file() {
        return Err("Install the complete runtime before enabling command-line tools.".into());
    }
    layout.initialize()?;
    layout.write_configs(&layout.ports()?)?;
    #[cfg(unix)]
    {
        write_unix_launchers(layout)?;
        let block = format!(
            "{BEGIN}\nexport PATH={}:\"$PATH\"\n{END}",
            shell_quote(&layout.bin.to_string_lossy())
        );
        let zdot = env::var_os("ZDOTDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| layout.home.clone());
        let files = [
            zdot.join(".zprofile"),
            zdot.join(".zshrc"),
            layout.home.join(".bash_profile"),
            layout.home.join(".bashrc"),
        ];
        let record_path = layout.data.join("cli-profiles.json");
        let old_records: Vec<ProfileRecord> = fs::read(&record_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut records = Vec::new();
        let mut edits = Vec::new();
        for file in &files {
            let file = if fs::symlink_metadata(file).is_ok_and(|m| m.file_type().is_symlink()) {
                fs::canonicalize(file).map_err(|e| e.to_string())?
            } else {
                file.clone()
            };
            let created = old_records
                .iter()
                .find(|r| r.path == file)
                .map(|r| r.created)
                .unwrap_or_else(|| !file.exists());
            let previous = read_profile(&file)?;
            let updated = profile_with_block(&previous, Some(&block))?;
            edits.push((file.clone(), updated));
            records.push(ProfileRecord {
                path: file,
                created,
            });
        }
        atomic_write(
            &record_path,
            &serde_json::to_vec(&records).map_err(|e| e.to_string())?,
        )?;
        for (file, updated) in edits {
            atomic_write(&file, updated.as_bytes())?;
        }
    }
    #[cfg(windows)]
    {
        write_windows_launchers(layout)?;
        windows_path(&layout.bin, true)?;
    }
    atomic_write(&layout.data.join("cli-installed"), b"1")?;
    Ok(())
}

pub fn remove(layout: &Layout) -> Result<()> {
    #[cfg(unix)]
    {
        let record = layout.data.join("cli-profiles.json");
        if let Ok(bytes) = fs::read(&record) {
            let files: Vec<ProfileRecord> =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            for record in files {
                let file = record.path;
                if file.exists() {
                    let previous = read_profile(&file)?;
                    let updated = profile_with_block(&previous, None)?;
                    if record.created && updated.is_empty() {
                        fs::remove_file(file).map_err(|e| e.to_string())?;
                    } else {
                        atomic_write(&file, updated.as_bytes())?;
                    }
                }
            }
            fs::remove_file(record).map_err(|e| e.to_string())?;
        }
    }
    #[cfg(windows)]
    windows_path(&layout.bin, false)?;
    for name in ["php", "composer", "php.cmd", "composer.cmd"] {
        let file = layout.bin.join(name);
        if file.exists() {
            fs::remove_file(file).map_err(|e| e.to_string())?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let launcher = layout.bin.join("fairway");
        if launcher.exists() {
            fs::remove_file(launcher).map_err(|e| e.to_string())?;
        }
    }
    let marker = layout.data.join("cli-installed");
    if marker.exists() {
        fs::remove_file(marker).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(unix)]
fn read_profile(file: &Path) -> Result<String> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!(
            "Cannot read {}: {e}. File has not been changed.",
            file.display()
        )),
    }
}

pub fn profile_with_block(previous: &str, block: Option<&str>) -> Result<String> {
    let mut text = previous.to_string();
    while let Some(start) = text.find(BEGIN) {
        let end = text[start..].find(END).ok_or(
            "The existing Fairway XMP PATH block is incomplete; repair it before continuing.",
        )? + start
            + END.len();
        let end = if text[end..].starts_with('\n') {
            end + 1
        } else {
            end
        };
        text.replace_range(start..end, "");
    }
    if let Some(block) = block {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(block);
        text.push('\n');
    }
    Ok(text)
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn windows_path(bin: &Path, add: bool) -> Result<()> {
    use winreg::{enums::*, RegKey};
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root
        .create_subkey("Environment")
        .map_err(|e| e.to_string())?;
    let old: String = match key.get_value("Path") {
        Ok(value) => value,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let entry = bin.to_string_lossy();
    let mut parts: Vec<String> = old
        .split(';')
        .filter(|part| {
            !part
                .trim()
                .trim_matches('"')
                .trim_end_matches('\\')
                .eq_ignore_ascii_case(entry.trim_end_matches('\\'))
        })
        .map(String::from)
        .collect();
    if old.is_empty() {
        parts.clear();
    }
    if add {
        parts.insert(0, entry.into());
    }
    let text = parts.join(";");
    use std::os::windows::ffi::OsStrExt;
    let bytes: Vec<u8> = std::ffi::OsStr::new(&text)
        .encode_wide()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    key.set_raw_value(
        "Path",
        &winreg::RegValue {
            bytes,
            vtype: REG_EXPAND_SZ,
        },
    )
    .map_err(|e| e.to_string())?;
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let environment: Vec<u16> = "Environment\0".encode_utf16().collect();
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            std::ptr::null_mut(),
        );
    }
    Ok(())
}

pub fn project_url(project: &crate::config::ProjectConfig, website: u16) -> String {
    if project.https {
        format!(
            "https://{}.test:{}/",
            project.domain,
            crate::tls::https_port()
        )
    } else {
        format!("http://{}.test:{website}/", project.domain)
    }
}

pub fn open(layout: &Layout, target: &str, website: u16, admin: u16) -> Result<()> {
    let path = match target {
        "website" => format!("http://127.0.0.1:{website}/"),
        "phpmyadmin" => format!("http://127.0.0.1:{admin}/"),
        "releases" => "https://github.com/eimg/fairway-xmp/releases".into(),
        "source" => "https://github.com/eimg/fairway-xmp".into(),
        "issues" => "https://github.com/eimg/fairway-xmp/issues".into(),
        name if name.starts_with("project:") => layout
            .registered_projects()?
            .into_iter()
            .find(|project| project.domain == name[8..] && project.root.is_dir())
            .map(|project| project_url(&project, website))
            .ok_or(
                "This project's web root folder is missing. Edit the project to choose it again.",
            )?,
        "coursework" => layout.www.to_string_lossy().into(),
        "logs" => layout.data.join("logs").to_string_lossy().into(),
        "backups" => layout.data.join("backups").to_string_lossy().into(),
        "terminal" => {
            #[cfg(target_os = "macos")]
            {
                let script = layout.data.join("fairway-terminal.command");
                let text = format!(
                    "#!/bin/sh\nexport PATH={}:\"$PATH\"\ncd {}\nexec /bin/zsh\n",
                    shell_quote(&layout.bin.to_string_lossy()),
                    shell_quote(&layout.www.to_string_lossy())
                );
                atomic_write(&script, text.as_bytes())?;
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
                let mut cmd = process::command("/usr/bin/open");
                cmd.arg("-a").arg("Terminal").arg(script);
                process::output(cmd)?;
            }
            #[cfg(windows)]
            {
                // Create a separate console; no shell interpolation of user-supplied text.
                let mut cmd = std::process::Command::new("cmd.exe");
                use std::os::windows::process::CommandExt;
                let mut paths = vec![layout.bin.clone()];
                if let Some(path) = env::var_os("PATH") {
                    paths.extend(env::split_paths(&path));
                }
                cmd.arg("/K")
                    .current_dir(&layout.www)
                    .env("PATH", env::join_paths(paths).map_err(|e| e.to_string())?)
                    .creation_flags(0x00000010)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "linux")]
            return Err("Run fairway shell to open a terminal with the bundled tools.".into());
            #[cfg(not(target_os = "linux"))]
            return Ok(());
        }
        _ => return Err("Unknown destination.".into()),
    };
    #[cfg(target_os = "macos")]
    {
        let mut cmd = process::command("/usr/bin/open");
        cmd.arg(path);
        process::output(cmd)?;
    }
    #[cfg(windows)]
    {
        // Explorer handles folders and URLs without cmd.exe metacharacter interpretation.
        let mut cmd = process::command("explorer.exe");
        cmd.arg(path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let mut cmd = process::command("xdg-open");
        cmd.arg(&path)
            .spawn()
            .map_err(|e| format!("Open {path} manually ({e})."))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_commands_do_not_depend_on_inherited_path() {
        let root = env::temp_dir().join(format!(
            "fairway-cli-paths-{}-{} Student May",
            std::process::id(),
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(&layout.bin).unwrap();
        assert!(launcher(&layout, "php").is_none());
        assert!(!installed(&layout));
        fs::write(layout.data.join("cli-installed"), "1").unwrap();
        let php = layout
            .bin
            .join(if cfg!(windows) { "php.cmd" } else { "php" });
        let composer = layout.bin.join(if cfg!(windows) {
            "composer.cmd"
        } else {
            "composer"
        });
        fs::write(&php, "test launcher").unwrap();
        assert!(!installed(&layout), "Composer must be installed too");
        fs::write(&composer, "test launcher").unwrap();
        assert!(installed(&layout));
        assert_eq!(launcher(&layout, "php"), Some(php.to_string_lossy().into()));
        assert_eq!(
            launcher(&layout, "composer"),
            Some(composer.to_string_lossy().into())
        );
        fs::remove_file(layout.data.join("cli-installed")).unwrap();
        assert!(
            !installed(&layout),
            "Launchers alone do not add PATH integration"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn profile_changes_are_idempotent_and_preserve_unrelated_edits() {
        let original = "export PATH=/custom:$PATH\n# my settings\n";
        let block = format!("{BEGIN}\nexport PATH='/fairway/bin':\"$PATH\"\n{END}");
        let once = profile_with_block(original, Some(&block)).unwrap();
        assert_eq!(profile_with_block(&once, Some(&block)).unwrap(), once);
        let edited = format!("{once}alias hello=world\n");
        assert_eq!(
            profile_with_block(&edited, None).unwrap(),
            format!("{original}alias hello=world\n")
        );
        assert!(profile_with_block(BEGIN, None).is_err());
    }
    #[test]
    fn shell_quotes_do_not_interpolate() {
        assert_eq!(shell_quote("a'b$(whoami)"), "'a'\\''b$(whoami)'");
    }
}
