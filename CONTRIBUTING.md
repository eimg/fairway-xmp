# Contributing to Fairway XMP

Fairway uses Tauri, Rust, React and TypeScript. [PROJECT_SPEC.md](PROJECT_SPEC.md) describes current behavior; the [documentation index](docs/README.md) points to focused guides. Work follows current requirements without an inherited release-stage freeze. Device testing is optional, and publication is an explicit owner action.

## Development setup

Install Node.js 24, Python 3.12+, Rust stable (at least 1.90) and the platform's Tauri prerequisites. Windows needs the MSVC toolchain, C++ Build Tools and Windows SDK; macOS needs Xcode Command Line Tools. These are builder requirements, not student-machine dependencies.

```sh
npm ci
npm run dev                    # disabled browser preview
npm run dev:ui                 # labeled in-memory UI scenarios
python3 scripts/prepare_runtime.py
npm run tauri -- dev           # native app with the staged runtime
```

On Windows use `python` and, if PowerShell blocks `npm.ps1`, `npm.cmd`. Runtime preparation verifies pinned official Windows archives or Mac bottles/upstream sources and stages the complete runtime without a system MySQL/PHP installation. Keep the committed download locks for ordinary preparation; do not refresh unrelated runtime inputs to resolve a local setup problem.

The UI simulation resets on reload, uses example paths, and does not change real files/services. To preview a machine without Node, choose **Scenario → Node not detected**, then open **Settings**. Choose **Existing fnm**, **Existing fnm store** or **fnm handover** to preview read-only guidance and explicit activation handover. The simulated Install & use flow works on both example platforms; changing scenarios resets Node installations and activation. Native development uses real services. Set `FAIRWAY_DEV_HOME` and `FAIRWAY_DEV_DATA` to isolated directories when testing startup, settings, launchers and data. See [Testing](docs/testing.md#isolated-development-profiles).

## Documentation website

The published how-to guides are available in [English](https://eimg.github.io/fairway-xmp/)
and [Myanmar](https://eimg.github.io/fairway-xmp/mm/). For guide content, translations,
local preview and publication, see the [website source guide](website/README.md).
The website builds independently of the desktop app with `npm run site:check`.

## Verification

```sh
npm run build
npm run test:unit
python3 -m unittest discover -s tests -v
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features terminal --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

Use the [testing guide](docs/testing.md) for browser/native bridge suites, real-runtime smoke checks, PHP version switching, PostgreSQL connections and optional platform checks. Use a fresh isolated home/data directory for service tests. Installer lifecycle tests are for disposable hosted accounts, not your personal profile.

Run **Actions → Build and verify Fairway XMP → Run workflow** for Windows/macOS source checks and Chromium/WebKit browser assertions/screenshots. Ordinary pushes and pull requests do not start these checks or cache warming. Installer workflows add real services, database restore, process recovery, native bridge and installation/update/removal checks. Browser fixtures establish UI behavior, not native dialogs or a platform's exact WebView rendering.

## Windows builds

On Windows x64, install Rust's `x86_64-pc-windows-msvc` target and the C++ workload. After staging the runtime:

```powershell
npm run tauri -- build --bundles nsis --target x86_64-pc-windows-msvc
```

Output is under `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. Quit the installed app through its tray before manually replacing it; closing the window only hides it. Missing compiler/linker DLLs are builder setup issues; staged PHP extensions and the selected runtime must pass verification before packaging.

## macOS builds

Apple Silicon/macOS 15+ is supported. Signed distribution builds include the native hosts helper and user-keychain trust features:

```sh
python3 scripts/build_macos_hosts_helper.py --team-id 4Q27NQ2UAF --output artifacts/hosts-helper
npm run tauri -- build --bundles app --features native-macos-trust,native-macos-hosts --no-sign
ditto artifacts/hosts-helper/Library 'src-tauri/target/release/bundle/macos/Fairway XMP.app/Contents/Library'
python3 scripts/verify_macos_permissions.py 'src-tauri/target/release/bundle/macos/Fairway XMP.app'
```

The helper build is offline and needs its Rust dependencies already available. `scripts/package_macos.sh` verifies the helper/features and signs nested code before sealing the app. For signed packaging and credentials, follow [Build and release](docs/build-and-release.md#signing). A development app signed locally is not automatically notarized. Use standalone workflow outputs to test permissions without replacing the installed application.

### macOS Node verification

The macOS fnm integration has a non-interactive smoke example. It downloads Node into a fresh scratch home/store, checks native fnm commands, npm/npx, default activation, profile preservation and removal, and never edits your own shell profiles:

```sh
cargo run --manifest-path src-tauri/Cargo.toml --example node_tools_smoke --no-default-features --features terminal --locked -- "$PWD/.runtime-work/fnm-check"
```

Use a fresh directory for each run. Node downloads require network access. The macOS and Windows runtime preparers stage and verify their pinned upstream fnm binaries. On Windows, run the equivalent isolated check (no UI, real shell profiles or personal registry changes):

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --example node_tools_windows_smoke --no-default-features --locked -- .runtime-work/fnm-windows-check
```

The Mac and Windows workflows run their respective native fnm checks after runtime preparation. Native unit tests exercise profile/AutoRun restoration against scratch files and a unique test registry subkey. The smoke check covers Node downloads, native CLI copies, Node/npm/npx, the actual Command Prompt and PowerShell startup bodies, retained versions in existing terminals, and default uninstall. Linux stages the unchanged upstream x64 binary and the same complete fnm/crate sources. Its headless smoke check exercises native fnm and optional Node downloads in an isolated store.

To verify existing tools against your current macOS shell setup without opening the UI, run:

```sh
cargo run --manifest-path src-tauri/Cargo.toml --example terminal_tools_smoke --no-default-features --features terminal --locked
```

This opens a headless embedded shell with command history off and prints only selected command paths and Node’s version. It checks Fairway PHP/Composer priority without logging exported environment values.

## Linux terminal builds

The Linux Rust CLI uses `--no-default-features --features cli`, without Tauri or desktop libraries. Build on Ubuntu 22.04 for the glibc 2.35 baseline. The builder packages and `deb-src` setup are defined in `.github/actions/prepare-linux-runtime/action.yml`.

```sh
python3 scripts/prepare_runtime.py --lockfile runtime-linux-downloads.lock.json
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --locked
cargo build --release --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin fairway-cli --locked
python3 tests/linux_fnm_smoke.py --runtime src-tauri/resources/runtime --install-node
python3 scripts/package_linux.py
python3 tests/linux_cli_smoke.py artifacts/linux-installer/fairway-xmp-*-linux-x86_64.run
```

The Linux workflow packages `.run`/`.tar.gz` installers, source archives, and checks clean distro environments. Target machines need no compiler or installed runtime library packages. The native fnm smoke checks its bundled version and optionally downloads Node LTS into a temporary store, then verifies Node/npm/npx. Node and npm are not required for building or using the Linux PHP environment. Service tests run as an ordinary user and bind to loopback.

## Builds, publication and maintenance

[Build and release](docs/build-and-release.md) is the single guide for candidate CI, corresponding sources, signing, owner-authorized publication and update feeds. The active publication workflow is `publish-release.yml`; it consumes successful candidate files without rebuilding. Required automated checks protect artifact identity, signatures and existing data. There is no device acceptance file or alpha/beta-specific approval process.

[Maintenance](docs/maintenance.md) covers changing runtime versions and source/download pins. [CI performance](docs/ci-build-performance.md) covers cache warming, retries and artifact cleanup. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) lists bundled licenses. Keep required corresponding sources beside distributed binaries.
