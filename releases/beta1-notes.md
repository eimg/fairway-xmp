Fairway XMP beta 1 is the first release whose installed apps report `0.1.0-beta.1`. It includes PHP 8.0 through 8.5, with PHP 8.5 selected by default.

## Downloads

| Platform | Requirements | Download |
| --- | --- | --- |
| Windows | Windows 10 22H2 / Windows 11, x64 | [Windows installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-windows-x64-setup.exe) |
| macOS | macOS 15+, Apple Silicon | [Mac installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-macos-arm64-unsigned.pkg) |
| Linux | x86_64, glibc 2.35+ | [Terminal installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-linux-x86_64.run) · [Archive](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-linux-x86_64.tar.gz) |

[SHA-256 checksums](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/SHA256SUMS.txt) · [Build information](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/BUILD-INFO.json)

Students only need their platform’s installer. [Mac/Windows bundled-tool sources](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-bundled-tool-sources.tar.gz) and [Linux corresponding sources](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-linux-x86_64-sources.tar.gz) are supplied for redistribution and development.

## What changed

- PHP 8.0 through 8.5 can be selected. PHP 8.5 remains the default. PHP 8.0 and 8.1 are labeled legacy compatibility options.
- Database backups are checked by restoring them into a fresh database. Export filenames are unique, and a failed export does not leave a partial file behind.
- Settings include a shareable diagnostic report. It omits filesystem paths, logs, and credentials.
- The Mac installer quits a running Fairway XMP, stops its services, and installs into `/Applications`.
- Windows still asks you to quit Fairway XMP from the system tray before upgrading.
- Linux upgrades back up student databases and stop services before replacing the installation.

Included tools are PHP 8.5.11 (plus 8.0.30, 8.1.34, 8.2.34, 8.3.35, and 8.4.26), MySQL 9.7.2 LTS, Nginx 1.30.5, Composer 2.10.3, and phpMyAdmin 5.2.3.

## Install or update

On a Mac, run the installer even if Fairway XMP is open. It quits the app and stops services first. On Windows, quit from the system tray before installing; closing the window leaves the app running. On Linux, the installer handles the running copy:

```sh
sh fairway-xmp-0.1.0-beta.1-linux-x86_64.run
# Reopen your shell, then:
fairway
```

Your projects and databases are kept. Linux ARM and Alpine/musl are not supported. Sign into phpMyAdmin using **fairway / fairway**.

These installers are unsigned, so the operating system may ask you to confirm them. Updates are manual. This prerelease is not the latest stable release.

## Verification

The files on this page are the installers from candidate run [38699450419](https://github.com/eimg/fairway-xmp/actions/runs/38699450419), commit `f3cf84da6270038274590860db00c58eda2f1df6`. They were not rebuilt for publication. Windows, Mac, and Linux lifecycle checks passed on that run. PHP 8.0/8.1 remain legacy options, and the exact Windows build provenance for those two versions is still unverified.
