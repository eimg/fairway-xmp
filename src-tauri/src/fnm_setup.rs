//! Discover the user's fnm without mistaking Fairway's own CLI for another manager.
use crate::config::{Layout, Result};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct Context {
    #[cfg_attr(not(any(target_os = "macos", windows, test)), allow(dead_code))]
    pub directory: PathBuf,
    pub external: Option<PathBuf>,
    pub error: Option<String>,
}

fn value(environment: &[(OsString, OsString)], name: &str) -> Option<PathBuf> {
    environment
        .iter()
        .find(|(key, _)| {
            if cfg!(windows) {
                key.to_string_lossy().eq_ignore_ascii_case(name)
            } else {
                key == name
            }
        })
        .map(|(_, value)| PathBuf::from(value))
        .filter(|path| path.is_absolute())
}

pub(crate) fn inspect(layout: &Layout) -> Context {
    #[cfg(target_os = "macos")]
    let result = crate::shell_environment::load(layout);
    #[cfg(windows)]
    let result: Result<Vec<(OsString, OsString)>> =
        Ok(crate::shell_environment::windows_environment());
    #[cfg(not(any(target_os = "macos", windows)))]
    let result: Result<Vec<(OsString, OsString)>> = Ok(std::env::vars_os().collect());
    let (environment, error) = match result {
        Ok(values) => (values, None),
        Err(error) => (std::env::vars_os().collect(), Some(error)),
    };
    let mut context = from_environment(layout, &environment);
    context.error = error;
    context
}

fn from_environment(layout: &Layout, environment: &[(OsString, OsString)]) -> Context {
    let directory = value(environment, "FNM_DIR").unwrap_or_else(|| {
        #[cfg(windows)]
        let modern = value(environment, "APPDATA")
            .unwrap_or_else(|| layout.home.join("AppData/Roaming"))
            .join("fnm");
        #[cfg(not(windows))]
        let modern = value(environment, "XDG_DATA_HOME")
            .unwrap_or_else(|| layout.home.join(".local/share"))
            .join("fnm");
        if modern.exists() {
            return modern;
        }
        let legacy = [
            layout.home.join(".fnm"),
            #[cfg(target_os = "macos")]
            layout.home.join("Library/Application Support/fnm"),
        ];
        legacy
            .into_iter()
            .find(|path| path.exists())
            .unwrap_or(modern)
    });
    let mut paths: Vec<PathBuf> = environment
        .iter()
        .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("PATH"))
        .map(|(_, path)| {
            std::env::split_paths(path)
                .filter(|p| p.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    #[cfg(unix)]
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        layout.home.join(".cargo/bin"),
        layout.home.join(".local/bin"),
        layout.home.join(".fnm"),
        directory.clone(),
    ]);
    #[cfg(windows)]
    {
        paths.extend([
            layout.home.join("scoop/shims"),
            layout.home.join(".cargo/bin"),
        ]);
        if let Some(local) = value(environment, "LOCALAPPDATA") {
            paths.push(local.join("Microsoft/WinGet/Links"));
        }
    }
    let name = if cfg!(windows) { "fnm.exe" } else { "fnm" };
    let external = paths.into_iter().map(|path| path.join(name)).find(|path| {
        if owned_path(layout, path) {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            path.is_file()
        }
    });
    Context {
        directory,
        external,
        error: None,
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn owned_path(layout: &Layout, path: &Path) -> bool {
    let name = if cfg!(windows) { "fnm.exe" } else { "fnm" };
    // Check the launcher location before resolving its symlink: an old app may
    // have left a launcher pointing at an earlier runtime.
    [
        layout.bin.join(name),
        layout.data.join("bin").join(name),
        layout.data.join("terminal-bin").join(name),
        layout.runtime.join("fnm/bin").join(name),
    ]
    .iter()
    .any(|own| path == own || same_path(path, own))
}

/// Remove only our symlink/copy, never an executable supplied by the user.
pub(crate) fn remove_launcher(layout: &Layout) -> Result<()> {
    let name = if cfg!(windows) { "fnm.exe" } else { "fnm" };
    let path = layout.bin.join(name);
    #[cfg(unix)]
    let owned = fs::read_link(&path).is_ok_and(|target| {
        same_path(&target, &layout.runtime.join("fnm/bin").join(name))
            || target.ends_with("runtime/fnm/bin/fnm")
    });
    #[cfg(windows)]
    let owned = {
        let recorded = fs::read(layout.bin.join(".fairway-fnm.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<(u64, u64, u32)>(&bytes).ok());
        (recorded.is_some() && recorded == launcher_identity(&path))
            || fs::read(&path)
                .ok()
                .zip(fs::read(layout.runtime.join("fnm/bin").join(name)).ok())
                .is_some_and(|(left, right)| left == right)
    };
    if owned {
        fs::remove_file(path).map_err(|error| error.to_string())?;
        #[cfg(windows)]
        match fs::remove_file(layout.bin.join(".fairway-fnm.json")) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn launcher_identity(path: &Path) -> Option<(u64, u64, u32)> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some((metadata.len(), modified.as_secs(), modified.subsec_nanos()))
}

/// Track our own copy across fnm upgrades, without deleting a later user replacement.
#[cfg(windows)]
pub(crate) fn record_launcher(layout: &Layout) -> Result<()> {
    let identity = launcher_identity(&layout.bin.join("fnm.exe"))
        .ok_or("Could not record the fnm CLI copy.")?;
    crate::config::atomic_write(
        &layout.bin.join(".fairway-fnm.json"),
        &serde_json::to_vec(&identity).map_err(|error| error.to_string())?,
    )
}

#[cfg(any(target_os = "macos", windows))]
pub(crate) fn store_owned(layout: &Layout, directory: &Path) -> bool {
    fs::read(layout.data.join("fnm-store.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PathBuf>(&bytes).ok())
        .is_some_and(|record| same_path(&record, directory))
}

#[cfg(any(target_os = "macos", windows))]
pub(crate) fn read_only_reason(layout: &Layout, context: &Context) -> Option<String> {
    if let Some(error) = &context.error {
        return Some(format!("Your fnm setup could not be checked. Management is unavailable until detection succeeds. {error}"));
    }
    if context.external.is_some() {
        return Some("An existing fnm installation was found. Manage Node versions with fnm in your terminal.".into());
    }
    if !store_owned(layout, &context.directory) {
        match fs::read_dir(&context.directory) {
            Ok(entries) => {
                if entries.into_iter().next().is_some() {
                    return Some("An existing fnm store was found. Its Node versions are managed outside Fairway.".into());
                }
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Some(format!("The fnm store could not be checked: {error}"))
            }
            _ => (),
        }
    }
    None
}

#[cfg(any(target_os = "macos", windows))]
pub(crate) fn require_management(layout: &Layout, context: &Context) -> Result<()> {
    match read_only_reason(layout, context) {
        Some(reason) => Err(reason),
        None => Ok(()),
    }
}

#[cfg(any(target_os = "macos", windows))]
pub(crate) fn claim_store(layout: &Layout, context: &Context) -> Result<()> {
    require_management(layout, context)?;
    fs::create_dir_all(&context.directory).map_err(|error| error.to_string())?;
    fs::create_dir_all(&layout.data).map_err(|error| error.to_string())?;
    let directory = fs::canonicalize(&context.directory).map_err(|error| error.to_string())?;
    crate::config::atomic_write(
        &layout.data.join("fnm-store.json"),
        &serde_json::to_vec(&directory).map_err(|error| error.to_string())?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, Layout) {
        let root =
            std::env::temp_dir().join(format!("fairway-fnm-policy-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(&layout.home).unwrap();
        (root, layout)
    }
    #[test]
    fn custom_store_from_shell_exports_and_external_after_own_launcher_are_detected() {
        let (root, layout) = fixture();
        let external = root.join("existing-fnm");
        fs::create_dir_all(&external).unwrap();
        fs::create_dir_all(&layout.bin).unwrap();
        let name = if cfg!(windows) { "fnm.exe" } else { "fnm" };
        for path in [layout.bin.join(name), external.join(name)] {
            fs::write(&path, b"existing native executable").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        let store = root.join("custom node storage");
        let env = vec![
            (
                "PATH".into(),
                std::env::join_paths([&layout.bin, &external]).unwrap(),
            ),
            ("FNM_DIR".into(), store.clone().into_os_string()),
        ];
        let context = from_environment(&layout, &env);
        assert_eq!(context.external, Some(external.join(name)));
        assert_eq!(context.directory, store);
        assert!(!store.exists(), "Detection must not initialize a store");
        let terminal = layout.data.join("terminal-bin").join(name);
        assert!(owned_path(&layout, &terminal));
        let mut tools = layout.clone();
        tools.bin = layout.data.join("terminal-bin");
        assert!(owned_path(&tools, &layout.bin.join(name)));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn login_shell_custom_store_and_external_fnm_are_resolved_without_claiming_them() {
        use std::os::unix::fs::PermissionsExt;
        let (root, layout) = fixture();
        let tools = root.join("user tools");
        fs::create_dir_all(&tools).unwrap();
        fs::write(tools.join("fnm"), b"#!/bin/sh\nexit 99\n").unwrap();
        fs::set_permissions(tools.join("fnm"), fs::Permissions::from_mode(0o755)).unwrap();
        let store = root.join("custom store");
        fs::write(
            layout.home.join(".zshrc"),
            format!(
                "export FNM_DIR={}\nexport PATH={}:\"$PATH\"\n",
                crate::integration::shell_quote(&store.to_string_lossy()),
                crate::integration::shell_quote(&tools.to_string_lossy())
            ),
        )
        .unwrap();
        let context = inspect(&layout);
        assert!(context.error.is_none(), "{:?}", context.error);
        assert_eq!(context.directory, store);
        assert_eq!(context.external, Some(tools.join("fnm")));
        assert!(!store.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn owned_windows_copy_is_removed_after_runtime_upgrade_but_user_replacement_is_preserved() {
        let (root, layout) = fixture();
        fs::create_dir_all(&layout.bin).unwrap();
        fs::create_dir_all(layout.runtime.join("fnm/bin")).unwrap();
        let cli = layout.bin.join("fnm.exe");
        fs::write(&cli, b"previous bundle").unwrap();
        record_launcher(&layout).unwrap();
        fs::write(layout.runtime.join("fnm/bin/fnm.exe"), b"new bundle").unwrap();
        remove_launcher(&layout).unwrap();
        assert!(!cli.exists());
        fs::write(&cli, b"previous bundle").unwrap();
        record_launcher(&layout).unwrap();
        fs::write(&cli, b"user supplied replacement executable").unwrap();
        remove_launcher(&layout).unwrap();
        assert!(cli.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_to_bundled_fnm_is_not_external_and_removal_preserves_external_binary() {
        let (root, layout) = fixture();
        let runtime = layout.runtime.join("fnm/bin/fnm");
        fs::create_dir_all(runtime.parent().unwrap()).unwrap();
        fs::create_dir_all(&layout.bin).unwrap();
        fs::write(&runtime, b"bundle").unwrap();
        let link = root.join("fnm");
        std::os::unix::fs::symlink(&runtime, &link).unwrap();
        assert!(owned_path(&layout, &link));
        std::os::unix::fs::symlink(&runtime, layout.bin.join("fnm")).unwrap();
        remove_launcher(&layout).unwrap();
        assert!(!layout.bin.join("fnm").is_symlink());
        fs::write(layout.bin.join("fnm"), b"user replacement").unwrap();
        remove_launcher(&layout).unwrap();
        assert!(layout.bin.join("fnm").is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn only_empty_stores_can_be_claimed_and_external_managers_always_block_mutations() {
        let (root, layout) = fixture();
        let mut context = Context {
            directory: root.join("store"),
            external: None,
            error: None,
        };
        assert!(read_only_reason(&layout, &context).is_none());
        fs::create_dir_all(context.directory.join("node-versions/v24.0.0")).unwrap();
        assert!(claim_store(&layout, &context).is_err());
        assert!(!layout.data.join("fnm-store.json").exists());
        fs::remove_dir_all(&context.directory).unwrap();
        claim_store(&layout, &context).unwrap();
        fs::create_dir_all(context.directory.join("node-versions/v24.0.0")).unwrap();
        assert!(read_only_reason(&layout, &context).is_none());
        context.external = Some(root.join("other-fnm"));
        assert!(require_management(&layout, &context).is_err());
        context.external = None;
        context.error = Some("startup failed".into());
        assert!(require_management(&layout, &context).is_err());
        context.error = None;
        context.directory = root.join("another-store");
        fs::create_dir_all(context.directory.join("aliases")).unwrap();
        assert!(
            require_management(&layout, &context).is_err(),
            "A claim must not follow a changed FNM_DIR"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
