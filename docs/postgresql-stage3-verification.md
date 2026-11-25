# PostgreSQL Stage 3 verification

2026-10-30, macOS 27.0.1 arm64; personal fork `feature/postgresql-integration`,
continuing from `3af0075`. Stage 3 implements database-explorer and SQL-editor
**backend only**. No graphical workspace, role/backup UI, service auto-start,
installed Fairway modification or personal database access was performed.

## Verified behavior

The existing SHA-pinned PostgreSQL 18.6 payload at
`/private/tmp/fw-pg2-review-runtime/postgresql` was reused read-only; no source
rebuild, runtime/dependency update, Homebrew command/service change, global default
or shell-profile change occurred. All PostgreSQL fixtures use fresh disposable
clusters and memory-only test vaults. Keychain was not accessed by these tests.

- Strict typed operations/targets/results/errors and independent native job registry.
  Memory-only session creation reports authentication **not checked**; jobs establish
  database/role/version identity. No external target gains cluster/process ownership.
- PostgreSQL catalog inspection for databases, schemas, relations, columns, indexes
  and views. Actual fixtures use Unicode and quoted identifiers containing semicolons
  and SQL-looking text, proving identifier/value separation against a real server.
- Ordered, filtered offset pages with primary-key tie breakers and hasMore; exact
  JSONB, UUID, text arrays, UTC timestamps, large decimals/BIGINT, NULL/empty strings,
  bytea and duplicate result-column labels. Unknown columns and oversized pages are
  rejected. Limits and absent stable keys remain explicit.
- Read-only defaults and explicit modifying-SQL mode/consent. A DML CTE is rejected
  by the server's read-only transaction; multiple statements are rejected by the
  extended protocol. Oversized DELETE RETURNING data is not committed; the original
  205 rows remain. Safe SQLSTATE/position contracts do not return a sensitive error
  literal; managed worker-session server logging suppresses that literal as well.
- Single-row streaming, row/cell/result limits, server statement timeout, three-second
  connection timeout against a stalled fixture listener, and bounded helper transport.
  The stalled listener remains owned/alive independently.
- Pre-dispatch cancellation, duplicate-job refusal, concurrent-job independence,
  connection-bound cancellation, session-close cancellation and owner-pipe EOF.
  The activity-catalog regression first observes the specific sleeping statement
  as active, then observes zero matching active statements after explicit cancellation
  and owner loss, using bounded polling rather than assuming startup timing. No external backend PID is killed.
- External loopback browsing succeeds as a separately created restricted fixture role;
  its scratch HBA entry is modified/reloaded only within this owned test fixture.
  Production external connection code never writes/reloads server configuration.
  Closing that descriptor leaves the server running. The managed fixture alone
  owns/stops its Child, through Stage 2's smart shutdown.

## Checks

| Check | Observed result |
| --- | --- |
| PostgreSQL milestone suite including Stage 1/2/3 native opt-ins | 22 passed before the final resolver regression |
| Final Stage 3 activity-catalog cancellation/owner-loss regression | Native fixture passed; observed active before and absent after |
| Resolver Busy contract regression | Passed without runtime/credential access |
| Default desktop/core Rust library suite, locked/offline | 186 passed; 9 opt-ins excluded |
| Strict Clippy, all targets/all features, locked/offline | Passed |
| CLI configuration, no default features | cargo check passed |
| TypeScript unit/IPC/error contracts | 26 passed |
| Production frontend build | TypeScript noEmit and Vite passed |
| Python regressions | 89 passed |
| Existing native PHP/MySQL worker smoke | Passed in fresh scratch database/home |
| Existing Node quit/update/PHP independence opt-in | Passed with read-only Node 22.23.4/26.12.1 binaries and disposable projects |
| Formatting, documentation links and diff whitespace | Passed |

Counts overlap. The nine ignored normal Rust tests are seven existing Node opt-ins
and the two PostgreSQL native fixtures; both PostgreSQL tests and one Node test were
run explicitly. The normal bare `--no-default-features --lib` Mac symbol-gate issue
remains as documented in Stage 1; use the documented terminal feature configuration.

Initial failures were investigated, not bypassed: macOS rejected RLIMIT_DATA/AS in
isolated child diagnostics, so owned-helper footprint supervision was implemented
with failure closed, core dumps disabled and existing result/deadline caps retained.
This is not a hard memory sandbox. A test attempting to demote the bootstrap role
failed with PostgreSQL's bootstrap-superuser restriction; the fixture was corrected
to create a separate restricted role. A headless TypeScript test initially imported
the browser bridge; pure contracts now live in `explorer-types.ts`. Clippy's two
redundant-conversion/format findings were corrected.

The final activity regression exposed a metadata-resolver race: contention was
incorrectly mapped from `busy` to `unavailable`. A zero-IO unit test reproduced the
mapping defect; connection and client-lease resolution now preserve `busy`. The
fixture retries only that pre-dispatch condition, observes the specific sleep query
as active, cancels/closes the owner pipe, then observes zero matching active queries.
It no longer infers server cancellation from a finished helper or an initial delay.
Final applicable checks pass; the expensive milestone suites were reused, and only
the affected native fixture, short Rust regressions and lint were rerun.

## Reproduction

After explicitly preparing/reviewing the pinned payload as described in
[Runtime preparation](postgresql-runtime.md):

```sh
cargo build --manifest-path src-tauri/Cargo.toml --locked \
  --no-default-features --features terminal --example postgresql_health_worker
FAIRWAY_TEST_POSTGRES_PAYLOAD=/canonical/reviewed/postgresql \
FAIRWAY_TEST_POSTGRES_WORKER="$PWD/src-tauri/target/debug/examples/postgresql_health_worker" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked \
  --no-default-features --features terminal --lib postgresql \
  -- --include-ignored --test-threads=1
```

This opt-in executes known native tools and explicit SQL only in newly created
scratch clusters. It never uses a personal profile/database. Failed fixture stages
are retained; successful fixtures remove only their disposable test directories.
Temporary logs are `/tmp/fw-pg3-*.log`, not committed artifacts.

## Blockers and limits

- **Stage 2 blockers remain:** final signed-runtime packaging/pin order and real
  signed-app Keychain access are unverified. These memory-vault backend fixtures do
  not establish managed-cluster acceptance. No credential fallback is added.
- Qualified path: macOS arm64, pinned client/server 18.6, explicit loopback only.
  External remote verify-full requests fail closed because this client lacks TLS.
  Windows/Linux/Intel, older macOS/server releases, actual Homebrew/system/remote
  installations, TLS/CA verification and signed desktop IPC are unverified.
- External passwords are explicit session-only input and never persisted. External
  server authentication/logging policy is not changed or guaranteed by Fairway.
  The client still requires the provisioned verified runtime even for external use.
- Offset pages are separate snapshots and may change under concurrent writes. Views
  and tables without primary keys can have unstable order. Catalog/query row caps
  do not enumerate arbitrarily large catalogs. No COUNT, keyset cursors, row-editing
  adapter or persistent connection/profile storage is implemented.
- SQL is one statement/one bounded transaction per job. Scripts, COPY streams, manual
  transactions and transaction-external commands are unsupported. Read-only is a
  PostgreSQL transaction policy, not a sandbox against privileged functions/views,
  foreign data wrappers or filesystem/network side effects. Use least privilege.
  Nontransactional effects cannot be promised undone; uncertain commit is reported.
- Cancellation is best effort and may race completion/commit. Worker cleanup never
  kills a server; server deadlines are a fallback. Physical-memory supervision can
  overshoot between samples and is not a hard OS allocation limit. Actual memory-
  pressure/forced-helper-death and commit-loss races are not exhaustively fault-injected.
- No GUI, native WebView/tray/update-install acceptance, signed/release build or full
  browser workspace suite was performed. Stage 2 orphan/journal/manual-review limits
  remain. No Stage 4 work has begun.

Stage 4 may integrate this backend with the graphical workspace after a new request,
while preserving the same acceptance blockers and explicit write/cancel/outcome
states. Release readiness requires the unresolved signed/vault/TLS/platform checks,
not merely these backend test results.
