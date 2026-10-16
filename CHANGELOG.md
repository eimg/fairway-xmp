# Changelog

## 0.1.0-beta.2 — candidate

- Embedded Terminal on macOS and Windows: up to two local shells in coursework or project folders, with PHP and Composer available for that session even when persistent command-line integration is off.
- Independent terminal tabs, project-folder selection, Ctrl+backtick access, private optional macOS command history, and discovery of common installed tool locations.
- Terminal output search, saved text size, expanded view, recoverable resizing, and access to existing sessions during service operations. Routine CI now includes terminal tests; installer workflows exercise the native browser-to-PTY flow.
- macOS and Windows data browser beside phpMyAdmin, with database and table management, row editing, and a SQL editor. Linux is unchanged.
- Desktop command-line tools include `fairway db` for SQL export and import.
- Both desktop candidate builds exercise real database CRUD, SQL export/import and cancellation, and browser input through native database/terminal bridges. Windows database workers and export cleanup handle native process and file semantics. Windows terminals normalize local drive paths, complete the cursor handshake during early shutdown, and close ConPTY after shell exit while preserving final output. Windows MySQL uses existing short-name aliases for non-ANSI paths without moving data.

This build reports `0.1.0-beta.2`. Installers are unsigned and updates are manual. It is the beta 2 candidate and is not a published release.

## 0.1.0-beta.1 — 2026-10-15

- Full backup restore verification against a fresh database, unique export filenames, and cleanup after export errors.
- Shareable diagnostic reports with embedded build identity and no filesystem paths, logs, or credentials.
- PHP 8.0/8.1 labeled as unsupported legacy options.
- An unsigned release-candidate workflow tying platform installers, installed upgrade/removal/reinstall checks, browser reports, and human acceptance to one commit and exact file hashes. Signing remains deferred.
- Mac keep-data uninstall handles protected application moves; source collection verifies repacked Gitiles archives against their committed source trees.
- The Mac installer quits a running Fairway XMP and stops its services before replacing the app.

This release is built as `0.1.0-beta.1` and published from candidate run 38699450419 without rebuilding. The installers are unsigned and updates are manual. PHP 8.0 and 8.1 are legacy options; their Windows dependency provenance remains unverified.

## 0.1.0-alpha.3 — 2026-10-14

- Linux x86_64 terminal dashboard and CLI, with a detached user supervisor and offline `.run`/`.tar.gz` installers for glibc 2.35+ systems.
- Linux bundled runtime libraries, crash recovery, database backups before installer upgrades, and uninstall that preserves projects/data.
- Native macOS menu bar and Windows system tray controls for all services and individual Nginx/MySQL services, red/green status dots, reopening the panel, phpMyAdmin, and Quit.
- PHP web workers are managed with Nginx; the PHP panel card shows status and logs without separate Start/Stop controls.
- Closing the window keeps services running. Quit waits for pending work and graceful shutdown; failures reopen the panel with an error.

This release reuses the tested Windows/Linux builds and packages the existing Mac app without recompiling it. Embedded app versions remain `0.1.0-alpha.2`; alpha 3 identifies this release.

## 0.1.0-alpha.2 — 2026-10-14

- PHP 8.5.11, MySQL 9.7.2 LTS, and stable Nginx 1.30.5.
- Aligned Updates buttons, clearer project paths, and improved layouts for smaller windows and long version labels.
- A saved copy of your stopped MySQL 8.4 database before its first upgrade to 9.7.

Composer 2.10.3 and phpMyAdmin 5.2.3 remain current. Both installers are unsigned, and updates are installed manually. Back up your databases, stop services, and quit Fairway XMP before updating.

## 0.1.0-alpha.1 — 2026-10-13

First public alpha for Windows x64 and Apple Silicon Macs.

- PHP 8.4, Composer, Nginx, MySQL 8.4 LTS, and phpMyAdmin in one control panel.
- Start, stop, and restart services; change ports and view service logs.
- PHP and Composer in your user PATH, with cleanup on uninstall.
- Projects in `fairway/www`, database backups, and local-only connections.
- Fairway XMP branding and green app, setup, and uninstall icons.

The alpha installers are unsigned and updates are installed manually. Back up important databases before testing. See the [README](README.md) for downloads and installation instructions.
