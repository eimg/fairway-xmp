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

/// Bundle updates do not run installer hooks on macOS. Rewrite app-owned
/// commands without editing profiles or re-enabling tools the user disabled.
pub fn refresh(layout: &Layout) -> Result<()> {
    if layout.data.join("cli-installed").exists() {
        #[cfg(unix)]
        write_unix_launchers(layout)?;
        #[cfg(windows)]
        write_windows_launchers(layout)?;
    }
    Ok(())
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
    write_fnm_launcher(layout)?;
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

/// Expose the upstream binary directly, with no argument translation or Node activation.
#[cfg(unix)]
pub fn write_fnm_launcher(layout: &Layout) -> Result<()> {
    write_fnm_launcher_in(layout, &crate::fnm_setup::inspect(layout))
}

#[cfg(unix)]
fn write_fnm_launcher_in(layout: &Layout, context: &crate::fnm_setup::Context) -> Result<()> {
    if context.external.is_some() {
        return crate::fnm_setup::remove_launcher(layout);
    }
    if context.error.is_some() {
        return Ok(());
    }
    let executable = layout.runtime.join("fnm/bin/fnm");
    if !executable.is_file() {
        return Ok(());
    }
    fs::create_dir_all(&layout.bin).map_err(|e| e.to_string())?;
    let temporary = layout.bin.join(format!(".fnm-{}", rand::random::<u64>()));
    std::os::unix::fs::symlink(executable, &temporary).map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(&temporary, layout.bin.join("fnm")) {
        let _ = fs::remove_file(temporary);
        return Err(error.to_string());
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
    crate::node_tools::write_launcher(layout)?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    if executable
        .file_name()
        .is_some_and(|name| name == "fairway.exe")
    {
        if executable.to_string_lossy().contains('%') {
            return Err("The application path contains a percent sign, which Windows command launchers cannot support.".into());
        }
        let console = executable
            .parent()
            .ok_or("Cannot locate Fairway.")?
            .join("cli/fairway-cli.exe");
        let launcher = if console.is_file() {
            &console
        } else {
            &executable
        };
        atomic_write(
            &layout.bin.join("fairway.cmd"),
            format!(
                "@echo off\r\nchcp 65001 >nul\r\n\"{}\" %*\r\nexit /b %errorlevel%\r\n",
                launcher.display()
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
            #[cfg(target_os = "macos")]
            let shell = if file
                .file_name()
                .is_some_and(|name| name == ".zshrc" || name == ".zprofile")
            {
                "zsh"
            } else {
                "bash"
            };
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
            #[cfg(target_os = "macos")]
            let profile_block = format!(
                "{}{}{}",
                block.strip_suffix(END).unwrap_or(&block),
                crate::node_tools::shell_init(layout, shell),
                END
            );
            #[cfg(not(target_os = "macos"))]
            let profile_block = &block;
            let updated = profile_with_block(&previous, Some(profile_block.as_str()))?;
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
        crate::node_tools::install_shells(layout)?;
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
    {
        crate::node_tools::remove_shells(layout)?;
        windows_path(&layout.bin, false)?;
    }
    for name in [
        "php",
        "composer",
        "php.cmd",
        "composer.cmd",
        "fnm",
        "fnm.exe",
        ".fairway-fnm.json",
    ] {
        let file = layout.bin.join(name);
        if file.exists() || file.is_symlink() {
            fs::remove_file(file).map_err(|e| e.to_string())?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let launcher = layout.bin.join("fairway");
        if launcher.exists() || launcher.is_symlink() {
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

#[cfg(feature = "desktop")]
pub fn open_guide(section: &str) -> Result<()> {
    let url = match section {
        "getting-started" => "https://eimg.github.io/fairway-xmp/#getting-started",
        "cli" => "https://eimg.github.io/fairway-xmp/#cli",
        _ => return Err("Unknown guide.".into()),
    };
    open_path(url)
}

pub fn open(layout: &Layout, target: &str, ports: &crate::config::Ports) -> Result<()> {
    let website = ports.website;
    let path = match target {
        "website" => format!("http://127.0.0.1:{website}/"),
        "phpmyadmin" => format!("http://127.0.0.1:{}/", ports.phpmyadmin),
        "mail" => format!("http://127.0.0.1:{}/", ports.mail),
        "releases" => "https://github.com/eimg/fairway-xmp/releases".into(),
        "source" => "https://github.com/eimg/fairway-xmp".into(),
        "issues" => "https://github.com/eimg/fairway-xmp/issues".into(),
        name if name.starts_with("project:") => layout
            .registered_projects()?
            .into_iter()
            .find(|project| project.domain == name[8..] && project.root.is_dir())
            .map(|project| project_url(&project, website))
            .ok_or("This site's web root folder is missing. Edit the site to choose it again.")?,
        name if name.starts_with("site-folder:") => layout
            .registered_projects()?
            .into_iter()
            .find(|site| site.domain == name[12..] && site.root.is_dir())
            .map(|site| site.root.to_string_lossy().into_owned())
            .ok_or("This site's web root folder is missing. Edit the site to choose it again.")?,
        "coursework" => layout.www.to_string_lossy().into(),
        "logs" => layout.data.join("logs").to_string_lossy().into(),
        "data" => layout.data.to_string_lossy().into(),
        "backups" => layout.data.join("backups").to_string_lossy().into(),
        "terminal" => {
            #[cfg(feature = "terminal")]
            return crate::terminal::open_external(&layout.www);
            #[cfg(not(feature = "terminal"))]
            return Err("Run fairway shell to open a terminal with the bundled tools.".into());
        }
        _ => return Err("Unknown destination.".into()),
    };
    open_path(&path)
}

#[cfg(feature = "desktop")]
fn checked_http_url(value: &str) -> Result<tauri::Url> {
    let invalid = "Only HTTP and HTTPS email links can be opened.";
    if value.len() > 16 * 1024 || value.chars().any(char::is_control) {
        return Err(invalid.into());
    }
    let url = tauri::Url::parse(value).map_err(|_| invalid.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid.into());
    }
    Ok(url)
}

#[cfg(feature = "desktop")]
pub fn open_http_url(value: &str) -> Result<()> {
    open_path(checked_http_url(value)?.as_str())
}

fn open_path(path: &str) -> Result<()> {
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
        cmd.arg(path)
            .spawn()
            .map_err(|e| format!("Open {path} manually ({e})."))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "desktop")]
    #[test]
    fn mail_links_accept_web_urls_and_reject_other_destinations() {
        for value in [
            "http://project.test:8800/reset?token=demo#next",
            "https://example.test/hello%20world",
        ] {
            assert_eq!(checked_http_url(value).unwrap().as_str(), value);
        }
        assert_eq!(
            checked_http_url("HTTPS://EXAMPLE.TEST").unwrap().as_str(),
            "https://example.test/"
        );
        for value in [
            "javascript:alert(1)",
            "file:///tmp/test",
            "data:text/html,hello",
            "mailto:hello@example.test",
            "/relative",
            "//example.test",
            "https://",
            "https://user:password@example.test",
            "https://example.test/\n--args",
        ] {
            assert!(checked_http_url(value).is_err(), "{value}");
        }
    }
    #[test]
    fn refresh_repairs_enabled_launchers_without_enabling_tools_or_editing_profiles() {
        let root = env::temp_dir().join(format!("fairway-refresh-{}", rand::random::<u64>()));
        let layout = Layout::at(
            root.join("home"),
            root.join("data"),
            root.join("new app/runtime"),
        );
        fs::create_dir_all(&layout.bin).unwrap();
        fs::create_dir_all(&layout.home).unwrap();
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::write(layout.runtime.join("manifest.json"), r#"{"php":"8.5.11"}"#).unwrap();
        let profile = layout.home.join(".zshrc");
        fs::write(&profile, "# preserve my profile\n").unwrap();
        refresh(&layout).unwrap();
        assert!(launcher(&layout, "php").is_none());
        assert!(!layout.data.join("cli-installed").exists());
        fs::write(layout.data.join("cli-installed"), "1").unwrap();
        refresh(&layout).unwrap();
        let php = launcher(&layout, "php").unwrap();
        fs::write(&php, "old installation").unwrap();
        refresh(&layout).unwrap();
        assert!(fs::read_to_string(&php).unwrap().contains("new app"));
        assert!(launcher(&layout, "composer").is_some());
        assert_eq!(
            fs::read_to_string(profile).unwrap(),
            "# preserve my profile\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

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
    #[cfg(unix)]
    #[test]
    fn native_fnm_launcher_tracks_runtime_updates_and_removal_preserves_store() {
        let root = env::temp_dir().join(format!(
            "fairway-linux-fnm-{} student's home",
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(layout.runtime.join("fnm/bin")).unwrap();
        fs::create_dir_all(layout.home.join(".local/share/fnm")).unwrap();
        let binary = layout.runtime.join("fnm/bin/fnm");
        fs::write(&binary, "native binary unchanged").unwrap();
        let context = crate::fnm_setup::Context {
            directory: layout.home.join(".local/share/fnm"),
            external: None,
            error: None,
        };
        write_fnm_launcher_in(&layout, &context).unwrap();
        write_fnm_launcher_in(&layout, &context).unwrap();
        assert_eq!(fs::read_link(layout.bin.join("fnm")).unwrap(), binary);
        assert_eq!(
            fs::read(layout.bin.join("fnm")).unwrap(),
            b"native binary unchanged"
        );
        fs::remove_file(&binary).unwrap();
        assert!(layout.bin.join("fnm").is_symlink());
        assert!(!layout.bin.join("fnm").exists());
        remove(&layout).unwrap();
        assert!(!layout.bin.join("fnm").exists());
        assert!(!layout.bin.join("fnm").is_symlink());
        assert!(layout.home.join(".local/share/fnm").is_dir());
        fs::remove_dir_all(root).unwrap();
    }
}
