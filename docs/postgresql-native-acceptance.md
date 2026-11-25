# PostgreSQL external workspace acceptance

Work on `feature/postgresql-integration` in the personal fork. The native workspace
uses Tauri PostgreSQL IPC and the private, integrity-checked libpq worker.
`npm run dev:ui` is an in-memory simulation, including its authentication labels.
Browser screenshots/tests do not establish database access or Mac WebView acceptance.

## Prepared development runtime

The project-local `src-tauri/resources/runtime/postgresql` payload is PostgreSQL
18.6, built from the unchanged checksum-pinned official source using the existing
authorized preparer. All 45 arm64 Mach-O files passed signing/dependency checks.
The reviewed development manifest pin is
`be6301329eff37fde51ab08099b4604de94eeba029efc0625a7d2345fd8f6378`.
Compiler/SDK differences changed the previous development payload's bytes; version,
source checksum, configure options and integrity/ownership checks remain intact.
The payload is ignored by Git. Build logs/stages are retained in `.runtime-work/`,
outside bundled resources. Do not copy it over an existing provisioned runtime.
Use fresh isolated profiles; an older payload/pin continues to fail closed.
Signed distribution/Keychain qualification remains pending; see
[runtime preparation](postgresql-runtime.md) to reproduce a missing payload.

External servers accept PostgreSQL 17.x and exactly 18.6. Managed servers still
require exactly 18.6 and their owned cluster identity. Real 17.12 access was tested
on this host; this is not qualification of every 17.x patch/platform. Connections
remain explicit loopback sessions without TLS. Passwords are session-only.

## Real read-only backend check

From the project root (add `$HOME/.cargo/bin` to this command's PATH if necessary):

```sh
cargo build --manifest-path src-tauri/Cargo.toml --locked --offline \
  --no-default-features --features terminal --example postgresql_external_smoke
python3 scripts/check_postgresql_external.py \
  --credentials-file /Users/rioo/projects/summit_task/.env
```

The Python helper reads literal `POSTGRES_USER`/`POSTGRES_PASSWORD` values only;
it never sources the dotenv file, exports credentials or prints them. It rejects
ambiguous/escaped credential syntax. The native example uses a fresh disposable
profile, provisions only the client payload, opens an external session through
production `Explorer::connect/execute`, then disconnects. Credentials cross private
stdin only. It checks database/server identity, databases, schemas, relations,
columns, indexes, rows, bound equality filters, descending sort and an offset page.
It prints version/counts, withholding all account values. It creates no server,
cluster, account or Keychain item. No Summit schema/data/configuration is changed.

To check the actual desktop executable's private worker without opening the GUI:

```sh
TAURI_CONFIG='{"bundle":{"resources":[]}}' cargo build \
  --manifest-path src-tauri/Cargo.toml --locked --offline --bin fairway
FAIRWAY_TEST_POSTGRES_WORKER="$PWD/src-tauri/target/debug/fairway" \
  python3 scripts/check_postgresql_external.py \
  --credentials-file /Users/rioo/projects/summit_task/.env
```

## Operator Mac GUI acceptance

Run these commands yourself; no GUI automation is required. The separate app
identifier avoids handing the launch to an installed Fairway instance. The fresh
debug profile keeps its settings/data/launchers separate. Leave Summit running.
The debug configuration omits redundant bundle resource copies: debug Fairway reads
`src-tauri/resources/runtime` directly. Tauri otherwise tries to overwrite its
previous read-only PostgreSQL copies on a later build (`Permission denied`). This
does not change the runtime pin, file protections or release bundle configuration.

```sh
cd /Users/rioo/projects/fairway-xmp-personal
mkdir -p .runtime-work
PG_ACCEPTANCE_ROOT="$(mktemp -d "$PWD/.runtime-work/pg-gui-XXXXXX")"
mkdir -p "$PG_ACCEPTANCE_ROOT/home" "$PG_ACCEPTANCE_ROOT/data"
PATH="$HOME/.cargo/bin:$PATH" \
FAIRWAY_DEV_HOME="$PG_ACCEPTANCE_ROOT/home" \
FAIRWAY_DEV_DATA="$PG_ACCEPTANCE_ROOT/data" \
ZDOTDIR="$PG_ACCEPTANCE_ROOT/home" \
npm run tauri -- dev --config '{"bundle":{"resources":[]},"identifier":"dev.fairway.personal.pgacceptance","build":{"beforeDevCommand":"npm run dev -- --port 1440 --strictPort","devUrl":"http://127.0.0.1:1440"}}'
```

1. Open **PostgreSQL → Prepare runtime** and authorize the runtime copy only.
   Do not initialize/start a managed cluster for Summit.
2. Open **New connection**, select **External PostgreSQL · loopback**, and enter
   host `127.0.0.1`, port `55432`, database `vector_one_auth`. Enter the user and
   password from Summit's `.env` manually; do not put them in shell commands,
   screenshots, issues or saved connection files. Authorize the connection.
3. Confirm **Authenticated by last successful operation**, choose schema
   **vector_one**, then table **app_user**. Confirm the account you created through
   Summit appears. The backend check only proves existing rows are readable; it
   does not establish which UI originally created them.
4. Create an account through Summit's normal registration UI if a fresh marker is
   needed. Back in Fairway, click **Rows** to reload, or **Refresh explorer** and
   reselect the schema/table. Find that account using a column equality filter.
   This manual step is the end-to-end Summit-to-native-GUI acceptance criterion.
5. Check columns/indexes and sorting. Disconnect, then reconnect: the password must
   be entered again. Quit through Fairway's Quit action; Summit remains running.

Do not use a browser-only dev page as acceptance. Never run modifying SQL for this
check. No database upgrades/resets, Docker volume operations, installation changes,
merge or push are part of this workflow.

## Observed verification (2026-11-24)

- Real production native worker against Summit: `server_version_num=170012`,
  `vector_one.app_user`, 8 columns and 2 rows; account values withheld.
- Production TypeScript/Vite build and all 32 TypeScript unit tests passed.
- Focused simulated workspace suite: 20 passed across Chromium/WebKit at normal
  and minimum sizes, on port 1439 (the existing port 1421 server was left alone).
- All 24 PostgreSQL native tests passed, including the rebuilt payload's disposable
  lifecycle/integrity and Stage 3 query/cancellation fixtures.
- Desktop `cargo check` and executable build passed (debug resource-copy override
  above). The built desktop executable's private worker independently passed the
  real Summit catalogs/rows/filter/sort/page check, without initializing the GUI.
- Focused strict Rust Clippy, formatting, two source-preparation safety tests,
  changed documentation links, whitespace and credential scans passed.
- Mac GUI/account-origin acceptance remains operator work, as described above.

Native socket checks initially failed under the restricted sandbox and were rerun
with local socket access. Such permission failures are not authentication evidence.
