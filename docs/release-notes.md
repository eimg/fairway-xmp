# Fairway XMP 0.2.1

This patch fixes Windows command-line launchers and the shared document-root path display.

## Changes

- Fix PHP and Composer commands reporting **The system cannot find the path specified** after enabling command-line tools. Normalize generated Windows launcher paths so commands work in ordinary terminals, including after Fairway XMP is closed.
- Apply the same path handling to the `fairway` management launcher. Preserve literal exclamation marks in paths and command exit status.
- Show the document root with consistent Windows separators, such as `C:\Users\name\fairway\www`, across Environment, Settings and Terminal.

## Release scope

Bundled PHP versions, development tools and dependencies are unchanged from 0.2.0. PHP 8.5 remains the default. Scheduled December OpenSSL maintenance is planned separately for a later patch after the required upstream packages become available.

Windows Authenticode signing remains deferred; desktop updater payloads retain their existing cryptographic signatures. Mac distribution builds remain Developer ID signed and notarized. Linux updates remain manual.

## Verification

The Windows fixes passed [test build 45010512779](https://github.com/eimg/fairway-xmp/actions/runs/45010512779), commit `4b2c0d26ebf1436937ce23199ae98a409516781b`, including native CLI and installed upgrade/reinstall checks. The release owner confirmed both reported issues are resolved on the affected Windows computer.

Full platform, runtime and installer checks passed in [candidate run 45019828647](https://github.com/eimg/fairway-xmp/actions/runs/45019828647), commit `10703b73bcc1e4bd9b6dc56e0a16237aafd29213`. Desktop updater lifecycle checks passed on macOS and Windows. Publication uses these exact tested artifacts without rebuilding.

## Supported platforms

- Windows 10 22H2 or Windows 11, x64.
- macOS 15 or later on Apple Silicon.
- Linux x86_64 with glibc 2.35 or newer.
