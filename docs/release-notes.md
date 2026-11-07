# Fairway XMP 0.1.3

This release uses the exact artifacts from [candidate run 42340364750](https://github.com/eimg/fairway-xmp/actions/runs/42340364750) at commit `d95418492eb998f4ae0c7bac090b9421b16cb825`. Automated installer lifecycle checks passed on all three platforms, and the signed updater checks passed on macOS and Windows.

## Changes

- On macOS and Windows, turn on **Run on start** for Nginx, MySQL or Mail to start that service when Fairway XMP opens. Nginx also starts its PHP worker. Start, Stop and Quit preserve those choices.
- Use **Settings → Launch Fairway XMP at login** to open the app in the background after sign-in. The setting defaults to off and is independent of the service switches. On macOS, Fairway XMP links to System Settings when approval is needed.
- Start, stop, restart and view logs from the Environment cards. The service actions use compact icons with tooltips and accessible labels.
- Startup recovery clears the loading message when a blocking error needs attention.

## Install or update

- Windows 10 22H2 or Windows 11, x64. The installer can show an unknown-publisher warning. Desktop updates use signed payloads.
- macOS 15 or later on Apple Silicon. The app and installer are Developer ID signed and notarized.
- Linux x86_64 with glibc 2.35 or newer. Download and rerun the installer to update the terminal edition.

On macOS and Windows, open **Updates → Check for updates**. See the [how-to guides](https://eimg.github.io/fairway-xmp/) and [current limitations](https://github.com/eimg/fairway-xmp/blob/v0.1.3/docs/maintenance.md#current-limitations).
