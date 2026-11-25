use super::*;
use std::{path::Path, process::Command};

pub(super) fn parse(tool: Tool, output: &str) -> Option<Version> {
    if output.len() > 1024 || output.trim().contains(['\n', '\r']) {
        return None;
    }
    let prefix = format!("{} (PostgreSQL) ", tool.name());
    let rest = output.trim().strip_prefix(&prefix)?;
    let token = rest.split_ascii_whitespace().next()?;
    let parts: Vec<_> = token.split('.').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = if parts.len() == 3 {
        Some(parts[2].parse().ok()?)
    } else {
        None
    };
    if major == 0 || (major >= 10 && patch.is_some()) || (major < 10 && patch.is_none()) {
        return None;
    }
    Some(Version {
        major,
        minor,
        patch,
    })
}

pub(super) fn command(path: &Path) -> Command {
    let mut command = crate::process::command(path);
    command.env_clear().arg("--version");
    // No PGDATA/PGHOST/PGPASSWORD/service file, shell startup, loader injection,
    // personal configuration, credentials or renderer-supplied arguments.
    if let Some(bin) = path.parent() {
        command.current_dir(bin);
    }
    command.env("LC_ALL", "C");
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    command
}

pub(super) fn probe(
    path: &Path,
    tool: Tool,
    seconds: u64,
) -> std::result::Result<Version, ToolIssue> {
    #[cfg(any(unix, windows))]
    {
        parse(
            tool,
            &crate::command_output::output(command(path), seconds, &|| false)
                .map_err(|_| ToolIssue::ProbeFailed)?,
        )
        .ok_or(ToolIssue::InvalidVersion)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, tool, seconds);
        Err(ToolIssue::ProbeFailed)
    }
}
