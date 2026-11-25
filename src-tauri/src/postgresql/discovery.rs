use super::*;
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

const MAX_ROOTS: usize = 64;
const MAX_CANDIDATES: usize = 24;
pub(super) struct Roots {
    pub entries: Vec<(PathBuf, Source)>,
    pub truncated: bool,
}

fn path_ok(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        {
            return false;
        }
    }
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|s| s.len() <= 4096 && !s.chars().any(char::is_control))
}
fn add(roots: &mut Roots, path: PathBuf, source: Source) {
    if path_ok(&path) && !roots.entries.iter().any(|(p, _)| *p == path) {
        if roots.entries.len() < MAX_ROOTS {
            roots.entries.push((path, source));
        } else {
            roots.truncated = true;
        }
    }
}
fn children(roots: &mut Roots, parent: &Path, source: Source, select: impl Fn(&str) -> bool) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let mut names = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index == 128 {
            roots.truncated = true;
            break;
        }
        if let Ok(entry) = entry {
            if entry.file_name().to_str().is_some_and(&select) {
                names.push(entry.path());
            }
        }
    }
    names.sort();
    for path in names {
        add(roots, path.join("bin"), source);
    }
}
fn numeric(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && name.bytes().any(|b| b.is_ascii_digit())
}
pub(super) fn roots(runtime: Option<&Path>) -> Roots {
    let mut roots = Roots {
        entries: Vec::new(),
        truncated: false,
    };
    if let Some(runtime) = runtime {
        add(
            &mut roots,
            runtime.join("postgresql/bin"),
            Source::FairwayRuntime,
        );
    }
    if let Some(bin) = std::env::var_os("PGBIN") {
        add(&mut roots, bin.into(), Source::Environment);
    }
    #[cfg(target_os = "macos")]
    {
        for prefix in ["/opt/homebrew/opt", "/usr/local/opt"] {
            children(&mut roots, Path::new(prefix), Source::Homebrew, |name| {
                name == "postgresql"
                    || name == "libpq"
                    || name.strip_prefix("postgresql@").is_some_and(numeric)
            });
        }
        children(
            &mut roots,
            Path::new("/Applications/Postgres.app/Contents/Versions"),
            Source::PostgresApp,
            |s| s == "latest" || numeric(s),
        );
    }
    #[cfg(target_os = "linux")]
    {
        children(
            &mut roots,
            Path::new("/usr/lib/postgresql"),
            Source::System,
            numeric,
        );
        // RHEL-style /usr/pgsql-MAJOR/bin directories, without inspecting clusters.
        children(&mut roots, Path::new("/usr"), Source::System, |s| {
            s.strip_prefix("pgsql-").is_some_and(numeric)
        });
    }
    #[cfg(windows)]
    for key in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(key) {
            let root = PathBuf::from(root);
            if path_ok(&root) {
                children(
                    &mut roots,
                    &root.join("PostgreSQL"),
                    Source::System,
                    numeric,
                );
            }
        }
    }
    #[cfg(unix)]
    for root in ["/usr/local/bin", "/usr/bin"] {
        add(&mut roots, root.into(), Source::System);
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for (index, path) in std::env::split_paths(&paths).enumerate() {
            if index == 64 {
                roots.truncated = true;
                break;
            }
            add(&mut roots, path, Source::Path);
        }
    }
    roots
}

fn tool_path(bin: &Path, tool: Tool) -> PathBuf {
    bin.join(if cfg!(windows) {
        format!("{}.exe", tool.name())
    } else {
        tool.name().into()
    })
}
/// Refuse wrappers/scripts, devices, directories and links escaping the canonical toolset.
pub(super) fn native_file(path: &Path, bin: &Path) -> Option<PathBuf> {
    let path = fs::canonicalize(path).ok()?;
    if !path_ok(&path) || path.parent()? != bin {
        return None;
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    } // FILE_FLAG_OPEN_REPARSE_POINT
    let mut file = options.open(&path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    let mut magic = [0; 4];
    file.read_exact(&mut magic).ok()?;
    let native = if cfg!(windows) {
        magic.starts_with(b"MZ")
    } else if cfg!(target_os = "macos") {
        matches!(
            magic,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
        )
    } else {
        magic == *b"\x7fELF"
    };
    native.then_some(path)
}
fn candidate(bin: PathBuf, source: Source) -> Option<Candidate> {
    let tools: Vec<_> = Tool::ALL
        .into_iter()
        .map(|name| {
            let path = tool_path(&bin, name);
            let exists = fs::symlink_metadata(&path).is_ok();
            let safe = native_file(&path, &bin).and_then(|p| p.to_str().map(str::to_owned));
            let issue = if !exists {
                Some(ToolIssue::Missing)
            } else if safe.is_none() {
                Some(ToolIssue::UnsafeFile)
            } else {
                None
            };
            ToolView {
                name,
                path: safe,
                version: None,
                issue,
            }
        })
        .collect();
    if tools.iter().all(|t| t.issue == Some(ToolIssue::Missing)) {
        return None;
    }
    let bin_directory = bin.to_str()?.to_owned();
    let id = ring::digest::digest(&ring::digest::SHA256, bin_directory.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Some(Candidate {
        id,
        source,
        bin_directory,
        tools,
        server_tools: ToolsetCheck::NotChecked,
        client_tools: ToolsetCheck::NotChecked,
    })
}
pub(super) fn inventory(roots: &Roots) -> Inventory {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    let mut truncated = roots.truncated;
    for (path, source) in &roots.entries {
        if !path_ok(path) {
            continue;
        }
        let Ok(bin) = fs::canonicalize(path) else {
            continue;
        };
        if !path_ok(&bin) || !bin.is_dir() || !seen.insert(bin.clone()) {
            continue;
        }
        if let Some(found) = candidate(bin, *source) {
            if candidates.len() == MAX_CANDIDATES {
                truncated = true;
                break;
            }
            candidates.push(found);
        }
    }
    Inventory { candidates, truncated, notes: vec![
        "Tool discovery does not find running servers, connect, inspect clusters or confer managed ownership.".into(),
        "Only bounded standard locations, PGBIN and absolute PATH entries are searched; custom/container/remote servers require later connection profiles.".into(),
        "Version inspection is explicit. Matching tool versions do not qualify a runtime for managed use.".into(),
    ] }
}
fn check(tools: &[ToolView]) -> ToolsetCheck {
    if tools.iter().any(|t| t.issue == Some(ToolIssue::Missing)) {
        return ToolsetCheck::Missing;
    }
    if tools
        .iter()
        .any(|t| t.issue.is_some() || t.version.is_none())
    {
        return ToolsetCheck::Invalid;
    }
    if tools.iter().any(|t| t.version != tools[0].version) {
        return ToolsetCheck::MixedVersions;
    }
    ToolsetCheck::Verified
}
pub(super) fn inspect(
    mut inventory: Inventory,
    id: &str,
    mut probe: impl FnMut(&Path, Tool, u64) -> std::result::Result<Version, ToolIssue>,
) -> Result<Candidate> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::new(
            ErrorCode::StaleCandidate,
            "Refresh PostgreSQL discovery and select a current candidate.",
        ));
    }
    let mut candidate = inventory
        .candidates
        .drain(..)
        .find(|c| c.id == id)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::StaleCandidate,
                "This PostgreSQL candidate is no longer available. Refresh discovery.",
            )
        })?;
    let deadline = Instant::now() + Duration::from_secs(8);
    for tool in &mut candidate.tools {
        let Some(path) = &tool.path else {
            continue;
        };
        if native_file(Path::new(path), Path::new(&candidate.bin_directory)).as_deref()
            != Some(Path::new(path))
        {
            tool.path = None;
            tool.issue = Some(ToolIssue::UnsafeFile);
            continue;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            tool.issue = Some(ToolIssue::DeadlineExceeded);
            continue;
        }
        match probe(Path::new(path), tool.name, remaining.as_secs().clamp(1, 2)) {
            Ok(version) => tool.version = Some(version),
            Err(issue) => tool.issue = Some(issue),
        }
    }
    candidate.server_tools = check(&candidate.tools[..3]);
    candidate.client_tools = check(&candidate.tools[3..]);
    Ok(candidate)
}
