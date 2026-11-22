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

Removing a PHP branch requires handling saved selections explicitly: preserve the user's choice, explain the unavailable version, and allow selecting a supported one without losing settings. Current builds support PHP 8.2–8.6; version numbers and extension requirements belong in the manifest rather than duplicated test instructions.

### PHP 8.6 and 0.2.x runtime checks

Current source builds support PHP 8.2–8.6, with patched PHP 8.5 as the default. PHP selection remains global to websites, CLI PHP and Composer; adding 8.6 does not switch existing installations. Windows 8.6 uses the official x64 NTS VS18 ZIP and the reviewed shared Visual C++ v14 prerequisite. Run the prerequisite step before runtime preparation, which executes PHP during verification:

```powershell
./tests/windows_vc_runtime.ps1
python scripts/prepare_runtime.py --windows-prerequisites
python scripts/prepare_runtime.py
```

Homebrew's latest branch uses the canonical `php` formula with a `php@8.6` alias. The downloader accepts canonical metadata only when that exact alias is advertised, and verifies the requested version. The default 8.5 branch uses its separate versioned formula. Preserve older branches' required library generations rather than replacing libraries solely by package name. Linux checks Nginx's actual version before reusing staged bytes.

Release 0.2.0 updates PHP 8.2–8.5 patches, adds 8.6.0, and updates stable Nginx, Composer and Mailpit. MySQL remains on 9.7.4. phpMyAdmin stays on its official 5.2.3 distribution; its PHP 8.6 compatibility and bundled dependency uncertainty are accepted for 0.2.0 while 8.5 remains the default. Do not describe phpMyAdmin as fully validated on 8.6 without test evidence.

Before publication, run the candidate's automated CLI/web, extension, database, mail, session/upload, installer and updater checks across all supported platforms. Sources, checksums and licenses must correspond to the actual packaged inputs. Interactive device/visual testing is handled by the release owner and is not a candidate prerequisite.

Release 0.2.1 fixes Windows CLI launcher paths and document-root separators without changing the 0.2.0 runtimes or dependencies. OpenSSL's scheduled December patches remain planned for a later patch, targeted for December 2–3, 2026 after the required upstream packages become available. The 0.2.0 and 0.2.1 runtime inputs do not include those future fixes. Maintenance notices and dependency-version matches are reviewed against this local development tool's actual usage; they do not by themselves establish an exploitable application path.

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
