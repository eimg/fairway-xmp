use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("fw-pg-discovery-{:032x}", rand::random::<u128>()));
        fs::create_dir(&root).unwrap();
        Self(fs::canonicalize(root).unwrap())
    }
    fn bin(&self, name: &str, tools: &[Tool]) -> PathBuf {
        let bin = self.0.join(name).join("bin");
        fs::create_dir_all(&bin).unwrap();
        for tool in tools {
            native(&executable(&bin, *tool));
        }
        bin
    }
    fn inventory(&self, entries: Vec<(PathBuf, Source)>) -> Inventory {
        discovery::inventory(&discovery::Roots {
            entries,
            truncated: false,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn executable(bin: &Path, tool: Tool) -> PathBuf {
    bin.join(if cfg!(windows) {
        format!("{}.exe", tool.name())
    } else {
        tool.name().into()
    })
}
fn native(path: &Path) {
    // Native header placeholders are never executed. Inspection uses an injected probe.
    let header: &[u8] = if cfg!(windows) {
        b"MZ00"
    } else if cfg!(target_os = "macos") {
        &[0xcf, 0xfa, 0xed, 0xfe]
    } else {
        b"\x7fELF"
    };
    fs::write(path, header).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
fn release(major: u16) -> Version {
    Version {
        major,
        minor: 1,
        patch: None,
    }
}

#[test]
fn parse_official_versions_without_exposing_vendor_output_or_accepting_prereleases() {
    for tool in Tool::ALL {
        assert_eq!(
            version::parse(
                tool,
                &format!("{} (PostgreSQL) 18.1 (Homebrew)\n", tool.name())
            ),
            Some(release(18))
        );
        assert_eq!(
            version::parse(tool, &format!("{} (PostgreSQL) 9.6.24", tool.name())),
            Some(Version {
                major: 9,
                minor: 6,
                patch: Some(24)
            })
        );
        for suffix in [
            "19rc1",
            "18beta2",
            "18",
            "18.1.2",
            "0.1",
            "9.6",
            "18.9999999",
            "18.1\nPGPASSWORD=fixture-secret",
        ] {
            assert!(
                version::parse(tool, &format!("{} (PostgreSQL) {suffix}", tool.name())).is_none()
            );
        }
        assert!(version::parse(tool, "not PostgreSQL: fixture-secret").is_none());
        assert!(version::parse(tool, &"x".repeat(4096)).is_none());
    }
    assert!(version::parse(Tool::Postgres, "psql (PostgreSQL) 18.1").is_none());
}

#[test]
fn inventory_is_read_only_distinguishes_client_only_and_does_not_create_managed_data() {
    let f = Fixture::new();
    let server = f.bin("managed runtime 日本", &Tool::ALL);
    let clients = f.bin("libpq", &[Tool::Psql, Tool::PgDump]);
    let external = f.0.join("existing-cluster");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("PG_VERSION"), "17\n").unwrap();
    fs::write(external.join("sentinel"), "coursework untouched").unwrap();
    let absent = f.0.join("missing-managed-data");
    let _service = Discovery::new(Some(absent.clone()));
    let found = f.inventory(vec![
        (server, Source::FairwayRuntime),
        (clients, Source::Homebrew),
    ]);
    assert_eq!(found.candidates.len(), 2);
    assert_eq!(found.candidates[0].server_tools, ToolsetCheck::NotChecked);
    assert!(found
        .candidates
        .iter()
        .flat_map(|c| &c.tools)
        .all(|t| t.version.is_none()));
    assert_eq!(found.candidates[1].tools[0].issue, Some(ToolIssue::Missing));
    assert_eq!(
        fs::read_to_string(external.join("sentinel")).unwrap(),
        "coursework untouched"
    );
    assert_eq!(
        fs::read_to_string(external.join("PG_VERSION")).unwrap(),
        "17\n"
    );
    assert_eq!(fs::read_dir(&external).unwrap().count(), 2);
    assert!(!absent.exists());
}

#[test]
fn inspection_checks_tool_roles_and_exact_versions_without_claiming_service_health() {
    let f = Fixture::new();
    let bin = f.bin("server", &Tool::ALL);
    let found = f.inventory(vec![(bin.clone(), Source::System)]);
    let id = found.candidates[0].id.clone();
    let mut calls = Vec::new();
    let checked = discovery::inspect(found, &id, |path, tool, seconds| {
        assert_eq!(path.parent().unwrap(), bin);
        assert!((1..=2).contains(&seconds));
        calls.push(tool);
        Ok(release(18))
    })
    .unwrap();
    assert_eq!(calls, Tool::ALL);
    assert_eq!(checked.server_tools, ToolsetCheck::Verified);
    assert_eq!(checked.client_tools, ToolsetCheck::Verified);
    let json = serde_json::to_value(checked).unwrap();
    for forbidden in [
        "password",
        "dataDirectory",
        "pid",
        "running",
        "ready",
        "managed",
    ] {
        assert!(json.get(forbidden).is_none());
    }
    let found = f.inventory(vec![(bin.clone(), Source::System)]);
    let mixed = discovery::inspect(found, &id, |_, tool, _| {
        Ok(release(if tool == Tool::Initdb { 17 } else { 18 }))
    })
    .unwrap();
    assert_eq!(mixed.server_tools, ToolsetCheck::MixedVersions);
    let found = f.inventory(vec![(bin, Source::System)]);
    let invalid = discovery::inspect(found, &id, |_, _, _| Err(ToolIssue::InvalidVersion)).unwrap();
    assert_eq!(invalid.server_tools, ToolsetCheck::Invalid);
    assert_eq!(invalid.tools[0].issue, Some(ToolIssue::InvalidVersion));
}

#[test]
fn missing_tools_are_not_inferred_from_client_binaries_or_directory_names() {
    let f = Fixture::new();
    let bin = f.bin("postgresql@18.1", &[Tool::Psql]);
    let found = f.inventory(vec![(bin, Source::Homebrew)]);
    let id = found.candidates[0].id.clone();
    let mut calls = 0;
    let checked = discovery::inspect(found, &id, |_, tool, _| {
        assert_eq!(tool, Tool::Psql);
        calls += 1;
        Ok(release(17))
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(checked.server_tools, ToolsetCheck::Missing);
    assert_eq!(checked.client_tools, ToolsetCheck::Missing);
    assert_eq!(checked.tools[3].version, Some(release(17)));
}

#[test]
fn consent_stale_candidates_and_unexpected_request_fields_are_rejected() {
    let service = Discovery::new(None);
    assert_eq!(
        service
            .inspect(InspectRequest {
                candidate_id: "invalid".into(),
                confirmed: false
            })
            .unwrap_err()
            .code,
        ErrorCode::ConfirmationRequired
    );
    let f = Fixture::new();
    for id in ["../../postgres", &"a".repeat(64)] {
        assert_eq!(
            discovery::inspect(f.inventory(vec![]), id, |_, _, _| panic!(
                "must not execute"
            ))
            .unwrap_err()
            .code,
            ErrorCode::StaleCandidate
        );
    }
    assert!(serde_json::from_value::<InspectRequest>(
        serde_json::json!({"candidateId":"a".repeat(64),"confirmed":true,"command":"initdb"})
    )
    .is_err());
    let _guard = service.gate.lock().unwrap();
    assert_eq!(
        service
            .inspect(InspectRequest {
                candidate_id: "a".repeat(64),
                confirmed: true
            })
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
}

#[test]
fn version_command_is_fixed_clean_and_cannot_use_pg_environment_or_shell() {
    let path = Path::new(if cfg!(windows) {
        r"C:\fixture\bin\postgres.exe"
    } else {
        "/fixture/bin/postgres"
    });
    let command = version::command(path);
    assert_eq!(command.get_program(), path);
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["--version"]);
    let keys: Vec<_> = command
        .get_envs()
        .map(|(key, _)| key.to_str().unwrap())
        .collect();
    assert!(keys.iter().all(|k| ["LC_ALL", "SystemRoot"].contains(k)));
    assert!(keys.contains(&"LC_ALL"));
}

#[cfg(unix)]
#[test]
fn native_probe_failures_do_not_expose_raw_child_output_and_cancel_reaps_owned_child() {
    // These are existing OS utilities, never generated scripts or PostgreSQL servers.
    let native_true = if Path::new("/usr/bin/true").exists() {
        "/usr/bin/true"
    } else {
        "/bin/true"
    };
    assert_eq!(
        version::probe(Path::new(native_true), Tool::Postgres, 1),
        Err(ToolIssue::InvalidVersion)
    );
    assert_eq!(
        version::probe(Path::new("/nonexistent/postgres"), Tool::Postgres, 1),
        Err(ToolIssue::ProbeFailed)
    );
    let sleep = if Path::new("/bin/sleep").exists() {
        "/bin/sleep"
    } else {
        "/usr/bin/sleep"
    };
    for cancel in [true, false] {
        let mut command = crate::process::command(sleep);
        command.env_clear().arg("30");
        let start = std::time::Instant::now();
        assert!(crate::command_output::output(command, 1, &|| cancel).is_err());
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
    }
}

#[cfg(unix)]
#[test]
fn canonical_dedup_symlink_escape_scripts_and_changed_tools_cannot_execute() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let bin = f.bin("server", &[Tool::Postgres, Tool::Initdb]);
    let alias = f.0.join("opt-alias");
    symlink(&bin, &alias).unwrap();
    let outside = f.bin("outside", &[Tool::Psql]);
    symlink(
        executable(&outside, Tool::Psql),
        executable(&bin, Tool::Psql),
    )
    .unwrap();
    fs::write(
        executable(&bin, Tool::PgCtl),
        "#!/bin/sh\necho must-not-run\n",
    )
    .unwrap();
    fs::set_permissions(
        executable(&bin, Tool::PgCtl),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let found = f.inventory(vec![
        (bin.clone(), Source::Homebrew),
        (alias, Source::Path),
        (PathBuf::from("."), Source::Path),
    ]);
    assert_eq!(found.candidates.len(), 1);
    assert_eq!(
        found.candidates[0].tools[2].issue,
        Some(ToolIssue::UnsafeFile)
    );
    assert_eq!(
        found.candidates[0].tools[3].issue,
        Some(ToolIssue::UnsafeFile)
    );
    let id = found.candidates[0].id.clone();
    fs::write(
        executable(&bin, Tool::Postgres),
        "#!/bin/sh\necho changed\n",
    )
    .unwrap();
    let checked = discovery::inspect(found, &id, |_, tool, _| {
        assert_eq!(tool, Tool::Initdb);
        Ok(release(18))
    })
    .unwrap();
    assert_eq!(checked.tools[0].issue, Some(ToolIssue::UnsafeFile));
    assert!(checked.tools[0].path.is_none());
}

#[test]
fn discovery_limits_candidates_and_never_searches_children_for_clusters() {
    let f = Fixture::new();
    let roots = (0..30)
        .map(|n| (f.bin(&format!("runtime-{n}"), &[Tool::Psql]), Source::Path))
        .collect();
    let inventory = f.inventory(roots);
    assert_eq!(inventory.candidates.len(), 24);
    assert!(inventory.truncated);
    let parent = f.0.join("runtime-0");
    assert!(f
        .inventory(vec![(parent, Source::Path)])
        .candidates
        .is_empty());
}
