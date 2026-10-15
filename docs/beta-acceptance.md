# Unsigned beta candidate and acceptance

Signing and automatic updates are deferred. PHP 8.5 is the default; PHP 8.0/8.1 remain unsupported legacy options for older coursework. Their Windows dependency build/patch provenance remains an open, explicitly recorded limitation in [release follow-ups](release-follow-ups.md). This process does not mark that work verified.

## Build a candidate

Run **Unsigned release candidate** (`release-candidate.yml`) on the intended source branch/ref. It calls the browser, Windows, Apple Silicon macOS, and Linux workflows against the **same caller commit**. It doesn't publish releases or modify versions.

Before preparing a beta release, set the chosen version consistently in `package.json`, the header and root package entry in `package-lock.json`, `src-tauri/Cargo.toml`, the `fairway` package in `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. The workflow checks these agree. Do not reuse alpha binaries under a beta filename.

Each platform runs the real service/backup-restore smoke check. Candidate source collection requires the committed source lock (`--locked`); it cannot silently accept new inputs or rewrite pins during release packaging. The desktop jobs then install the checksum-verified alpha 3 baseline in a disposable hosted account, seed a project and database, install the candidate over it, verify preservation, uninstall while keeping work, and reinstall the exact candidate. The Linux installed-CLI test covers the same upgrade/removal/reinstall sequence, plus its existing terminal and recovery checks. CI account mutation is guarded by both an explicit flag and GitHub-hosted runner checks; do not use these scripts against your own account.

Source distributions require the committed archive SHA256. For Gitiles snapshots with a committed revision and full Git tree, archive recompression is allowed only when all files, symlinks, and executable modes reproduce that exact tree. The shipped source inventory records the actual archive SHA256; candidate packaging never rewrites the committed content pins or accepts a changed source tree.

The Mac installer needs a logged-in console account for its user-profile hooks. If a hosted image has no console session, the job fails with that reason; it must not silently count package extraction as an installer pass. Use a runner with the required session, or record and investigate the infrastructure limitation before accepting a candidate. The uninstaller's `--yes-keep-data` mode keeps profile cleanup in that account and uses `sudo` only to move the application to that user's Trash when `/Applications` requires administrator access. Hosted lifecycle tests use the runner's passwordless administrator access for that move.

The final job runs only when every called workflow passes, including Linux's clean-distro matrix. It uploads:

- `candidate-manifest.json`: source commit, application/runtime versions, artifact names, exact file SHA256/size, installed lifecycle results, signing status, and known limitations.
- `acceptance-template.json`: checks still requiring a person, bound to each exact installer's hash. All checks start false.

Installers remain in their platform artifacts in the same workflow run. Mac's artifact also contains the combined Mac/Windows bundled-tool source archive. Linux's corresponding sources are in `fairway-linux-x86_64-sources`; download that artifact alongside the Linux installer artifact when verifying its file inventory.

## Review the candidate

Download the platform installers, source archives, UI report, and acceptance artifact from that run. Keep them together. Before installing, verify each downloaded directory using its `candidate-TARGET.json` from the metadata artifact:

```sh
python scripts/release_candidate.py verify-files \
  --candidate /path/to/candidate-windows-x64.json \
  --directory /path/to/downloaded/windows-artifact
```

The browser HTML report provides screenshots for normal/minimum window sizes, service/error/update scenarios, command detection, and diagnostics. Review those screenshots; a green assertion does not approve their appearance. A browser preview doesn't verify native tray menus or the Linux terminal dashboard.

For this candidate, perform a short native check on the supported target OS. Record the OS version, tester, date, and installer SHA256 in a copy of the generated acceptance template:

1. **Install and first project:** install as an ordinary user. Windows must exercise Visual C++/WebView2 prerequisites on a clean machine. Start services, run a PHP/database project, open phpMyAdmin, and verify PHP/Composer in a new terminal.
2. **Upgrade with existing work:** retain a small project, database rows, backup, ports, and PHP selection across alpha → candidate installation. Try a supported alternative PHP version and reopen the app.
3. **Uninstall and reinstall:** choose the default keep-data behavior; reinstall and open the same project/database. Confirm unrelated PHP/MySQL installations and shell settings still work.
4. **Desktop native controls:** close/reopen the window, use the tray/menu bar, and Quit while an operation is pending. Verify services shut down and no error is hidden. For Linux, review dashboard rendering, `q` detach, and `Q` shutdown instead.
5. **Desktop visual review:** inspect the same commit's browser screenshots and briefly check the installed app for native WebView differences.

Set a check to true only after it passed for the exact artifact. Preserve notes/screenshots of failures separately and rebuild after a code/runtime change; old acceptance does not apply to the new hash.

```sh
python scripts/release_candidate.py check-acceptance \
  --manifest /path/to/candidate-manifest.json \
  --evidence /path/to/completed-acceptance.json
```

This rejects mismatched commits/versions/hashes, missing tester information, and unfinished checks. It reports acceptance; it never publishes. When publishing is authorized, reuse the reviewed files unchanged, include their matching source archives and checksums, and update downloads/changelog/known limitations. Don't rebuild between acceptance and publication.

## Support reports

Desktop: **Settings → Help and diagnostics → Diagnostic report**, review the JSON, then **Copy diagnostic report**. Linux: `fairway diagnostics`. The report includes app version, source commit/modified-source flag, OS/architecture, runtime readiness/versions, selected PHP, service ports/readiness, and command-detection categories. It excludes filesystem paths, logs, project/database contents, and credentials. GUI app PATH detection can differ from a new terminal.

Desktop executables and the Linux CLI also accept `--build-info` for machine-readable version/commit/OS/architecture without launching the GUI or supervisor. Build identity is embedded at compilation; local builds with source edits are marked modified and cannot be recorded as clean release candidates.
