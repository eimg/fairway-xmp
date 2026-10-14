# Fairway XMP development environment

## Product

Display name: **Fairway XMP** on Windows, macOS and Linux. The green application mark is shared by the app, Windows installer, and uninstaller. The macOS bundle is `/Applications/Fairway XMP.app`; installer hooks and removal target only that bundle.

A classroom PHP development environment with a desktop control panel on Windows/macOS and a terminal dashboard on Linux.
Supported targets: Windows 10 22H2 / Windows 11 x64, macOS 15+ Apple Silicon, and Linux x86_64 with glibc 2.35+. Linux ships as a terminal edition; CI covers its installer and five distro runtimes, with release-owner testing still encouraged.
No Docker/WSL requirement, distro-specific Linux packages, ARM Linux or musl support, Intel Mac packages, Node/npm bundle, or Laravel-specific project management.

## Application and runtimes

Tauri 2, Rust service manager, React/TypeScript UI. Exact versions and required PHP extensions are recorded in `runtime-manifest.json`. Download inputs are pinned by SHA256 in `runtime-downloads.lock.json` once prepared and reviewed. Runtime preparation never installs Homebrew or system services.

The alpha 2 runtime uses PHP 8.5.11 CLI/FastCGI, stable Nginx 1.30.5, MySQL 9.7.2 LTS, phpMyAdmin 5.2.3, Composer 2.10.3, CA trust roots, native runtime libraries and upstream notices. Mac Nginx is compiled from its pinned upstream source using pinned PCRE2/OpenSSL bottles and the macOS SDK's zlib. No system installation or Homebrew service is required. Windows inputs are official x64 ZIP distributions with checked SHA256 pins.

Linux uses a separate `cli` Cargo feature, without Tauri, WebView or GUI dependencies. Runtime inputs are SHA256-pinned in `runtime-linux-downloads.lock.json`; PHP and Nginx are compiled on the glibc 2.35 baseline. MySQL uses the official generic x86_64 archive. Each component bundles its native library closure with relative RUNPATHs; MySQL and PHP OpenSSL versions remain isolated. Build validation rejects newer glibc symbol requirements. Corresponding source archives include the exact app commit, PHP/Nginx/MySQL sources and Ubuntu sources/patches for copied libraries.

## Linux terminal edition

`fairway` opens the ANSI terminal dashboard; CLI commands provide status (including JSON), Start/Stop/Restart all or Nginx/MySQL, logs, ports, backup, bundled PHP/Composer/MySQL tools, a shell and managed PATH integration. PHP-FPM follows Nginx; PHP CLI does not require starting web services. phpMyAdmin uses the displayed browser URL.

A detached user supervisor owns the service manager. A private Unix socket and lifetime advisory lock coordinate multiple clients; mutations run sequentially, while status/log requests report busy promptly during an operation. Requests identify the installation before performing actions. No root execution, systemd dependency or automatic service startup. `q`/Ctrl+C close the TUI while services persist; `Q`/`fairway shutdown` queue graceful shutdown after pending operations, then stop services and the supervisor. TUI commands are delivered before acknowledging their keys, so closing immediately cannot cancel an accepted action. Failed shutdown leaves the controller available. Linux process recovery verifies `/proc` executable and exact service configuration before adopting MySQL/Nginx/PHP-FPM; recovered ownership also checks process start time to reject a reused PID.

The offline `.run` and equivalent `.tar.gz` install under `~/.local/share/fairway-xmp` (custom dedicated prefix optional), with versioned release directories and a current symlink. Reinstallation backs up initialized student databases and gracefully shuts down before switching runtimes. Bash/Zsh integration exposes Fairway/PHP/Composer; other shells can invoke the absolute path. Uninstall always preserves coursework, databases and backups. Manual updates only; no desktop updater on Linux.

## Files

- Document root: the current user's lowercase `fairway/www` folder under their home directory.
- Application data: `dev.fairway.environment` under the platform's local user data directory.
- Application data contains settings, generated service configs, logs, MySQL data, phpMyAdmin's writable overlay, PHP sessions, temporary files, Composer home, backups, and CLI launchers.
- Runtime files remain inside the installed application resources and are not mutated at startup.
- Existing coursework is never overwritten except the initial welcome page, created only if `index.php` is absent.

## Services

Default loopback ports: website 8800, phpMyAdmin 8801, MySQL 13306, internal PHP FastCGI 19000.
Nginx owns two listeners and serves phpMyAdmin; phpMyAdmin is not an independent server process.

Start all preflights all ports before launching. Ports are validated, distinct, unprivileged, and saved only while services are stopped. Conflicts suggest an available alternative and show the occupying application where available. No silent port reassignment.

The application tracks processes it starts. Nginx startup starts PHP; stopping or restarting Nginx through the panel or tray also stops or restarts its PHP worker. PHP has status and logs in the panel but no independent service controls in the panel or tray. PHP CLI remains available without running web services. Internally, stopping PHP first stops Nginx. Starting/stopping MySQL is independent. A partial Start all failure rolls back newly started services while retaining previously running ones. MySQL is shut down through authenticated mysqladmin and is never forcibly killed. A surviving MySQL instance can be recovered only when its PID and exact runtime/config command identity match this installation.

Minimizing and closing the window leave the application and services running. A native macOS menu bar / Windows system tray menu offers Show Fairway XMP, Start all, Stop all, individual Nginx/MySQL Start/Stop, Open phpMyAdmin, and Quit. On macOS, small colored dots follow the text using AppKit attributed titles with a separate font size. Windows uses small colored bitmap dots in the native icon column, avoiding monochrome emoji rendering. Native menu layout keeps the labels aligned. Nginx/MySQL show green when ready and red when stopped or not ready. Start all and Stop all use plain text without dots. Their enabled state reflects the three owned processes (PHP, Nginx, MySQL); phpMyAdmin is not counted as another process. The macOS tray Quit item displays the native Cmd+Q shortcut. phpMyAdmin shares Nginx and has no independent Start/Stop. Reopening the app restores the existing window; macOS Dock reopening does the same.

Tray and panel actions share one Rust operation guard and service manager. Native status monitoring continues while the window is hidden. Quit (including macOS Cmd+Q/application Quit) prevents new operations, waits for an existing operation, and gracefully stops all services before allowing exit. Repeated quit requests cannot bypass shutdown. If shutdown fails, the window reopens with the reason and Quit can be retried. Tray creation must succeed before the app can run with close-to-background behavior. Services do not start at login or automatically at app launch.

Status distinguishes an owned live process from a ready endpoint. Website/phpMyAdmin listeners are checked against a Fairway XMP health endpoint. Unexpected process exits surface a log hint.

## Database

Fresh initialization creates a random private root password and a local teaching account `fairway` / `fairway`, with a `fairway` database. The teaching account may create/manage student databases locally. phpMyAdmin uses cookie authentication and disallows root login.

Student connection host is explicitly `127.0.0.1` with the saved database port. Port changes update phpMyAdmin and copyable connection settings; project files are never silently edited.

Logical backups include student databases, routines, events, and triggers and exclude MySQL system schemas. Failed exports are not presented as completed backups. Existing database data and credentials are never silently reset on an error.

Before the first supported MySQL 8.4 → 9.7 startup, preserve the stopped physical database, credentials, settings and version markers under the user's backups folder. Record the completed backup before starting MySQL; failed startup retries never replace it. Record the new database family after authenticated provisioning succeeds. Reject unknown transitions and attempted downgrades. Fresh initialization records its engine version separately from account provisioning so interrupted setup can retry. Test migration against isolated alpha 1 data before release; Mac and Windows isolated migration checks have passed. Both platform installers have also been installed and tested by the release owner.

## CLI integration

Persistent user-level integration provides PHP and Composer launchers. Composer explicitly runs under bundled PHP and uses an app-owned Composer home.

Windows: add/remove only the app's exact entry in HKCU Environment Path and notify the shell. macOS: managed blocks in zsh/bash profiles with recorded locations, removed precisely while preserving unrelated edits. The UI displays inherited command resolution and explains reopening existing terminals/VS Code. Open terminal places the bundled launchers first for that session.

## Installation and removal

Windows NSIS current-user installer checks OS build, handles Visual C++ and WebView2 prerequisites, enables CLI integration, and exposes uninstall cleanup choices. macOS pkg installs the app and enables integration only for the logged-in account; a resource-bundled uninstall utility removes integration and moves the app to Trash.

Uninstall requires stopped services and preserves coursework/data by default. Deleting data and deleting coursework are separate explicit choices. Shared prerequisites are not removed. Installation/update/removal must not terminate unrelated PHP, Nginx, or MySQL processes.

## Updates

The source repository and download location are `https://github.com/eimg/fairway-xmp`. Version 0.1.0-alpha.3 is an unsigned prerelease with manual updates. It reuses Windows build 9f18e4f48de1b2672044da3b03a30c2b6f7bb601 and Linux build 6572da16a1e1ea5caaf839a77e5932e931d4064e unchanged, and repackages the tested local Mac tray app without recompiling it. These binaries retain embedded version 0.1.0-alpha.2; release notes and BUILD-INFO distinguish the release label from build versions. Source version is advanced to alpha 3 for future builds. Unsigned builds offer links to the releases page and explain manual backups and service shutdown. Alpha releases include checksums and corresponding bundled-tool source; they are not marked as the latest stable release. The default signed-update endpoint is `https://github.com/eimg/fairway-xmp/releases/latest/download/latest.json`. Release overlays configure the HTTPS endpoint and Tauri public verification key; signed artifacts are mandatory for the updater. Checks are student-triggered. Before installation, Fairway XMP starts MySQL if necessary to make a student-database backup, then gracefully stops services. Runtime and UI are updated together. The only major database transition currently implemented is 8.4 → 9.7, with a stopped physical backup. Other major transitions require a separate reviewed migration strategy.

Updates actions share equal-height buttons in a wrapping row. Long version labels and file paths must fit at the 820×650 minimum desktop window size. The document root displays the actual platform path; browser previews show manifest versions but keep service controls disabled.

Installer upgrades must also preserve data and coursework. Release testing must verify clean-machine operation without Homebrew, OS signing/notarization, Windows prerequisites, non-ASCII/space-containing user paths, uninstall, update recovery, and database compatibility. A successful compile is not proof of these checks.

## Verification

Core tests cover port validation/conflicts, path quoting, and idempotent PATH-block removal. Packaging tests reject archive traversal and bad checksums. The runtime smoke example verifies actual browser → Nginx → PHP → MySQL → response, phpMyAdmin rendering, database backup, graceful shutdown, settings persistence, and removal of isolated CLI integration without deleting coursework.

Linux CI exercises offline installation, concurrent first clients, real terminal rendering and detach, PHP/Composer, HTTP/PDO queries, backup, crash recovery, safe port changes, reinstall and uninstall with isolated paths containing spaces, apostrophes and non-ASCII characters. Clean distro containers verify bundled runtimes on Ubuntu 22.04/24.04, Debian, Fedora and Arch without installing runtime dependencies. Docker is used only for CI verification.
