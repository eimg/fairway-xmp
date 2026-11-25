# PostgreSQL Stage 1 verification

2026-10-30, macOS arm64. Branch `feature/postgresql-integration` was created from
freshly fetched/verified `origin/main` at `dbc03ff1634acc64985ff0714cd140c5e723c095`.
No existing PostgreSQL branch existed. `feature/node-project-templates` remains at
`3f81e1e`; its pending manual acceptance/merge is independent. AGENTS.md was copied
unchanged from that branch because the main baseline did not contain it.

## Implemented and verified

- Independent Rust discovery state, bounded standard/PGBIN/PATH tool inventory,
  canonical deduplication and native file filtering; no manager initialization.
- Explicit confirmed `--version` inspection with fresh candidate selection,
  strict DTOs, clean environment, bounded capture/deadlines and typed errors.
  No caller-supplied executable path/arguments or raw command output in diagnostics.
- Two typed Tauri commands and matching TypeScript API/types; no UI/polling/startup
  discovery is enabled. Creating the state performs no IO.
- Separate architecture/design for managed ownership, external profiles, secrets,
  health, schemas/rows, SQL, roles and backup/restore. Those operations are not
  implemented in this stage; see [the design](postgresql-integration.md).

| Check | Observed result |
| --- | --- |
| Focused PostgreSQL Rust tests | 9 passed |
| Core/terminal regression suite, locked | 163 passed; 7 opt-ins excluded; zero doc-tests |
| Default desktop/library regression suite, locked | 174 passed; 7 opt-ins excluded |
| TypeScript unit/contract tests | 24 passed |
| Production frontend build | TypeScript noEmit and Vite passed |
| Strict Clippy, all targets/all features, locked | Passed |
| CLI configuration, no default features | cargo check passed |
| Existing PostgreSQL client fixture Python units | 3 passed; subprocesses mocked, no cluster initialization |
| Read-only discovery example | Completed; no tool candidates in this host's searched locations |
| Formatting, diff whitespace and documentation links | Passed |

Counts overlap and must not be summed. Focused fixtures cover native header
placeholders (never executed), missing/client-only installations, version formats,
legacy releases, prerelease/malformed/vendor output, exact group-version conflicts,
relative paths, symlink dedup/escape, wrapper rejection, changed files, stale IDs,
consent, unknown fields, busy inspection, bounds and preserved cluster sentinels.
Injected version results test PostgreSQL parsing/selection. Known native OS utilities
test safe failure, cancellation and timeout cleanup; they are not PostgreSQL servers.

An initial `--no-default-features --lib` test build failed in unchanged
`shell_environment.rs`: a Mac test references `is_tool_variable`, which is compiled
only with `terminal`. This is present on the main baseline. No unrelated fix was
made; the documented `--no-default-features --features terminal` configuration and
default desktop tests pass. The normal seven ignored native Node integration
opt-ins were not run. Node source/execution behavior is unchanged.

## Limits and deferred checks

No PostgreSQL server was initialized, started, stopped, connected to or modified.
No Homebrew command/service or personal database was accessed for administration.
Discovery never reads clusters or credential files. No passwords were accepted,
stored or logged. No installed Fairway, global runtime/profile, main/upstream or
other feature branch was modified. No Computer Use/GUI automation was used.

The actual host inventory was empty: genuine PostgreSQL executable/version
interoperability, Windows/Linux native behavior, remote connections, vaults,
health/authentication, managed lifecycle, packaging and GUI IPC invocation remain
unverified. Script/wrapper installations are intentionally refused; custom paths
outside the bounded inventory and container/remote servers are not auto-detected.
Matching version output does not qualify any PostgreSQL major, prove service health
or confer process/cluster ownership. Native code and same-user replacement are not
sandboxed; future mutations need fresh receipts and ownership validation.

The existing `check_postgres_clients.py` server smoke, database/service smokes,
full browser suite, signed/release builds and installers were not run: Stage 1
prohibits PostgreSQL initialization and does not change the existing UI, database
SQL adapters, runtime packaging or service controls. PHP/MySQL/Node regression
evidence here is source preservation plus normal Rust/TypeScript suites, not a
new end-to-end service qualification.

Temporary evidence: `/tmp/fw-pg-stage1-{focused-final,core,desktop,unit-final,
frontend,clippy-final,cli,python,discovery}.log`. Logs/build outputs are disposable,
not committed artifacts. No Stage 2 work is authorized by this report.
