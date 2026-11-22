#!/usr/bin/env python3
"""Install, upgrade, remove and reinstall exact artifacts on disposable hosted runners."""
import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def run(args, **kwargs):
    return subprocess.run(args, check=True, timeout=600, **kwargs)


def wait_removed(path):
    deadline = time.monotonic() + 60
    while path.exists() and time.monotonic() < deadline:
        time.sleep(.2)
    if path.exists(): raise RuntimeError('Uninstaller did not remove the installed executable')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--installer', required=True, type=Path)
    parser.add_argument('--baseline', required=True, type=Path)
    parser.add_argument('--probe', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--updater', type=Path)
    parser.add_argument('--update-probe', type=Path)
    parser.add_argument('--allow-user-profile-changes', action='store_true')
    args = parser.parse_args()
    if not args.allow_user_profile_changes or os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted':
        parser.error('This test modifies the current account; explicitly allow it on a disposable GitHub-hosted runner only')
    windows = sys.platform == 'win32'
    if not windows and sys.platform != 'darwin': parser.error('Use linux_cli_smoke.py for Linux')
    home = Path.home()
    data = Path(os.environ['LOCALAPPDATA']) / 'dev.fairway.environment' if windows else home / 'Library/Application Support/dev.fairway.environment'
    if data.exists() or (home / 'fairway').exists():
        raise RuntimeError('Refusing to modify an account with existing Fairway data/coursework')
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=True)
    with args.installer.open('rb') as stream:
        installer_sha256 = hashlib.file_digest(stream, 'sha256').hexdigest()
    state = output / 'installed-state.json'
    if windows:
        installation = Path(os.environ['RUNNER_TEMP']) / 'Fairway XMP installed test'
        exe = installation / 'fairway.exe'; runtime = installation / 'runtime'
    else:
        installation = Path('/Applications/Fairway XMP.app')
        exe = installation / 'Contents/MacOS/fairway'; runtime = installation / 'Contents/Resources/runtime'
        console_user = subprocess.check_output(['stat', '-f', '%Su', '/dev/console'], text=True).strip()
        if console_user != getpass.getuser(): raise RuntimeError('Installer hooks require a logged-in runner account; no console user is available')
    if installation.exists(): raise RuntimeError('Refusing to replace an existing installation')

    def install(path):
        if windows:
            # NSIS /D must be last and unquoted, including when the directory contains spaces.
            run(f'"{path.resolve()}" /S /D={installation}')
        else: run(['sudo', 'installer', '-pkg', str(path.resolve()), '-target', '/'])
        if not exe.is_file(): raise RuntimeError('Installed executable is missing')

    def probe(mode):
        run([str(args.probe.resolve()), mode, str(runtime), str(state)], env={**os.environ, 'FAIRWAY_INSTALLER_TEST': '1'})

    def uninstall():
        if windows:
            uninstallers = list(installation.glob('uninstall*.exe'))
            if len(uninstallers) != 1: raise RuntimeError('Expected one installed uninstaller')
            run([str(uninstallers[0]), '/S'])
        else:
            run(['sh', str(installation / 'Contents/Resources/Uninstall Fairway XMP.command'), '--yes-keep-data'])
        wait_removed(exe)

    completed = []
    candidate_installed = False
    try:
        install(args.baseline); probe('seed'); completed.append('baseline-install-and-real-project')
        install(args.installer); candidate_installed = True
        probe('verify'); completed.append('upgrade-preserves-database-settings-backup-coursework')
        build = json.loads(subprocess.check_output([str(exe), '--build-info'], text=True))
        help_text = subprocess.check_output([str(exe), 'db', '--help'], text=True)
        if 'fairway db import DATABASE FILE --yes' not in help_text:
            raise RuntimeError('Installed database commands are unavailable')
        launcher = data / 'bin' / ('fairway.cmd' if windows else 'fairway')
        if not launcher.is_file(): raise RuntimeError('Installed database command launcher is missing')
        if windows:
            help_text = subprocess.check_output(['cmd.exe', '/D', '/C', str(launcher), 'db', '--help'], text=True)
            if 'fairway db export DATABASE' not in help_text:
                raise RuntimeError('Windows database launcher did not return command output')
            # Exercise the actual installed PATH commands, including the spaced
            # installation directory, from both supported Windows shells.
            cli_env = {**os.environ, 'PATH': str(data / 'bin') + os.pathsep + os.environ['PATH']}
            for command, expected in [('php -v', 'PHP 8.'), ('composer --version', 'Composer version')]:
                for shell in [
                    ['cmd.exe', '/D', '/V:ON', '/C', command],
                    ['powershell.exe', '-NoLogo', '-NoProfile', '-NonInteractive',
                     '-Command', command + '; exit $LASTEXITCODE'],
                ]:
                    result = subprocess.run(shell, env=cli_env, capture_output=True, text=True, timeout=60)
                    if result.returncode or expected not in result.stdout:
                        raise RuntimeError(f'Installed {command} launcher failed: {result}')
        completed.append('installed-database-cli-and-launcher')
        update_evidence = None
        if args.updater:
            if not args.update_probe: raise RuntimeError('The signed update drill requires --update-probe')
            from updater_lifecycle import exercise
            if windows:
                update_evidence = exercise(args.update_probe, exe, args.updater, output)
            else:
                # The pkg lifecycle uses /Applications. The hosted account
                # cannot answer macOS's password prompt for a protected app.
                # Exercise the real updater on this exact installed bundle in
                # a writable location, then return the updated app for the
                # remaining installed-data and removal/reinstall checks.
                writable = output / 'user-applications' / installation.name
                writable.parent.mkdir()
                if writable.exists(): raise RuntimeError('Updater test location already exists')
                permissions = {'protectedParentWritable': os.access(installation.parent, os.W_OK),
                               'parentMode': oct(installation.parent.stat().st_mode & 0o7777),
                               'parentUid': installation.parent.stat().st_uid,
                               'parentGid': installation.parent.stat().st_gid,
                               'installedAppUid': installation.stat().st_uid}
                print('Mac updater location: ' + json.dumps(permissions), flush=True)
                run(['sudo', 'mv', str(installation), str(writable)])
                try:
                    # Moving a pkg-installed bundle retains root ownership.
                    # Match a normal per-user app installation, including its
                    # contents, before exercising replacement without a prompt.
                    run(['sudo', 'chown', '-R', f'{os.getuid()}:{os.getgid()}', str(writable)])
                    if writable.stat().st_uid != os.getuid():
                        raise RuntimeError('Updater test app is not owned by the test account')
                    run(['codesign', '--verify', '--deep', '--strict', str(writable)])
                    update_evidence = exercise(args.update_probe, writable / 'Contents/MacOS/fairway', args.updater, output)
                    update_evidence['installationScope'] = 'Exact pkg-installed code relocated to a per-user app directory with per-user ownership, signature verified, updated, then returned to /Applications.'
                    update_evidence['protectedInstallation'] = {**permissions, 'status': 'requires-human-authorization-prompt-check'}
                    (output / 'updater.json').write_text(json.dumps(update_evidence, indent=2) + '\n')
                finally:
                    if writable.exists() and not installation.exists():
                        run(['sudo', 'mv', str(writable), str(installation)])
            updated_build = json.loads(subprocess.check_output([str(exe), '--build-info'], text=True))
            if updated_build != build: raise RuntimeError('Updated app identity differs from the tested candidate')
            probe('verify'); completed.append('signed-update-preserves-database-settings-backup-coursework')
        runtime_manifest = json.loads((runtime / 'manifest.json').read_text())
        expected = json.loads((ROOT / 'runtime-manifest.json').read_text())
        if runtime_manifest != expected: raise RuntimeError('Installed runtime differs from the candidate')
        uninstall()
        saved = json.loads(state.read_text())
        if not (Path(saved['dataDir']) / 'mysql/mysql').is_dir() or not Path(saved['backup']).is_file() or not (Path(saved['documentRoot']) / 'beta-installer-check.php').is_file():
            raise RuntimeError('Uninstall did not preserve database, backup or coursework')
        if (data / 'cli-installed').exists(): raise RuntimeError('Uninstall left command integration enabled')
        completed.append('uninstall-preserves-work-and-removes-cli')
        install(args.installer); probe('verify'); completed.append('reinstall-opens-preserved-database')
        with args.installer.open('rb') as stream:
            if hashlib.file_digest(stream, 'sha256').hexdigest() != installer_sha256:
                raise RuntimeError('Candidate installer changed during lifecycle testing')
        report = {'schema': 1, 'status': 'passed', 'build': build, 'runtime': runtime_manifest,
                  'installer': args.installer.name, 'installerSha256': installer_sha256,
                  'baseline': '0.1.0-alpha.3', 'checks': completed, 'updater': update_evidence}
        (output / 'lifecycle.json').write_text(json.dumps(report, indent=2) + '\n')
    finally:
        if candidate_installed and exe.exists(): uninstall()
    print('PASS: actual installer, alpha upgrade, real database, CLI, uninstall and reinstall lifecycle')


if __name__ == '__main__': main()
