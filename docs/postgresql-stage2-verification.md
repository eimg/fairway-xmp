# PostgreSQL Stage 2 verification

2026-10-30, macOS 27.0.1 arm64. Work continues from `4af4d1e` on the personal fork's
`feature/postgresql-integration`. No installed Fairway, existing PostgreSQL cluster,
Homebrew service, global default/profile, main/upstream or other feature branch was
modified. No GUI or Computer Use was used. All server fixtures used fresh scratch
home/data directories and ephemeral loopback ports.

## Runtime and lifecycle evidence

The user explicitly authorized the official PostgreSQL 18.6 source configure/make/
DESTDIR installation in disposable staging. The source SHA-256 matched the committed
pin and official sidecar before source execution. No sudo, package installation or
Homebrew modification occurred. One compilation was used; packaging refinements
reused its output. The rejected bottle candidate's actual initdb logs showed
embedded Homebrew catalog/timezone paths; no global symlink workaround was used.

The accepted 41 MiB payload contains 45 arm64 Mach-O files: signatures and private/
system-only dependency closure passed, and all nine fixed tools returned 18.6.
The Mach-O minimum is macOS 15.0, SDK 27.0. The reviewed manifest SHA-256 is
`27159615e4b9992d31ab311fd083382b30c7077ff052de1c8f36dd465aa41013`.
See [the runtime recipe](postgresql-runtime.md); compiler/signing changes require
new review and pinning, never automatic acceptance based only on version strings.

The real fixture provisions the runtime, initializes a private SCRAM cluster,
starts/authenticates/stops/restarts it, distinguishes accepting from rejected
credentials, rejects inherited libpq service settings, and verifies that smart
shutdown times out safely with an authenticated active client while retaining the
lease. It retries only after the fixture query finishes. It also checks reserved
and occupied ports, durable credential intent on failed vault before cluster creation, filesystem/inode/link
protections, changed permissions/config/runtime, retained PID markers, reopening
owned registration/logs, interrupted registration refusal, unrelated listener
survival and absence of the plain secret from retained metadata/logs.

The native lifecycle fixture uses a **test-only memory vault**, never the login
Keychain. No real Keychain item was added/read/deleted during automated tests.
Production uses Security.framework with no plaintext or memory-only fallback.

An initial active-client fixture incorrectly assumed closing psql cancelled its
running statement. The server correctly continued smart shutdown waiting. The
fixture was corrected to await a short bounded query; no forced server shutdown
was introduced. Failed fixture stages/logs were retained for diagnosis.

## Reproduction

With the reviewed payload available and normal Rust prerequisites:

```sh
cargo build --manifest-path src-tauri/Cargo.toml --locked \
  --no-default-features --features terminal --example postgresql_health_worker
FAIRWAY_TEST_POSTGRES_PAYLOAD=/canonical/reviewed/payload \
FAIRWAY_TEST_POSTGRES_WORKER="$PWD/src-tauri/target/debug/examples/postgresql_health_worker" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked \
  --no-default-features --features terminal --lib postgresql \
  -- --include-ignored --test-threads=1
```

The ignored test is intentionally opt-in: it executes the pinned tools and creates
only disposable clusters. Never point a fixture at installed Fairway or personal
home/data. The seven pre-existing Node opt-ins require their separate controlled
runtimes/runner; one targeted quit/update/PHP independence test was run in this stage.

## Unverified acceptance and release limits

- macOS Keychain success/locked-denied behavior from the final **signed** app remains
  unverified. Automated tests prove the controller fails before initdb when its
  vault is unavailable; they do not prove a live OS-vault round trip. No security
  fallback is enabled to bypass this qualification.
- The headless `postgresql_managed_smoke` example is compile-checked only. An operator
  may explicitly run it in a disposable macOS account with a reviewed payload at
  `RUNTIME_ROOT/postgresql`; it creates a new app-owned Keychain item and retains
  the cluster/secret, never deleting either:

  ```sh
  cargo run --manifest-path src-tauri/Cargo.toml --locked \
    --no-default-features --features terminal --example postgresql_managed_smoke -- \
    --confirm-managed-cluster-init /canonical/new-test-profile /canonical/runtime-root
  ```

  This CLI check does not replace signed desktop Keychain/hardened-loader tests.
  Do not run it against a personal profile or existing cluster.
- Release workflows still omit the independent server payload. Final nested-code
  signing must precede manifest pinning/app compilation; signing changes integrity
  hashes. Signing/notarization/installer and older supported macOS qualification
  remain blocked, not bypassed. Windows/Linux/Intel managed runtimes are unsupported.
- No service UI, automatic startup, explorer, SQL editor, connection profiles,
  external server control, remote TLS, role management or backup/restore was added.
  No Homebrew/system installation was used for administration.
- Shutdown is smart-only. Active clients can delay it; app crashes/Quit anyway can
  leave an orphan server. PID markers and interrupted stages are retained and never
  adopted/repaired/deleted automatically. There is no automated log pruning; after
  32 captures Start refuses. POSIX permissions/ownership/link checks are tested;
  hostile same-user races and extra filesystem ACL policies are not sandboxed.
- Native Tauri IPC/WebView/tray/update installation, real Mac acceptance, full browser
  suite and signed/release builds were not run. Native backend evidence must not be
  confused with those checks. Stage 3 has not begun and needs new authorization.

The reviewed development payload is retained at
`/private/tmp/fw-pg2-review-runtime/postgresql` (current-user private staging).
It is not an installed runtime or a signed distribution. Temporary evidence is in
`/tmp/fw-pg2-*.log`; binaries, payloads and build/failed
fixture stages are disposable and excluded from git. Regression results below are
source/backend evidence, not a fresh release certification.

## Regression results

| Check | Observed result |
| --- | --- |
| Focused PostgreSQL discovery/managed/contract fixtures, including real cluster | 17 passed |
| Default desktop/core Rust library suite, locked/offline | 181 passed; 8 opt-ins excluded |
| Strict Clippy, all targets/all features, locked/offline | Passed |
| CLI configuration, no default features | cargo check passed |
| TypeScript unit/IPC contracts | 24 passed |
| Production frontend build | TypeScript noEmit and Vite passed |
| Python regression suite, including hostile source archive/checksum fixtures | 89 passed |
| Existing native PHP/MySQL data-browser smoke | Passed using bundled tools and a new scratch database |
| Existing Node quit/update/PHP independence opt-in | Passed with Node 22.23.4 and 26.12.1 read-only binaries and disposable projects |
| Native Keychain smoke example | Compile/Clippy checked only; not executed |
| Formatting, local documentation links and diff whitespace | Passed |

Counts overlap. The eight normal Rust opt-ins include seven existing Node tests
and the PostgreSQL native test; the latter and one Node test were explicitly run
separately. The unchanged bare `--no-default-features --lib` Mac test configuration
has the Stage 1 documented terminal-gated symbol issue; use the documented terminal
feature configuration. No extra toolchain/platform installations were performed.
