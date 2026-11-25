# Free sharing builds for friends

These are development sharing builds with manual installation and updates. Neither
builder nor recipient needs paid Apple Developer membership. They are not the
Developer ID/notarized release candidate described in the separate release guide.

## Getting the two packages

In the personal fork's GitHub **Actions**, run **Free sharing builds** on the
reviewed branch. Enable the explicit PostgreSQL source-build authorization; that
build runs only after checksum verification and only in disposable CI storage.
No Apple secrets, notarization API or Tauri updater keys are used.

Download artifacts after both jobs succeed:

- **Mac:** `fairway-xmp-macos-arm64-free-sharing-...` contains the application ZIP,
  SHA-256 checksums, build information, notices and corresponding source archives.
- **Windows:** `fairway-xmp-windows-x64-0.2.0-rc.1` contains the unsigned `.exe`
  installer and its normal verification artifacts. This uses the existing Windows
  installer workflow with `signed_release=false` and no updater configuration.

Keep the complete artifact contents available, including notices and source
archives. A GitHub Actions artifact expires; keep a copy for future downloads.
Creating a GitHub release/uploading public downloads is a separate owner action.

## Installing on an Apple Silicon Mac

Requirements: Apple Silicon (M-series), macOS 15+. Intel Macs are unsupported.
Quit any existing Fairway copy before installing or opening another one; these
builds retain the existing application identifier/data locations.

1. Extract the GitHub artifact, then extract its `macos-arm64-sharing.zip`.
2. Move `Fairway XMP.app` to Applications using Finder.
3. Open it. macOS may block the unnotarized app. If you trust the exact download,
   use **System Settings → Privacy & Security → Open Anyway**, when offered,
   then confirm opening that app. No global Gatekeeper disablement is needed.
4. Start only the services you want. Local `.test` registration uses the existing
   administrator-password flow; it does not install the Developer ID hosts helper.
5. Install a newer sharing build manually after quitting the previous copy.

Apple explains the per-app approval flow in
[Safely open apps on your Mac](https://support.apple.com/en-us/102445).
Managed Macs can restrict these approvals. Gatekeeper, quarantine, local-domain/
HTTPS approval and Keychain access require real user acceptance; headless tests do
not establish those results. No security settings are changed by the build tools.

The Mac package includes PostgreSQL 18.6, with a compile-time pin generated from
the verified ad-hoc payload. The compiled pin is recorded in BUILD-INFO.json and
postgresql-runtime-pin.json. It is never selected through a runtime environment
variable. The tracked signed-release pin remains unchanged. Existing provisioned
clusters/runtimes are never automatically replaced or adopted; a differing pin
fails closed. Keep old development profiles separate.

## Installing on Windows

Requirements: Windows 10 22H2 or Windows 11, x64. Extract the artifact and run its
`-setup.exe`. It has no Authenticode signature. Windows may show SmartScreen or an
unknown-publisher prompt; approve only your trusted download if your policy allows.
The installer includes the offline WebView2 installer configured by the project.

PHP, Nginx/MySQL, sites, mail, MySQL/SQLite browsing, terminal and optional Node
management remain available. PostgreSQL browsing/managed services and the macOS
Node Projects workspace are not implemented for Windows. Updates are manual.

## Build boundaries

The standard signed release pipeline remains unchanged and still requires its
own reviewed pin/credentials. Free sharing artifacts cannot be recorded as signed
release candidates. Mac app sealing preserves the pinned PostgreSQL bytes; it
never uses deep re-signing, Developer ID claims, notarization or a library-validation
disablement entitlement. Native tools are tested only in disposable data profiles.

Both platforms still need actual recipient installation checks. The builder must
report which CI/runtime checks passed and must not describe an unrun workflow as
a tested installer.
