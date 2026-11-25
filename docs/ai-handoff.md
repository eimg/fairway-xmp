# AI handoff

## Owner-authorized main integration (2026-11-25)

- Owner explicitly requested committing the prepared code, merging it into the
  personal fork's `main`, and pushing to `origin`. This supersedes the earlier
  instruction to leave `main` unchanged for this integration only.
- Scope is the current PostgreSQL workspace and release preparation on
  `feature/postgresql-integration`. The separate Node templates branch remains
  separate. Check Git history and remote status for the resulting commit/merge.
- Publication is still pending owner credentials, a reviewed signed PostgreSQL pin
  and successful candidate CI. Merging/pushing source does not publish a release.

## Release preparation (2026-11-25)

- User requested finishing release prerequisites. Candidate is `0.2.0-rc.1`;
  updater/publication/help links now target `RizzRioo06/fairway-xmp`.
- Owner confirmed no release credentials are configured. The inherited public
  updater key and development PostgreSQL pin are deliberately retained; preflight
  blocks signed release until reviewed owner inputs replace them.
- Added separate PostgreSQL build/sign workflow, signed artifact validation/staging
  before compilation, preserved pin during app signing, configurable Apple
  identities, source redistribution and final signed-app worker CI fixtures.
- Bundle source reads allow root-owned non-writable installer resources; managed
  storage rules remain private. Unsupported platforms show an unavailable workspace.
- [Fork guide](fork-release.md) lists features and remaining owner/CI work. Notes
  are an unpublished draft. Initial preparation did not commit, merge or push;
  source integration is now authorized above. Release signing, Keychain changes
  and publication remain unperformed.
- Local checks: frontend build/32 TS tests; 177 Rust regressions; all 25 PostgreSQL
  tests including native disposable fixtures against a copied public-mode payload;
  24 simulated Chromium/WebKit checks; Python regression/release fixtures; strict
  all-target Clippy (debug resource-copy override), formatting and doc links passed.
  The fork guide records skipped checks, initial sandbox/resource-copy failures
  and the still-unverified signed/platform behavior.

## Existing PostgreSQL workspace

- Personal fork `RizzRioo06/fairway-xmp`; branch `feature/postgresql-integration`.
  Base main `dbc03ff`; Stage 2 `3af0075`. Use `git log -1` for latest commit.
- **PostgreSQL workspace is included in the prepared source integration.**
  Native external PostgreSQL 17.x compatibility and a rebuilt private 18.6 client
  payload are prepared. Real Summit 17.12 access to `vector_one.app_user` succeeded
  (8 columns/2 rows, values withheld). Simulated UI authentication is not evidence
  of real access. [Current acceptance/evidence](postgresql-native-acceptance.md)
  contains isolated native GUI commands; the user performs Mac acceptance.
- Stage 3 explorer/SQL backend:
  Native pg_catalog inspection, schema-aware paginated/sorted/filtered rows, exact
  typed text/NULL results, single-statement SQL with read-only default and explicit
  write consent, bounded private libpq workers and connection-bound cancellation.
  Activity tests observe the server query active before cancellation/owner loss and
  absent afterward. Resolver contention preserves pre-dispatch `busy`.
- Backend: `src-tauri/src/postgresql/explorer/` and `explorer_contract.rs`;
  IPC: `desktop/postgresql_explorer_commands.rs`; TS: `src/postgresql/explorer*.ts`.
  Explicit memory-only managed/external sessions; opening a descriptor reports auth
  not checked. External sessions never own servers. No history/password persistence.
- [Architecture/contracts](postgresql-integration.md),
  [Stage 3 evidence/limits](postgresql-stage3-verification.md),
  [runtime/pin/release blockers](postgresql-runtime.md).
- Qualified backend fixtures: macOS arm64/client-server 18.6 and real external
  Summit server 17.12 using client 18.6, loopback only. Remote
  verify-full fails closed: pinned client has no TLS. Other platforms/versions remain
  unverified. Disposable-cluster tests use memory vaults, never the login Keychain.
- **Stage 2 signed packaging and real signed-app Keychain acceptance remain blocked.**
  No real managed-cluster release acceptance is claimed. Preserve integrity/leases,
  journal/cluster ownership, Keychain-only managed credentials and smart shutdown.
  Never adopt/delete/upgrade clusters or control external services. Interrupted
  stages/orphan servers still require manual review; generic data removal refuses PG.

The current request authorizes finishing the existing workspace/native connection;
preserve explicit SQL consent, cancellation, truncation and uncertain-commit
handling. No role/backup UI or unrelated work is authorized. Upstream remains unchanged;
`feature/node-project-templates` stays at `3f81e1e`, awaiting manual acceptance/merge.
