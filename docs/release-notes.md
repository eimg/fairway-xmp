# Fairway XMP 0.1.1

This patch fixes Laravel development servers losing Fairway's PHP configuration when started with `php artisan serve`.

- Preserve Fairway's managed `php.ini`, PHP settings and SNMP startup settings in the child server process, including automatic restarts after `.env` changes.
- Prevent repeated SNMP `Cannot find module` warnings caused by the child process losing Fairway's environment settings.
- Use PHP's standard development `variables_order=GPCS`. Host environment variables remain available through `getenv()` and `$_SERVER`; application code such as Dotenv populates `$_ENV`.

## Upgrading

Restart running Artisan development servers after updating so they load the regenerated PHP configuration. Normal `php artisan serve` commands support `.env` reloading; `--no-reload` is no longer needed as a workaround for this issue.

Projects, databases, backups, saved PHP settings and installed Node versions are preserved by normal updates. Linux users rerun the new installer. Bundled runtime versions and supported platforms are unchanged from 0.1.0.

## Requirements and limitations

- Windows 10 22H2 or Windows 11, x64; macOS 15 or later on Apple Silicon; Linux x86_64 with glibc 2.35 or newer.
- The macOS app and installer are signed and notarized. Windows installers do not have Authenticode signatures; desktop updater payloads use separate cryptographic signatures. Linux updates are manual.

See [current limitations](https://github.com/eimg/fairway-xmp/blob/main/docs/maintenance.md#current-limitations) and the [README](https://github.com/eimg/fairway-xmp#readme) for setup and platform guidance.
