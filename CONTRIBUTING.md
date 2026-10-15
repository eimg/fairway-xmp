# Contributing to Fairway XMP

The application uses Tauri 2, Rust, React, and TypeScript. Source checks run on Windows and macOS. Manual/reusable workflows build platform installers as Actions artifacts; the unsigned candidate workflow combines them without publishing GitHub Releases. After testing, publish the exact downloaded installer rather than rebuilding it.

## Development

Install Node 24, Rust stable (at least 1.90), and the platform Tauri development prerequisites. Windows builders need Microsoft's C++ Build Tools; Mac builders need Xcode Command Line Tools. Students do not need these build tools.

```sh
npm ci
cargo install tauri-cli --version 2.12.2 --locked
npm run dev                   # browser interface preview; service controls disabled
cargo tauri dev               # native app; real services require staged runtime
```

The browser preview explicitly labels itself and never simulates service success.

## Visual UI review in the browser

Run `npm run dev:ui` and open `http://127.0.0.1:1420`. This uses the desktop app's React screens with a development-only, in-memory backend. The amber **UI test · Simulated data** toolbar offers stopped, running, partially running, port-conflict, missing-runtime, and update scenarios. Example Windows/macOS paths use the sample username `Student May` and include spaces. Choose a scenario, then use the ordinary controls to review pending states, service actions, logs, backup messages, PHP selection, settings validation, and update failures.

The example platform defaults to macOS on a Mac and Windows otherwise; it can be changed in the toolbar. CLI scenarios also cover installed commands missing from the app's inherited PATH, another PHP installation on PATH, and disabled integration. The main Command-line tools section displays Fairway's own installed commands. Expand **Command detection details** to inspect inherited PATH resolution; on macOS, a GUI app can inherit a different PATH from a new terminal. Windows fixtures use backslashes and `.cmd` launcher names.

All service, backup, settings, PATH, and update results in this mode are simulated. Open-folder/browser/terminal actions display what would be opened. Reload or **Reset test** restores the initial state; nothing is persisted and no native services or files are changed. Changing a scenario resets its data, notices, and update results. Ordinary `npm run dev` and production browser builds retain the disabled preview; the simulation is only enabled by the dedicated Vite development mode.

For repeatable checks and a visual review gallery:

```sh
npx playwright install chromium webkit  # once; on Linux add --with-deps
npm run test:ui                        # starts its own servers on 1421/1422
npm run test:ui:report                 # opens the HTML report
```

The tests exercise Chromium and WebKit at 1080×760 and the desktop minimum 820×650. Each scenario attaches Environment, Settings, Updates, and Help screenshots to the HTML report, including Windows/macOS example paths. Interaction tests cover pending states, logs, backups, settings validation/PHP selection, and recovery from a port conflict. Failures retain screenshots, video, and traces. These are human-review screenshots and behavior assertions; there is no approved pixel-diff baseline yet. `scripts/check_ui.mjs` remains an optional legacy check for the ordinary disabled preview.

The **ui-review** Actions job runs these checks on each push/PR and uploads `ui-visual-review-COMMIT` even when tests fail. Download and extract that artifact, then run `npx playwright show-report /absolute/path/to/playwright-report` to review it in your browser. A passing test run still needs your visual judgment. The candidate UI and native service checks must refer to the same source commit.

The full Windows/Linux service smoke checks currently run in their manually dispatched installer workflows. The regular Windows/macOS verification job builds and runs source checks; a green regular build alone does not confirm the full service smoke checks ran.

Browser review plus platform CI is the normal development loop. Browser engines and example paths do not reproduce an OS or its native WebView exactly. The Linux terminal dashboard, installers, tray menus, native dialogs, PATH integration, and database upgrades remain covered by their separate checks and targeted native testing when those areas change. See the [Playwright browser documentation](https://playwright.dev/docs/browsers) for engine differences.

To stage native runtime resources (Python 3.12+):

```sh
python3 scripts/prepare_runtime.py --refresh-lock
# If a download was interrupted: python3 scripts/prepare_runtime.py --resume-lock
# Review and commit runtime-downloads.lock.json after initial preparation.
python3 scripts/prepare_runtime.py
```

Preparation downloads binaries into the workspace, verifies checksums, bundles required libraries, relocates macOS load commands, and checks PHP extensions. It does not install Homebrew or start services. macOS PHP 8.1–8.5/MySQL inputs use ARM macOS 15 bottles; PHP 8.0 uses an archived, checksum-pinned ARM Sonoma bottle because Homebrew removed the formula. Stable Nginx is compiled locally from the pinned upstream source against pinned PCRE2/OpenSSL bottles and SDK zlib, using Xcode Command Line Tools. Its configure inputs are retained in `runtime/nginx/build.json`. Windows inputs use official ZIP distributions. Runtime files and caches are intentionally ignored by Git.

Alpha 2 installers have passed the runtime and migration checks on both platforms, and the release owner has tested both installers. Corresponding-source collection and publication reuse those exact files; neither step rebuilds the installers.

Refreshing the lock trusts upstream HTTPS for inputs without independently published SHA256 checksums and records their SHA256. Review those inputs and available upstream signatures before a classroom release. Normal builds require the reviewed lock and reject changed inputs.

For native UI testing without touching your real user folders, debug builds accept both `FAIRWAY_DEV_HOME` and `FAIRWAY_DEV_DATA`; use absolute workspace paths. `FAIRWAY_RUNTIME_DIR` overrides the debug runtime directory. These folder overrides are disabled in release builds.

## Build locally on Windows

Use Windows 10 22H2 or Windows 11 x64. Install Git, Node.js 24, Python 3.12 or later (available as `python`), Rust stable with the MSVC toolchain, and Visual Studio Build Tools with **Desktop development with C++** and a Windows SDK. Install the Microsoft Edge WebView2 Runtime if it is missing. These are the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows).

Open a new PowerShell window after installing the tools, then run:

```powershell
git clone https://github.com/eimg/fairway-xmp.git
cd fairway-xmp
rustup default stable-msvc
rustup target add x86_64-pc-windows-msvc
npm ci
python scripts/prepare_runtime.py
npm exec --yes --package @tauri-apps/cli@2.12.2 -- tauri build --bundles nsis --target x86_64-pc-windows-msvc
```

Runtime preparation downloads and verifies the pinned Windows PHP/Nginx/MySQL runtime and supporting files. Use the committed checksum lock; no `--refresh-lock` or separate PHP/MySQL installation is needed.

The installer is under `src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\`, with a name ending in `-setup.exe`. Quit any running Fairway XMP instance using its tray menu before installing and testing the new build; closing its window leaves the app running.

For faster iteration after staging the runtime, run the native development app:

```powershell
npm exec --yes --package @tauri-apps/cli@2.12.2 -- tauri dev
```

If PowerShell blocks `npm.ps1`, use `npm.cmd` in these commands. If `link.exe` is missing, add the C++ workload in Visual Studio Installer. If runtime verification reports a missing Visual C++ DLL, install the [latest Microsoft Visual C++ x64 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) and rerun preparation.

## Verify

```sh
npm run build
npm run test:scripts
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo run --manifest-path src-tauri/Cargo.toml --example smoke --no-default-features --locked -- /absolute/path/to/isolated-test
# Optional MySQL migration check using the saved alpha 1 runtime:
cargo run --manifest-path src-tauri/Cargo.toml --example smoke --no-default-features --locked -- /absolute/path/to/isolated-upgrade /absolute/path/to/alpha1-runtime /absolute/path/to/alpha2-runtime
```

The smoke example starts real runtimes on available test ports and uses an isolated home/data directory. It never edits your real shell profile. Browser checks are in `scripts/check_ui.mjs` (requires Playwright and its Chromium browser).

## Package

```sh
# Windows, on a Windows builder:
cargo tauri build --bundles nsis

# Apple Silicon Mac:
cargo tauri build --bundles app
sh scripts/package_macos.sh
```

Windows setup uses current-user NSIS hooks and an offline WebView2 prerequisite. macOS packaging creates a pkg plus the app's bundled uninstall utility. Unsigned output can be published as an explicitly labeled alpha prerelease. Stable classroom releases require distribution signing and macOS notarization. `scripts/package_macos.sh` supports `APPLE_SIGNING_IDENTITY`, `APPLE_INSTALLER_IDENTITY`, and `APPLE_NOTARY_PROFILE` for distribution signing and notarization.

To configure a signed updater release:

```sh
python3 scripts/configure_release.py --public-key /path/to/updater.pub
cargo tauri build --config artifacts/tauri.release.json
```

Set Tauri's signing key environment variables on the builder. Updater signatures are separate from Apple/Windows distribution certificates. The release owner supplies the public key. The default feed is `https://github.com/eimg/fairway-xmp/releases/latest/download/latest.json`; unsigned builds link to the releases page for manual downloads. Ship all runtime resources with every release, preserve the application resource location, and verify installer/updater behavior on both OSes.

## Publishing a release

For new unsigned candidates, use the [beta candidate and acceptance process](docs/beta-acceptance.md). The **Unsigned release candidate** workflow orchestrates all platforms plus browser review from one commit, exercises actual installer upgrades/removal/reinstallation, verifies embedded identity, and generates an acceptance template for the exact hashes. It does not publish. Signing remains deferred; PHP 8.0/8.1 are legacy compatibility options, and their existing Windows source-verification follow-up remains open.

For an unsigned x64 Windows installer, dispatch the **Windows installer** workflow with `gh workflow run windows-test-build.yml --ref main`. It checks the source, stages the pinned runtime, exercises real services and cleanup, and verifies an 8.4 → 9.7 database upgrade before building an NSIS installer. Confirm the resulting run uses the intended source commit before accepting its installer. The alpha 1 migration fixtures retain its exact Windows versions and download checksums; the preparer stages them into a separate folder without replacing the current runtime.

Download its versioned artifact, for example `gh run download RUN_ID --name fairway-windows-x64-0.1.0-alpha.2 --dir artifacts/windows-alpha2`. It contains the installer, SHA256, exact build commit/run, runtime metadata, and application source archive. Actions retains it for 90 days; keep a local copy for later publication. Publish that tested installer unchanged. An application or runtime change requires another build; release notes or corresponding-source collection do not. The workflow never creates a GitHub Release.

The initial release is `v0.1.0-alpha.1`, an unsigned GitHub prerelease with manual updates. Publish alpha builds with clear installation limitations and never mark them as the latest stable release. Stable releases require installation, updates, backups, and removal checks on the supported systems.

On the Mac builder, after staging the locked runtime, run `python3 scripts/package_sources.py`. For a reviewed runtime version change, add `--refresh-lock`; ordinary source distributions retain their archive checksums. Gitiles snapshots with a committed revision and full Git tree can be recompressed upstream: every file, symlink and executable mode must still reproduce that pinned tree, and the source inventory records the actual archive checksum. Candidate builds use `--locked`, preserve the committed pins, and reject new inputs or changed content. The collector verifies the cached bottles, saves their recipes and build metadata, downloads version-matched upstream source and patches, and creates a bundled-tool source archive under `artifacts/alpha-VERSION`. Review and commit `runtime-sources.lock.json` when changing source versions. Use `--source-ref BUILD_COMMIT` when collecting sources from a newer checkout for existing installers. The archive also contains the application source at that build commit, Windows PHP dependency sources identified by the official SBOM or the pinned older-version manifest, and the runtime staging scripts. Source packages are distributed beside the installers; students only need their platform's installer.

The PHP 8.0 and 8.1 Windows ZIPs predate the official embedded SBOM. `scripts/windows_legacy_sources.json` records version-matched source commits for their Windows dependency packages; the source archive includes these trees and the distribution notices. The older ZIPs do not identify the exact dependency build commits, so review the mapping and any package-specific patches before publishing a Windows installer containing those versions.

Track this remaining source verification, the release owner's Mac visual test, and publication of the PHP-selection feature in [Release follow-ups](docs/release-follow-ups.md). The source archive hashes are pinned; the old Windows binary/source build and patch mapping is still open.

GPL binary releases must provide the corresponding source code, including packaging patches and required dependency sources, beside the installers. An upstream project link alone does not replace that source distribution. `runtime-downloads.lock.json` and `runtime-sources.lock.json` record the exact inputs.

The dedicated repository is [eimg/fairway-xmp](https://github.com/eimg/fairway-xmp). A stable GitHub release should include:

- Signed Windows and signed/notarized macOS installers.
- Tauri updater artifacts and their signatures.
- A `latest.json` file with the release version, notes, publication date, and supported platform entries. Each entry needs its signature and an HTTPS URL to the corresponding release asset. Windows uses `windows-x86_64`; Apple Silicon uses `darwin-aarch64`.
- Checksums and the corresponding source archives required by bundled components.

Keep the updater private key outside the repository. The public key used by installed clients must match the key signing update artifacts. Build the macOS app with the release overlay, then run the packaging script to produce its installer. Unsigned alpha prereleases omit updater artifacts and `latest.json`; users install updates manually. Include both installers, SHA-256 checksums, pinned runtime metadata, and the bundled-tool source archive. Use `--prerelease --latest=false` when publishing an alpha with `gh release create`.

See [PROJECT_SPEC.md](PROJECT_SPEC.md) for behavior and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for component licenses.

## Linux terminal builds

Linux terminal builds use the existing core through `--no-default-features --features cli`; Node, Tauri and desktop development libraries are not required. Build on Ubuntu 22.04 x86_64 to preserve the glibc 2.35 baseline. The **Linux terminal installer** workflow installs build dependencies, stages the reviewed runtime, tests the services, produces `.run`/`.tar.gz` installers and corresponding sources, and checks clean Ubuntu, Debian, Fedora and Arch containers. It uploads artifacts without creating a release.

```sh
gh workflow run linux-test-build.yml --ref main
# On the Ubuntu 22.04 builder with the workflow's build dependencies and deb-src enabled:
python3 scripts/prepare_runtime.py --lockfile runtime-linux-downloads.lock.json
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --locked
cargo build --release --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin fairway-cli --locked
python3 scripts/package_linux.py
python3 tests/linux_cli_smoke.py artifacts/linux-installer/Fairway-XMP-*-linux-x86_64.run
```

Keep `fairway-linux-x86_64-sources` beside the tested binary artifact when publishing. Sources include Ubuntu distribution patches for every copied system library and upstream sources pinned separately from Mac/Windows runtime downloads. No runtime libraries are installed on target machines. Linux services bind only to loopback and run as the current user. Use an isolated `HOME` and `XDG_DATA_HOME` when testing release binaries; the provided smoke script does so automatically.

Alpha 3 reuses tested binaries whose embedded version is alpha 2. `BUILD-INFO.json` records their original commits, runs and checksums. The Mac installer is repackaged from the existing app, without compiling it. The release’s source version is alpha 3 for future builds. `publish-reused-alpha.yml` stages assets on a GitHub runner and validates their hashes before uploading them to a draft release; publication happens after all assets are verified.
