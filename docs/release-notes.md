# Fairway XMP 0.2.0

PHP 8.6 is now available alongside patched PHP 8.2–8.5, with PHP 8.5 remaining the default. This release also includes appearance choices and updates to the bundled development tools.

## Changes

- PHP 8.6.0 is available alongside patched PHP 8.2–8.5. **PHP 8.5.13 remains the default.** Switch versions in Advanced service settings with services stopped; the choice applies to websites, terminal PHP and Composer.
- Update Nginx to 1.30.7, Composer to 2.11.2 and Mailpit to 1.34.0. MySQL remains at 9.7.4 LTS and phpMyAdmin at its official 5.2.3 distribution.
- Keep session upload progress Off by default for patched PHP; phpMyAdmin remains compatible when a project opts in. Regular file uploads remain available.
- Update compatible frontend and Rust dependencies, including updater/archive and TLS verification fixes.
- Add Light and Dark appearance modes and Windows Fluent overlay scrollbars.
- Simplify local access controls and recover cleanly when a selected SQLite database file is missing.

## Release scope

phpMyAdmin's PHP 8.6 compatibility uncertainty is accepted for this release while PHP 8.5 remains the default. The scheduled December OpenSSL patches are planned for 0.2.1, targeted for December 2–3 once the needed packages are available. This is a local development environment.

Automated platform, runtime, installer and updater checks passed in [candidate run 44859372902](https://github.com/eimg/fairway-xmp/actions/runs/44859372902), commit `3ea8998af8d2ddafb0ad4040e1380fada4ac4035`. Installers and signed updater payloads are published from those tested artifacts without rebuilding. The release owner handles interactive visual review separately.

## Supported platforms

- Windows 10 22H2 or Windows 11, x64. The installer can show an unknown-publisher warning; desktop updates use signed payloads.
- macOS 15 or later on Apple Silicon. Distribution app and installer builds are Developer ID signed and notarized.
- Linux x86_64 with glibc 2.35 or newer. Updates use the terminal installer.
