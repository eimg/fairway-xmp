# Fairway XMP 0.1.2

This release uses the exact tested artifacts from [candidate run 41815500921](https://github.com/eimg/fairway-xmp/actions/runs/41815500921), commit `a4de4047613d7e722172c55a77145731b7baab90`, without rebuilding. Automated installer lifecycle checks passed on all three platforms, and desktop updater lifecycle checks passed on macOS and Windows.

## Changes

- Add the management CLI on macOS and Windows, sharing service ownership with the desktop through one supervisor. Include the portable Fairway XMP agent skill in source archives and improve desktop help and guide navigation.
- Add deadlines to service commands and database dumps. Cancel protective backups promptly; explicit Quit cancels manual backups and active database-browser work before stopping services. Completed SQL statements are not undone.
- Strengthen settings and SQL/SQLite backup publication, preserve existing exports, and restore generated configuration after a failed port save.
- Retry brief Windows configuration-file sharing conflicts while preserving the previous file on persistent failure. Fetch superseded Windows PHP packages from PHP's official archive with the original pinned checksums.
- Update bundled Composer to 2.11.1 and Mailpit to 1.33.0. Upgrade older Windows Visual C++ prerequisites while preserving equal/newer installations.

## PHP 8.6 preparation

This build includes the existing PHP 8.6 packaging preparation and regression coverage, including the Windows toolset mapping and versioned Homebrew formula selection. PHP 8.6 is not bundled or offered in the application. Bundled branches remain PHP 8.2–8.5, with PHP 8.5.12 as the default. Adding final PHP 8.6 requires pinned upstream packages and the compatibility checks in [Maintenance](https://github.com/eimg/fairway-xmp/blob/v0.1.2/docs/maintenance.md#php-86-preparation-and-release-checks).

## Requirements and limitations

- Windows 10 22H2 or Windows 11, x64; macOS 15 or later on Apple Silicon; Linux x86_64 with glibc 2.35 or newer.
- The Mac app and installer are Developer ID signed and notarized by Apple. Windows installers do not have Authenticode signatures; desktop updater payloads use separate cryptographic signatures. Linux updates are manual.
- Download and run the installer to update. Update-feed promotion is separate and is not part of this publication.

See [current limitations](https://github.com/eimg/fairway-xmp/blob/v0.1.2/docs/maintenance.md#current-limitations) and [build and release](https://github.com/eimg/fairway-xmp/blob/v0.1.2/docs/build-and-release.md).
