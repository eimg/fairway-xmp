# Personal fork release preparation

Target: `RizzRioo06/fairway-xmp`, candidate version **0.2.0-rc.1**.
Prepared on `feature/postgresql-integration`; the owner authorized committing,
merging and pushing this work to the personal fork's `main` on 2026-11-25.
No release has been built,
signed, notarized or published by this work. The owner confirmed that release
credentials are not configured. Teacher upstream remains unchanged.

## Implemented features

- Existing PHP version/settings controls, Nginx/MySQL, local sites/HTTPS, mail,
  MySQL/SQLite browsing, backups, terminal and optional fnm/Node management.
- Existing macOS Node Projects workspace: explicit project registration, selected
  Node/npm runtime and script, owned start/stop, bounded logs and local URLs.
  The separate `feature/node-project-templates` branch is **not** included.
- PostgreSQL workspace on macOS arm64: managed status and explicit lifecycle
  actions, external loopback sessions, database/schema/table/view browsing,
  columns/indexes/view definitions, pagination, sorting and bound filters.
- PostgreSQL SQL editor: read-only default, exact-statement modifying SQL consent,
  cancellation, bounded results, exact text/NULL values and uncertain-commit
  reporting. External passwords and SQL remain in memory. External servers are
  never owned or started/stopped by Fairway.
- Private PostgreSQL client/server payload 18.6. External 17.x and exactly 18.6
  are accepted; managed clusters require exactly 18.6. Real external evidence
  covers 17.12, not every accepted patch. Remote/TLS connections, PostgreSQL role
  administration, backups, cluster adoption/upgrades and Windows/Linux PostgreSQL
  execution are unavailable.

## Release code prepared

Updater defaults, metadata and in-app source/help links target the personal fork.
Publication and feed promotion reject other repositories. Preflight refuses the
inherited teacher updater key and Apple identity. License/copyright notices remain.

`postgresql-runtime.yml` builds checksum-pinned sources on a credential-free runner.
Its separate signing job signs a new private copy without executing payload tools,
verifies private library dependencies, then computes the final manifest/pin. It
never changes the application pin automatically. Temporary Apple credentials are
removed on success or failure.

After review and commitment of that pin, the macOS candidate downloads the selected
artifact, checks its exact tree, source archive, notices, source recipe, signatures
and pin, and stages it exclusively **before** compiling the app. Final app signing
preserves these PostgreSQL bytes. Installer-owned bundle resources are readable;
managed copies keep their private ownership, inode, hash, link and no-overwrite
checks. No installed app, personal runtime or existing cluster is changed.

CI runs isolated PostgreSQL lifecycle/query/cancellation fixtures before packaging
and again using the final signed app's private workers after credential cleanup.
These fixtures use disposable clusters and a memory vault; they do not establish
real signed-app Keychain acceptance. Source archives include PostgreSQL's source,
notices, preparation recipe, manifest and pin.

## Owner credential setup

Set these GitHub Actions **repository variables** in the personal fork:

| Variable | Value |
| --- | --- |
| `FAIRWAY_APPLE_TEAM_ID` | Your own ten-character Apple Developer team ID |
| `APPLE_SIGNING_IDENTITY` | Full `Developer ID Application: ... (TEAMID)` identity |
| `APPLE_INSTALLER_IDENTITY` | Full `Developer ID Installer: ... (TEAMID)` identity |
| `FAIRWAY_POSTGRES_RUNTIME_RUN` | Reviewed signed runtime workflow run ID, after the runtime step below |

Set these **repository secrets**, keeping private values out of chat, source,
logs and artifacts:

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATES_P12` | Base64 certificate export containing your Application/Installer certificates and private keys |
| `APPLE_CERTIFICATES_PASSWORD` | Certificate export password |
| `APPLE_API_PRIVATE_KEY` | Apple notarization API private key contents |
| `APPLE_API_KEY` | Notarization key ID |
| `APPLE_API_ISSUER` | Notarization issuer ID |
| `TAURI_SIGNING_PRIVATE_KEY` | Your fork's Tauri updater signing key |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password, if used |

Use the locked Tauri CLI's `signer generate` command and help to generate/store
your updater key in a secure location outside this repository. Replace only
`packaging/updater.pub` with the resulting **public** key and commit it. The current
file intentionally remains inherited and fails release preflight. No private key
was generated here. Teacher installations will not automatically trust your key or
follow your fork's feeds. App identifiers/data locations remain unchanged; avoid
running both copies against the same profile.

## Build and publish sequence

1. Review/commit this branch. Signing requires commits already in fetched
   `origin/main`; obtain owner permission before merging into the personal fork's
   `main`. Never merge/push to teacher upstream.
2. Configure your Apple variables/secrets. Run **Prepare signed PostgreSQL runtime**
   from reviewed `origin/main`. This workflow explicitly authorizes its
   checksum-verified source build. Download the `postgresql-signed` artifact.
3. Review its payload, provenance and `runtime-pin.json`. Commit that exact pin to
   `src-tauri/src/postgresql/managed/runtime-pin.json` through the normal reviewed
   branch/main process. Set `FAIRWAY_POSTGRES_RUNTIME_RUN` to the successful run ID.
   Retain an immutable copy; Actions artifacts expire after 90 days. A replacement
   needs a newly reviewed pin/build. Never overwrite an existing development
   runtime or cluster to make a new pin pass.
4. Run **Release candidate** at the final commit. All Windows/macOS/Linux checks
   must pass. An unsigned branch installer is development evidence, not this
   signed candidate. Local results do not substitute for these jobs.
5. Qualify final signed managed PostgreSQL/Keychain behavior in a disposable macOS
   account using the existing native acceptance guides. Record actual OS versions;
   a macOS 15 deployment target alone is not execution evidence. The user performs
   GUI checks; do not automate the Mac GUI.
6. Review candidate artifacts and notes. With explicit owner authorization, create
   the version tag and run **Publish tested release** with the successful candidate
   run ID and source commit. Update-feed promotion is a separate explicit choice.
   Publish exact tested files without rebuilding during publication.

See [Build and release](build-and-release.md),
[runtime integrity](postgresql-runtime.md), and
[native workspace evidence](postgresql-native-acceptance.md).

## Local verification — 2026-11-25

- Production frontend build and 32 TypeScript unit tests passed.
- Full Python regression milestone: 96 passed. Changed signing, publishing, feed
  and PostgreSQL release suites were rerun after final edits; the later-added
  archive/publication and workflow-boundary tests also passed.
- Headless Rust regression suite: 177 passed, nine opt-ins ignored. Both PostgreSQL
  opt-ins were subsequently run: all 25 PostgreSQL tests passed with a fresh copied
  payload using installer-readable modes, disposable clusters and a memory vault.
  Seven unrelated Node opt-ins were not run. No login Keychain was used.
- Chromium/WebKit at normal/minimum sizes: 24 simulated PostgreSQL tests passed,
  including Windows unavailability without PostgreSQL IPC. These are browser
  simulations, not native Mac WebView or real authentication evidence.
- Strict all-target Rust Clippy, formatting, source pin verification, changed
  documentation links, shell/Python syntax and Git whitespace checks passed.
  Clippy used the documented debug resource-copy override after Tauri's redundant
  copy hit existing read-only resource files; runtime integrity was not bypassed.
- Workflow YAML parsed and signing-boundary fixtures passed. Real Actions jobs,
  Developer ID/notarization, signed-app workers, Keychain, installers/updates and
  Windows/Linux runtime execution were not performed in this session.
- Browser socket startup initially hit the sandbox's loopback restriction and
  passed when rerun with local fixture access. Release preflight intentionally
  fails on the inherited updater public key; it does not claim release readiness.
