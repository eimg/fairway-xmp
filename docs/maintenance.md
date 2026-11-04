# Maintenance

Work from current behavior in [PROJECT_SPEC.md](../PROJECT_SPEC.md), reproduce a problem, then update the relevant implementation, tests and guide. Git history and published releases retain prior decisions and evidence.

## Repository cleanup

Generated files are ignored by Git. Before removing them, stop any development servers, test bridges and services using those directories. Smoke-test homes/data contain disposable databases; check that a directory is a test profile before deleting it.

| Path | Cleanup guidance |
| --- | --- |
| `dist/`, `website/dist/` | Generated frontend/website output; rebuild with `npm run build` or `npm run site:build`. |
| `playwright-report/`, `test-results/` | Generated browser reports, traces and screenshots; retain any evidence still needed for a current failure or review. |
| `artifacts/` | Generated local bundles and source archives; remove obsolete copies after retaining required distribution sources and release evidence. |
| `.runtime-work/` | Runtime staging, one-off scripts/logs and isolated smoke-test profiles; remove completed work after checking no process uses it. |
| `.runtime-cache/`, `.build-tools/`, `src-tauri/target/` | Reusable downloads, toolchains and compiler caches; keep for ordinary cleanup. |
| `src-tauri/resources/runtime/` | Staged native runtime required for local desktop/service checks; keep unless deliberately restaging it. |

Keep committed runtime/source locks, `tests/fixtures/runtime-alpha1-*.json` and installer/updater probes: older versions remain active compatibility inputs. Keep release history in [CHANGELOG.md](../CHANGELOG.md), current publication notes in [release-notes.md](release-notes.md), and implementation behavior in the specification and focused guides. Do not use broad `git clean -fdx` commands for routine cleanup; they also remove useful ignored runtimes and tools.

## Runtime updates

Windows PHP ZIPs can move from the active releases directory into PHP's official archive. The downloader follows this relocation only for checksum-pinned PHP ZIPs after HTTP 404/410, retaining the original cache key and verifying the same reviewed bytes. A relocation is not a runtime update and does not refresh the lock or substitute a rebuilt ZIP.

1. Review upstream versions, checksums and security fixes. Update `runtime-manifest.json`, including the default PHP version and every affected `phpVersions` entry.
2. Update the relevant desktop/Linux download lock entries and dependency closure. Preserve unrelated pins and both platforms' entries in the shared desktop lock.
3. Restage runtimes. `--refresh-lock` can refresh unrelated components; it is not a PHP-only updater. `--resume-lock` retains existing bottle pins and does not automatically replace an outdated PHP entry.
4. Regenerate `runtime-sources.lock.json` with `scripts/package_sources.py --refresh-lock`, using the exact bottles/Windows SBOMs. Review sources, recipes, checksums and notices. Final packaging uses `--locked`.
5. Run runtime verification, focused regressions, CLI/web checks for every bundled PHP branch, and the affected platform builds. Verify Composer, required MySQL/SQLite/PostgreSQL/socket/SOAP/LDAP extensions and Unix worker support, mail, sessions and uploads as relevant. Both common and platform-specific extension requirements belong in the runtime manifest.
6. Publish a new version through the [build and release](build-and-release.md) process when authorized. Preserve published artifacts and user data.

Linux uses Ubuntu 22.04/glibc 2.35 as its build baseline and bundles component-specific library dependencies. Do not mix PHP/Nginx libraries with MySQL's private OpenSSL/Kerberos copies. Linux PHP reuse verifies both version and required extensions. Check all source packages remain available for the copied library versions.

Removing a PHP branch requires handling saved selections explicitly: preserve the user's choice, explain the unavailable version, and allow selecting a supported one without losing settings. Current builds support PHP 8.2–8.5; version numbers and extension requirements belong in the manifest rather than duplicated test instructions.

### PHP 8.6 preparation and release checks

Preparation does not add an unreleased PHP version to the manifest or advertise it in the application. The Windows packager now maps 8.2/8.3 to `vs16`, 8.4/8.5 to `vs17`, and 8.6 to `vs18`, for both the default and additional branches. Unknown branches fail until their toolset has been reviewed. Confirm the actual final ZIP name before adding 8.6.0.

Existing matching `brew-php-arm64-sequoia` pins remain reusable. When refreshing default-PHP bottles or recording new pins, staging selects the versioned formula (currently `php@8.5`), preventing Homebrew's unversioned `php` formula from silently moving the default to 8.6. A resumed preparation retains the existing bottle and its dependency pins. Review any new library requirements from the actual 8.6 bottle; do not substitute incompatible library versions into older PHP branches.

The Windows prerequisite URL is pinned alongside the other downloads. Historical upgrade fixtures retain their original installer URL/checksum. On a Windows builder, run the prerequisite step **before** ordinary runtime preparation, which executes `php.exe` during verification:

```powershell
./tests/windows_vc_runtime.ps1
python scripts/prepare_runtime.py --windows-prerequisites
python scripts/prepare_runtime.py
```

The PowerShell tests simulate missing, older, equal and newer installations, reboot requests, installer failures and false success. They do not modify the machine's runtime. The actual prerequisite step verifies the download checksum, compares installed/bundled versions, and installs only when required. NSIS independently performs the equivalent version check for users.

Once final PHP packages are available:

1. Confirm the Windows x64 NTS ZIP, PHP source archive, and `php@8.6` macOS 15 ARM64 bottle. Add `"8.6": "8.6.0"` to `phpVersions`; retain the 8.5 default unless a separate default change is intended.
2. Record new platform inputs with `--resume-lock`, preserving unrelated pins. To replace an existing pin intentionally, remove only the affected entry after saving/reviewing the old lock; resume mode does not refresh an existing Homebrew bottle. Review dependency changes, checksums and source availability. Use the normal Linux preparation workflow on its Ubuntu 22.04/glibc 2.35 builder.
3. Recheck every bundled component. Composer is prepared at 2.11.1 and Mailpit at 1.33.0; review newer stable fixes at build time. Keep MySQL on the 9.7 series and Nginx on its stable series unless a concrete requirement calls for a larger change. A different MySQL series needs explicit migration support, backup/restore and upgrade tests.
4. Treat phpMyAdmin compatibility as a release check: test login, browsing, SQL execution, import and export under 8.6 with its packaged dependencies, checking PHP logs as well as HTTP responses. Its upstream SQL parser has received PHP 8.6 support changes since the current distribution was published. Prefer an official stable phpMyAdmin update if fixes are required; a version requirement saying PHP 7.2+ is not proof of final 8.6 compatibility.
5. Regenerate the complete corresponding-source lock with `python scripts/package_sources.py --refresh-lock`, review the changed entries/notices, then verify packaging with `--locked`. Check the exact OpenSSL, curl, ICU, SQLite and PostgreSQL client libraries selected by each platform's packages.
6. Run Composer install/check-platform-reqs and all existing CLI/web, extension, database, mail, session and upload checks across every bundled branch. Verify a Windows installation with an older v14 runtime upgrades it and starts PHP 8.6. Confirm equal/newer runtimes are preserved. Update current support wording and the changelog only after these checks pass.

PHP's [published schedule](https://wiki.php.net/todo/php86) targets November 19, 2026. November 20–21 is a check window, not a promise that all required artifacts will exist. Final artifact availability and verification determine readiness; publication remains a separate authorized action.

## Node updates

`fnm` in the runtime manifest pins all three platform binaries. Update the Mac/Windows download pins and the separate Linux pin, plus `fnm-source` in both locks. Regenerate fnm’s full locked crate source plan; retain GPL notices and the offline reconstruction recipe. Keep fnm native, with no Fairway argument wrapper. Run the Mac/Windows activation smoke checks and Linux fnm smoke after changing fnm or startup integration. Preserve read-only handling for external managers and unrecorded populated stores; never infer ownership merely from an active marker.

## Current limitations

- Windows installers have no Authenticode signature. Desktop updater signatures remain enabled independently.
- Windows MySQL requires ANSI-compatible filesystem paths or existing short-name aliases. Non-UTF-8 code pages restrict MySQL CLI backup/export identifiers; Unicode row/query handling is supported. Failed protective backups prevent the related destructive action/update. Details are in [Data browser](data-browser.md#windows-backupexport-identifiers).
- Linux updates are manual. The desktop Data browser and embedded terminal are Windows/macOS features.
- PHP selection is global to the app instance. PostgreSQL PHP clients connect to a server supplied by the user. The desktop data browser has no PostgreSQL adapter, and Fairway manages no PostgreSQL server.
- Trust-anchor constraint enforcement differs between clients; encoded root constraints alone do not establish every browser's enforcement. [Security boundaries](security-boundaries.md) documents the verifier behavior and test controls.

These describe the current implementation and can be improved. They are not permanent scope restrictions or release-stage exceptions.

## Further improvements

The following are useful directions when a concrete project need justifies them, rather than prerequisites for ordinary work:

- Improve Windows CLI backup/export handling for Unicode identifiers and filesystem paths.
- Extend native permission, protected-update, WebView and clipboard coverage where automation or reproduction is practical.
- Consider shared-machine isolation: teaching-account privileges, Unix FastCGI sockets, and hosts/CA/helper ownership. Preserve existing credentials and coursework when changing these boundaries.
- Split large modules, deduplicate PHP worker logic, introduce a service enum or typed errors when doing so clarifies an actual change.
- Review Action revision pinning and toolchain/dependency updates as part of routine maintenance.
- Label LTS releases and consider recommending an LTS default while keeping all requested non-LTS majors available.
- Extend Windows discovery of custom FNM_DIR values defined only in PowerShell profiles; macOS already reads the login shell exports.

## Diagnosing native permissions

macOS helper approval, hosts writes, user-keychain trust and updater replacement use different authorization paths. Record the app's actual signature/notarization status, helper registration/approval and launchd availability before attributing a repeated approval prompt to one cause. A signed but unnotarized development copy can have different Gatekeeper behavior from a distributed app. Respect pending approval and explicit retry rather than repeatedly registering copies or automatically repairing unrelated hosts differences.

Do not broaden user-keychain trust to system-wide trust merely to change a prompt. Keep privileged helper requests bounded to Fairway's managed hosts block and certificate profile. Use [optional platform checks](testing.md#optional-platform-checks) for real prompt/focus behavior; unit tests cannot establish which authentication dialog macOS will offer.
