Fairway XMP alpha 3 adds background service controls on Mac and Windows, plus a Linux terminal edition.

## Downloads

| Platform | Requirements | Download |
| --- | --- | --- |
| Windows | Windows 10 22H2 / Windows 11, x64 | [Windows installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-windows-x64-setup.exe) |
| macOS | macOS 15+, Apple Silicon | [Mac installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-macos-arm64-unsigned.pkg) |
| Linux | x86_64, glibc 2.35+ | [Terminal installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-linux-x86_64.run) · [Manual-install archive](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-linux-x86_64.tar.gz) |

[SHA-256 checksums](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/SHA256SUMS.txt) · [Build information](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/BUILD-INFO.json)

Students only need their platform’s installer. [Mac/Windows bundled-tool sources](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-bundled-tool-sources.tar.gz) and [Linux corresponding sources](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-linux-x86_64-sources.tar.gz) are supplied for redistribution and development.

## What changed

- Closing the Mac/Windows window keeps Fairway and its services running. Reopen it from the menu bar or system tray.
- Start/stop all services or Nginx/MySQL individually from the tray. Small green/red dots show each service’s status; Mac Quit displays ⌘Q.
- Nginx manages its PHP web worker automatically. PHP CLI remains available without starting web services.
- Quit waits for pending work and gracefully stops services. Failed shutdown leaves the app available for retry.
- Linux gets a TUI and CLI for service controls, logs, ports, backups, PHP, Composer and MySQL. `q` closes the TUI while services keep running; `Q` shuts down after pending operations.
- Linux installs into your home directory, bundles runtime libraries and requires no GUI, root access or systemd service. Reinstall backs up student databases; uninstall preserves projects, database files and backups.

Included tools remain PHP 8.5.11, MySQL 9.7.2 LTS, Nginx 1.30.5, Composer 2.10.3 and phpMyAdmin 5.2.3.

## Install or update

Back up your databases and **Quit** Fairway XMP before updating. Closing the desktop window leaves it running. On Linux, use `fairway shutdown`.

For Linux:

```sh
sh Fairway-XMP-0.1.0-alpha.3-linux-x86_64.run
# Reopen your shell, then:
fairway
```

Linux ARM and Alpine/musl are not supported. Sign into phpMyAdmin using **fairway / fairway**. Node.js is installed separately when needed.

Mac and Windows installers remain unsigned; platform approval may be required. Updates are manual. See the [installation guide](https://github.com/eimg/fairway-xmp#downloads).

## Build reuse and verification

This release reuses the tested Windows and Linux artifacts unchanged and repackages the existing Mac app without recompiling it. **Installed apps still report `0.1.0-alpha.2` internally; alpha 3 is the release label.** Source versions are advanced to alpha 3 for future builds.

Windows service/migration checks passed. Linux checks covered the installed TUI/CLI, shutdown during startup, concurrent clients, crash recovery, backups, reinstall and uninstall, plus clean Ubuntu 22.04/24.04, Debian, Fedora and Arch runtimes. The Mac tray app was tested by the release owner. Build information records the original sources and hashes.
