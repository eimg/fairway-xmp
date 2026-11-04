# Fairway XMP 0.1.2 — draft build notes

This is an unpublished production build candidate. Publication and update-feed promotion are separate actions.

## Changes

- Add the management CLI on macOS and Windows, sharing service ownership with the desktop through one supervisor. Include the portable Fairway XMP agent skill in source archives and improve desktop help and guide navigation.
- Add deadlines to service commands and database dumps. Cancel protective backups promptly; explicit Quit cancels manual backups and active database-browser work before stopping services. Completed SQL statements are not undone.
- Strengthen settings and SQL/SQLite backup publication, preserve existing exports, and restore generated configuration after a failed port save.
- Update bundled Composer to 2.11.1 and Mailpit to 1.33.0. Upgrade older Windows Visual C++ prerequisites while preserving equal/newer installations.

## PHP 8.6 preparation

This build includes the existing PHP 8.6 packaging preparation and regression coverage, including the Windows toolset mapping and versioned Homebrew formula selection. PHP 8.6 is not bundled or offered in the application. Bundled branches remain PHP 8.2–8.5, with PHP 8.5.12 as the default. Adding final PHP 8.6 requires pinned upstream packages and the compatibility checks in [Maintenance](maintenance.md#php-86-preparation-and-release-checks).

## Requirements and limitations

- Windows 10 22H2 or Windows 11, x64; macOS 15 or later on Apple Silicon; Linux x86_64 with glibc 2.35 or newer.
- Candidate builds must pass their signing, installer, updater and runtime checks before they are considered ready for publication. Windows installers do not have Authenticode signatures; desktop updater payloads use separate cryptographic signatures. Linux updates are manual.

See [current limitations](maintenance.md#current-limitations) and [build and release](build-and-release.md).
