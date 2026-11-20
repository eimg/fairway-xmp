# Fairway XMP 0.2.0

Release candidate preparation. The candidate workflow will build and verify installers and signed updater payloads from one source commit; this document does not claim a completed run or publication.

## Changes

- PHP 8.6.0 is available alongside patched PHP 8.2–8.5. **PHP 8.5.13 remains the default.** Switch versions in Advanced service settings with services stopped; the choice applies to websites, terminal PHP and Composer.
- Update Nginx to 1.30.7, Composer to 2.11.2 and Mailpit to 1.34.0. MySQL remains at 9.7.4 LTS and phpMyAdmin at its official 5.2.3 distribution.
- Update compatible frontend and Rust dependencies, including updater/archive and TLS verification fixes.
- Add Light, Dark and System appearance modes and Windows Fluent overlay scrollbars.
- Simplify local access controls and recover cleanly when a selected SQLite database file is missing.

## Release scope

phpMyAdmin's PHP 8.6 compatibility uncertainty is accepted for this release while PHP 8.5 remains the default. The scheduled December OpenSSL patches are planned for 0.2.1, targeted for December 2–3 once the needed packages are available. This is a local development environment.

Automated platform, runtime, installer and updater checks run in the release-candidate workflow. The release owner handles interactive visual testing separately. Candidate dispatch does not publish a release or update feed.

## Supported platforms

- Windows 10 22H2 or Windows 11, x64. The installer can show an unknown-publisher warning; desktop updates use signed payloads.
- macOS 15 or later on Apple Silicon. Distribution app and installer builds are Developer ID signed and notarized.
- Linux x86_64 with glibc 2.35 or newer. Updates use the terminal installer.
