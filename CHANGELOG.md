# Changelog

Past entries record released changes; they are not current development or release restrictions. See [Build and release](docs/build-and-release.md) for the current process.

## Unreleased

## 0.1.1 — 2026-10-29

- Preserve Fairway's PHP configuration and SNMP startup settings in Laravel development-server child processes, including restarts after `.env` changes. Restart existing Artisan servers after updating.
- Use PHP's standard development `variables_order=GPCS`; host variables remain available through `getenv()` and `$_SERVER`, while application code such as Dotenv populates `$_ENV`.

Published from [candidate run 40710498846](https://github.com/eimg/fairway-xmp/actions/runs/40710498846), commit `e3e92f05fbc946b09a1df92f1c2c499281c38e4c`, using the tested installers and updater payloads without rebuilding. Installer lifecycle checks passed on all three platforms, and desktop updater lifecycle checks passed on macOS and Windows. Bundled runtimes and supported platforms are unchanged from 0.1.0.

## 0.1.0 — 2026-10-27

- Release the first stable version for Windows x64, Apple Silicon macOS and Linux x86_64, carrying forward RC 5's features and pinned runtimes. Stable desktop builds use the stable update feed.
- On macOS, closing the window hides the Dock icon while services and the menu bar remain available. Reopening restores the window and Dock icon; minimizing keeps the Dock icon.
- Verify release configuration against both preview and stable update feeds.

Published from [candidate run 40445387643](https://github.com/eimg/fairway-xmp/actions/runs/40445387643), commit `c6bf2d086890fd4dc50f877c6ab28abb87e419c5`, using the tested installers and updater payloads without rebuilding. macOS and Windows clients receive 0.1.0 through both stable and preview update feeds; Linux updates remain manual.

## 0.1.0-rc.5 — 2026-10-26

- Bundle native fnm 1.39.0 on all platforms and add optional Node.js management to desktop Settings. Preserve external fnm ownership, installed versions and global npm packages, with explicit activation handover and no automatic project switching.

- Make Node selection and activation transactional, restoring the previous fnm default after failed setup. Add cancellable Node installations with progress that survives Settings navigation, and browser coverage for Node management and recovery. Repair Linux candidate lint configuration and the notification test bridge.

- Import user tool environments in new desktop terminals, repair Windows Node activation with Unicode and canonical storage paths, and show clearer desktop update progress with expandable release notes.

- Load supported remote images automatically in Mail previews, with Retry image placeholders for failures. Preserve downloaded images across HTML/Text switches and keep the preview’s restrictive security policies through a bounded, cookie-free native image loader.

Published from [candidate run 40288383232](https://github.com/eimg/fairway-xmp/actions/runs/40288383232), commit `d8646771670400591b2a5af743aa4b83d6a491da`, using the tested installers and updater payloads without rebuilding. Installed macOS and Windows clients receive RC 5 through the preview update feed; Linux updates remain manual.

## 0.1.0-rc.4 — 2026-10-24

- Add per-version PHP settings for memory, execution time, upload size, timezone, displayed errors and session upload progress. Keep overrides across app/runtime updates, coordinate PHP/Nginx upload limits and response timeouts, and provide Reset to defaults. Upload progress remains enabled by default.

- Update bundled Nginx to 1.30.6, MySQL 9.7 LTS to 9.7.4 and Mailpit to 1.32.0, including their available security fixes. Refresh platform download pins and corresponding source inputs. PHP remains on 8.2.35, 8.3.36, 8.4.27 and 8.5.12; session upload progress stays enabled by default.

- Enable sockets, SOAP and LDAP across bundled PHP versions on Windows, macOS and Linux, and require PCNTL/POSIX for Unix CLI workers. Add platform-aware runtime verification and CLI/web extension smoke checks. Redis projects use project-installed Predis and a user-supplied server.

- Consolidate current development, testing, release and maintenance guidance. Remove completed prerelease investigations, device acceptance tooling and one-off release workflows; retain installed-client update channels and automated upgrade fixtures.

- Successful automated candidates are ready for owner-authorized publication. Remove mandatory human device acceptance from publishing; device checks are optional guidance rather than release tooling. Publication still checks exact candidate identity, artifacts and updater signatures, and feed promotion remains explicit.

- Enable PostgreSQL client support (`pdo_pgsql` and `pgsql`) in every bundled PHP version on Windows, macOS and Linux. Applications can connect to a PostgreSQL server supplied by the user. This pass includes client support; PostgreSQL server packaging, service controls and Data browser integration remain future work. Runtime verification requires both extensions; candidate builds exercise CLI and Nginx/PHP connections against disposable PostgreSQL servers.

Published from [candidate run 40053605436](https://github.com/eimg/fairway-xmp/actions/runs/40053605436), commit `f547bcff4683f06b56e1a6d2749656a99e6db8c2`, using the tested installers and updater payloads without rebuilding. Installed macOS and Windows clients receive RC 4 through the preview update feed; Linux updates remain manual.

## 0.1.0-rc.3 — 2026-10-22

- View row is a dedicated reader with full wrapped text, field metadata, distinct NULL/empty values, per-field copy and exact JSON copying.

- Terminal tabs and actions share one compact toolbar. Exiting the first terminal opens a fresh shell; exiting the extra terminal closes its tab. The prompt shows a green arrow and current folder (full path on Windows). External terminal opens the current folder without injecting scripts, commands or environment overrides; CLI availability follows Settings.
- Terminal, Logs, Mail and Data browser share an in-window maximize mode with isolated input, nested dialogs and focus restoration. Terminal fitting keeps the final input line visible.
- Logs adds Clear view and Open log folder, consolidates controls, and keeps recurring service failures visible after recovery. Page descriptions are shorter and functional.
- MySQL adds confirmed SQL-file import with a mandatory target backup, visible phases, cancellation and partial-failure recovery. Open backups stays available in the footer. SQLite remains file/backup-only.
- SQL results name inserts, deletes and updates. MySQL updates count changed rows; SQLite explicitly reports matched rows. MySQL remembers the selected database/table in the current app session, and nullable row fields accept typing directly.
- Fast TypeScript unit tests now run in verification and candidate builds. Native regressions cover unchanged updates, import cancellation/phases and external-terminal path resolution.
- PHP 8.0 and 8.1 are no longer bundled on Windows, macOS, or Linux. PHP 8.2–8.5 remain, with 8.5 as the default. A saved 8.0 or 8.1 selection is preserved and blocks service startup until the user chooses 8.2 or later.

- Create table and Structure offer copyable foreign-key SQL examples. Structure opens Table SQL in a copyable modal and distinguishes outgoing references from tables referencing this one. Redundant MySQL CLI examples are removed.
- Mail previews resolve inline attachment images and open HTTP/HTTPS links in the external browser. Email scripts and forms remain blocked; preview keyboard focus works inside maximized Mail.
- Removed PHP selections can be replaced through desktop Settings or the Linux PHP-version command. Failed saves preserve the previous selection and configuration. Runtime verification rejects stale manifests and leftover PHP directories.

- Update bundled PHP to 8.2.35, 8.3.36, 8.4.27 and 8.5.12, fixing CVE-2026-109618 and CVE-2026-109619 across Windows, macOS and Linux. PHP 8.5 remains the default.

Published from [candidate run 42](https://github.com/eimg/fairway-xmp/actions/runs/39710252813), source `809547ab1b71143faedc39ca6c0492aa46be3a13`, using the exact accepted artifacts without rebuilding. The preview update feed offers RC 3 to installed macOS and Windows beta and RC clients; Linux updates remain manual. The owner confirmed pre-publication device checks. Actual in-app update/restart checks remain pending.

## 0.1.0-rc.2 — 2026-10-20

Published from [candidate run 36](https://github.com/eimg/fairway-xmp/actions/runs/39366414344), source `3ace851c69096a709549ef540955db4b630358dc`. The preview update feed offers this release to installed macOS and Windows beta and RC clients.

- Mac candidates include native login-keychain HTTPS trust installation/removal and the signed background address helper, with packaged-feature/payload checks before distribution. Authorization focus handoffs and concise site help are included; actual dialog/helper upgrade acceptance remains required.
- A simpler welcome page for newly created document roots; existing or customized `index.php` files are preserved.
- Protect SQLite unique indexes backing foreign keys even when referenced columns are reordered.
- MySQL SQL runs reject unfinished transactions and report rollback. The ten-result display cap now drains all statements, reports omitted results, and preserves later errors instead of emitting a late execution-limit error.
- A broken view or unreadable table no longer prevents opening the database catalog, choosing another table, or using SQL to repair it.

- MySQL/SQLite Structure now shows indexes and incoming/outgoing foreign keys. Create ordinary or composite unique indexes with ordered columns; remove supported indexes with confirmation and a protective backup. Primary, constraint-owned, advanced and relationship-dependent definitions stay protected in forms. Foreign-key inspection is read-only; filtering stays in SQL.

- SQLite project files in the desktop Data browser: a visible Open SQLite file action, recent paths, shared table/row editing and SQL workspace, and consistent file backups without starting services or installing an editor extension.
- Data browser success and error messages stay on screen. Long messages scroll inside the notice, without moving the page or keyboard focus.
- Mail, terminal, and log action results use that same fixed notice. Dialog validation stays in its dialog and scrolls there. Service status, missing runtimes, an unreadable site list, inbox loading, and log-file failures stay on the page.
- Data browser row and column actions are icons. Every table row can be viewed in full, including when edit and delete are unavailable. An empty database offers a labeled Create table action beside Open SQL query.
- MySQL and SQLite are data-source tabs. Both stay available: choosing MySQL while the service is stopped shows Start MySQL, and choosing SQLite before a file is chosen shows Open SQLite file. While a file is open, its name, path, immediate-save warning, Back up SQLite file, and Close SQLite file sit in one card under those tabs. Close releases the file without deleting it, writing it, or opening the picker. Remembered paths stay, and that session does not reopen the file until it is chosen.
- SQLite-specific schema controls, safe row identity and conflict checks, guarded destructive schema changes, and native/browser regression coverage. Advanced column redesign remains in SQL; `fairway db` stays MySQL-only.
- Main pages fill the content width beside the sidebar, with the same horizontal padding Mail and Logs already use.
- The main sidebar can fold to icons from the workspace breadcrumb. Unfold restores the labels. Show and hide tables is the same kind of icon, placed in front of the table name.
- Table rows use the height left in the window. A new MySQL column starts required, and a new SQLite column still allows NULL. The expanded terminal lines up with a folded sidebar.

Installed macOS and Windows clients receive this release through the preview update feed. Linux updates stay manual.

## 0.1.0-rc.1 — 2026-10-19

- Built-in Mailpit catcher and in-app inbox, PHP mail integration, SMTP settings and attachment saving.
- Safe migration of existing ports and native Mail Start/Stop tray controls.
- Grouped sidebar with Environment, Terminal and Logs as core services, plus Manage Sites, View Mail and Browse Data.
- Searchable troubleshooting Logs with service shortcuts, site request filters alongside shared PHP output, readable Unicode requests, and bounded background reads and display.
- Reliability fixes for backup names and empty-database upgrades, mail response races and incomplete downloads, special-character/long paths, and bounded CLI log reading.
- Explicit Windows MySQL backup/export errors for unsupported non-ASCII identifiers; updates stop when their database backup cannot complete. Nginx supports Unicode and short-alias Windows data paths.
- RC/stable-aware release publication, exact-installer human acceptance gates, and separate stable/preview update endpoints.

Feature scope locked on 2026-10-18. Per-site PHP is deferred; all sites use the selected global PHP version. Windows PHP 8.0/8.1 dependency provenance remains unverified and the Windows installer remains without Authenticode, accepted as disclosed RC limitations.

## 0.1.0-beta.3 — 2026-10-17

- Developer ID signing, Apple notarization and stapling for the Mac app and installer.
- In-app beta updates on Mac and Windows, with version-bound signatures, download progress, verification before service shutdown, database backup, safe installer handoff, and enabled CLI launcher refresh.
- A fixed beta feed promoted after verified versioned assets are published; native updater rejection/reinstall drills and exact artifact hashes gate candidate recording.
- Saved Sites with optional local domains and HTTPS, clearer readiness/recovery, folder and terminal actions, and project workflow improvements.

Beta 2 and earlier require one manual installation of beta 3. Windows Authenticode is deferred; Linux keeps manual updates. Current release guidance is in [Build and release](docs/build-and-release.md).

## 0.1.0-beta.2 — 2026-10-16

- Embedded Terminal on macOS and Windows: up to two local shells in coursework or project folders, with PHP and Composer available for that session even when persistent command-line integration is off.
- Independent terminal tabs, project-folder selection, Ctrl+backtick access, private optional macOS command history, and discovery of common installed tool locations.
- Terminal output search, saved text size, expanded view, recoverable resizing, and access to existing sessions during service operations. Routine CI now includes terminal tests; installer workflows exercise the native browser-to-PTY flow.
- macOS and Windows data browser beside phpMyAdmin, with database and table management, row editing, and a SQL editor. Linux is unchanged.
- Desktop command-line tools include `fairway db` for SQL export and import.
- Both desktop candidate builds exercise real database CRUD, SQL export/import and cancellation, and browser input through native database/terminal bridges. Windows database workers and export cleanup handle native process and file semantics. Windows terminals normalize local drive paths, complete the cursor handshake during early shutdown, and close ConPTY after shell exit while preserving final output. Windows MySQL uses existing short-name aliases for non-ANSI paths without moving data.

This release reports `0.1.0-beta.2` and reuses the tested installers from candidate run 38848919567, commit `764bccae0287af74cca0382359ab3de7853c5f48`, without rebuilding. Installers are unsigned and updates are manual. The release owner authorized publication; individual human acceptance checks remain unrecorded.

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
