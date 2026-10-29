# Testing

Choose checks for the behavior being changed. Device checks help investigate platform behavior; they are not release approval requirements. Record observed results.

## Automated checks

Run the commands below locally as needed. For the complete Windows/macOS source checks and Chromium/WebKit suite in CI, use **Actions → Build and verify Fairway XMP → Run workflow**. Application verification and cache warming run only when explicitly requested; pushes and pull requests do not start them. Release candidates still run their full checks.

```sh
npm run build
npm run test:unit
python3 -m unittest discover -s tests -v
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features terminal --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

Use `python` on Windows. Desktop/all-feature tests additionally need the platform's Tauri prerequisites. Network-listener, process-ownership and PostgreSQL checks need normal local socket/process access; sandbox permission failures are not evidence of an application defect.

For documentation website changes, run `npm run site:check`. With `npm run site:dev` serving the preview, run `npm run site:test:navigation` and `npm run site:test:theme` for browser navigation, accessibility and theme behavior. See the [website source guide](../website/README.md) for setup and screenshot capture. These checks use `website/dist/`, independently of the desktop frontend.

After staging the runtime, use a fresh isolated directory:

```sh
python3 scripts/prepare_runtime.py --verify-only
cargo run --manifest-path src-tauri/Cargo.toml --example smoke --no-default-features --locked -- /path/to/fresh-smoke
python3 scripts/check_postgres_clients.py
cargo run --manifest-path src-tauri/Cargo.toml --example php_extension_smoke --no-default-features --locked -- /path/to/fresh-extension-smoke
cargo run --manifest-path src-tauri/Cargo.toml --example php_settings_smoke --no-default-features --locked -- /path/to/fresh-settings-smoke
```

The service smoke checks real PHP/Composer, Nginx, MySQL, mail, backup restoration, process recovery and every bundled PHP version. The PostgreSQL check starts its own disposable cluster using external server binaries and checks both PHP APIs through CLI and web requests. Set `PGBIN` or pass `--postgres-bin` when needed. It does not stop or modify an existing PostgreSQL service. See [PostgreSQL clients](postgres-client-support.md).

The extension smoke checks `sockets`, SOAP and LDAP initialization/options/escaping through generated CLI/web configuration on every PHP branch, plus Unix CLI signals and child exit status using `pcntl`/`posix`. It requires local socket/process access but no external service. It does not authenticate against an LDAP directory. See [PHP clients and workers](php-client-support.md).

The PHP settings smoke checks each branch through generated CLI/web configuration, multipart uploads with session progress disabled, upload-size enforcement, restart persistence, inactive-version edits, failed-save rollback, reset, unlimited execution and a quiet 65-second response that exceeds Nginx’s former 60-second timeout. `php-settings-native.spec.ts` additionally drives the production settings UI against the real Rust backend; `php-settings.spec.ts` covers per-version forms and running-service behavior in Chromium/WebKit.

The settings smoke verifies the identity of the loaded `php.ini` before checking values. Its Windows fixture uses an ASCII data-directory name because PHP startup requires an ANSI-compatible configuration path or existing NTFS short aliases; the runner's workspace volume need not provide those aliases. Project paths remain Unicode. Other platforms also exercise Unicode application-data paths.

Database engine upgrade fixtures and installed-lifecycle baselines remain automated compatibility tests. Their old version names identify test inputs, not current development restrictions. Installer lifecycle scripts modify profiles only in disposable GitHub-hosted accounts; do not run them against a personal account.

## Browser and native bridge

`node-tools.spec.ts` covers desktop Install/Install & use, explicit selection, failed activation/retry, read-only stores, activation handover, confirmed active-version uninstall, and cancellable downloads across navigation in Chromium/WebKit at normal/minimum sizes. Native selection regression tests verify rollback of existing and absent defaults, failed fnm selection, and process-tree cancellation.

Node/fnm checks are noninteractive and use isolated stores. Policy tests cover external-manager detection after Fairway launchers, shell-exported custom storage, ownership-bound mutations, read-only listing and activation handover without invoking fnm. On macOS run `cargo run --manifest-path src-tauri/Cargo.toml --example node_tools_smoke --no-default-features --features terminal --locked -- /path/to/fresh-fnm-check`; on Windows use `node_tools_windows_smoke`. The examples install Node into scratch storage, test version selection and native startup scripts, and preserve personal profiles/installations. Mac/Windows candidate jobs run the appropriate example after staging fnm.

On Linux, after preparing the runtime, run `python3 tests/linux_fnm_smoke.py --runtime src-tauri/resources/runtime --install-node`. It checks the bundled native fnm and downloads Node LTS into a temporary home/store, then verifies Node/npm/npx without editing personal shell profiles. Omit `--install-node` to check fnm without a Node download. See [Node tools](node-tools.md).

`npm run dev` is a disabled browser preview. `npm run dev:ui` enables an explicitly labeled in-memory backend for service/error/update scenarios; no native files, services or settings are changed.

```sh
npx playwright install chromium webkit
npm run test:ui
npm run test:ui:report
```

The browser suite covers Chromium/WebKit at normal and minimum desktop sizes and attaches screenshots, traces and failure recordings. Assertions establish behavior; screenshots remain available for optional visual review. Browser engines do not reproduce native tray menus, dialogs or Windows ConPTY.

For real packaged tools and database operations, build `terminal_bridge` and use `FAIRWAY_TEST_RUNTIME` with the `*-native.spec.ts` suites. Candidate workflows use `FAIRWAY_NATIVE_ONLY=1` with the built production frontend. The bridge uses isolated data, communicates over stdio, and opens no database HTTP endpoint.

## Isolated development profiles

For a debug desktop build on macOS:

```sh
export FAIRWAY_DEV_HOME="$PWD/.runtime-work/test-home"
export FAIRWAY_DEV_DATA="$PWD/.runtime-work/test-data"
export ZDOTDIR="$FAIRWAY_DEV_HOME"
mkdir -p "$FAIRWAY_DEV_HOME" "$FAIRWAY_DEV_DATA"
npm run tauri -- dev
```

On Windows PowerShell, set `$env:FAIRWAY_DEV_HOME` and `$env:FAIRWAY_DEV_DATA` to fresh local directories before launching. These overrides are debug-only. Standalone installed/signature test apps can use the normal profile: quit the installed app first and use a disposable OS account when changing hosts or trust.

To verify PHP switching, stop services, select each bundled version in Settings, and compare a served `<?php echo PHP_VERSION;` page with the app-owned PHP/Composer launcher. Reopen the app and verify the selection persists. Linux uses `fairway stop all`, `fairway php-version VERSION`, `fairway php --version` and `fairway start all`. Read expected patch versions from `runtime-manifest.json`.

## Optional platform checks

Use these when the changed behavior warrants them, without treating unperformed cases as release blockers:

- Install/update/uninstall/reinstall while preserving projects, rows, sites, ports, selected PHP, backups and CLI integration. Verify unrelated services and shell settings remain untouched.
- Exercise tray/menu-bar controls, window hiding/reopening, Quit during work, and both choices after failed shutdown. On macOS, closing must hide the Dock icon while keeping the menu bar and services running; Show Fairway XMP and relaunching must restore the window and Dock icon. Check repeated close/reopen cycles and that minimizing keeps the Dock icon. Use real PTYs/ConPTY for terminal interruption, resize, clipboard and cleanup behavior.
- Check Data browser native pickers, Unicode paths, large values, clipboard failures and MySQL/SQLite source switching. Detailed database scenarios are in [Data browser](data-browser.md).
- Verify registered `.test` addresses through the OS resolver and a normal browser, not only `curl --resolve`, `--cacert` or `-k`. Check direct files, front controllers, custom ports, removal and preserved unrelated hosts entries. Malformed saved site lists should surface errors without replacing them.
- On macOS, test fresh background-helper approval in System Settings, explicit recheck/continuation, cancellation, password fallback, focus return and helper reuse. Hosts authorization and user-keychain HTTPS trust are separate operations; a biometric check alone does not grant hosts-write permission. Prompt count and available authentication methods depend on macOS/account state.
- For HTTPS, enable a first site, reuse trust for another, disable the last site, cancel trust removal and retry. Test root replacement cancellation/recovery and actual DNS/IP/subordinate-chain rejection in the platform verifier. See [Security boundaries](security-boundaries.md).
- Check a subsequent-version signed update and restart when investigating updater behavior. The automated same-version reinstall drill has a narrower scope; neither result implies the other was performed.

Before moving/deleting a standalone Mac helper test app, quit it and run its own executable with `--remove-native-hosts-helper`. This unregisters that helper without deleting projects or rewriting sites. Avoid running multiple app copies against the same data/helper registration while diagnosing approval state.
