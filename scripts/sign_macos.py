#!/usr/bin/env python3
"""Sign independent Mach-O files first, then seal and verify the app bundle."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import re
import plistlib
import subprocess
import sys
from prepare_runtime import macho
from postgresql_release import verify_payload, verify_code, PIN, read

ENTITLEMENTS = Path(__file__).resolve().parents[1] / 'packaging/macos/php.entitlements.plist'


def signing_command(file, identity, app, adhoc=False):
    command = ['codesign', '--force']
    if not adhoc:
        command.extend(['--options', 'runtime', '--timestamp'])
    command.extend(['--sign', '-' if adhoc else identity])
    # Nested tools otherwise inherit the enclosing app's identifier. Native XPC
    # authenticates this exact helper identity in both directions.
    if file == app / 'Contents/Library/HelperTools/dev.fairway.environment.hosts-helper':
        command.extend(['--identifier', 'dev.fairway.environment.hosts-helper'])
    try:
        parts = file.relative_to(app / 'Contents/Resources/runtime').parts
    except ValueError:
        parts = ()
    # PHP's PCRE JIT uses MAP_JIT. Grant that permission only to the PHP
    # interpreters, never the desktop app, servers, or shared libraries.
    if (len(parts) == 3 and re.fullmatch(r'php(?:-\d+\.\d+)?', parts[0])
            and parts[1] == 'bin' and parts[2] in ('php', 'php-cgi', 'php-fpm')):
        command.extend(['--entitlements', str(ENTITLEMENTS)])
    return [*command, str(file)]


def signing_targets(app):
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    main_binary = app / 'Contents/MacOS' / info['CFBundleExecutable']
    # Signing the app seals its main executable. Sign it only at that final
    # stage, after all nested code and resources are ready.
    postgres = app / 'Contents/Resources/runtime/postgresql'
    return [file for file in app.rglob('*')
            if file != main_binary and not file.is_relative_to(postgres)
            and not file.is_symlink() and macho(file)]


def main():
    app, identity = Path(sys.argv[1]), sys.argv[2]
    uninstaller = app / 'Contents/Resources/Uninstall Fairway XMP.command'
    if uninstaller.is_file():
        uninstaller.chmod(uninstaller.stat().st_mode | 0o111)

    def sign(file):
        subprocess.run(signing_command(file, identity, app), check=True)

    # Snapshot before signing creates transient files. Symlink aliases are
    # skipped; seal the containing app after every code file has succeeded.
    files = signing_targets(app)
    postgres = app / 'Contents/Resources/runtime/postgresql'
    if postgres.exists() or postgres.is_symlink():
        # These bytes were signed before the app was compiled against their pin.
        # Re-signing them here would invalidate the compiled integrity boundary.
        match = re.fullmatch(r'Developer ID Application: .+ \(([A-Z0-9]{10})\)', identity)
        if not match:
            raise ValueError('Pinned PostgreSQL requires a Developer ID Application identity')
        verify_payload(postgres, read(PIN), bundled=True)
        verify_code(postgres, match[1])
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(sign, files))
    sign(app)
    subprocess.run(['codesign', '--verify', '--deep', '--strict', '--verbose=2', str(app)], check=True)
    if postgres.exists():
        verify_payload(postgres, read(PIN), bundled=True)


if __name__ == '__main__': main()
