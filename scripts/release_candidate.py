#!/usr/bin/env python3
"""Validate and describe release-candidate artifacts; never publish."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('windows-x64', 'macos-arm64', 'linux-x86_64')


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8-sig'))


def write(path, data):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + '\n')


def versions(root=ROOT):
    version = read(root / 'package.json')['version']
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', version):
        raise ValueError('Invalid application version')
    values = {
        'tauri': read(root / 'src-tauri/tauri.conf.json')['version'],
        'cargo': tomllib.loads((root / 'src-tauri/Cargo.toml').read_text())['package']['version'],
        'npm lock': read(root / 'package-lock.json')['packages']['']['version'],
        'npm lock header': read(root / 'package-lock.json')['version'],
        'cargo lock': next(package['version'] for package in tomllib.loads((root / 'src-tauri/Cargo.lock').read_text())['package'] if package['name'] == 'fairway'),
    }
    if any(value != version for value in values.values()):
        raise ValueError(f'Version mismatch: package.json={version}, {values}')
    return version


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def record(target, directory, artifact, lifecycle, output, signing="unsigned"):
    version = versions()
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT, text=True).strip():
        raise ValueError('Candidate checkout changed after compilation; commit reviewed inputs and rebuild')
    evidence = read(lifecycle)
    build = evidence['build']
    if build.get('distribution', 'standard') != 'standard':
        raise ValueError('Free sharing builds are not signed release candidates')
    if evidence.get('status') != 'passed' or build['version'] != version or build['commit'] != commit or build['modifiedSources']:
        raise ValueError('Installed artifact identity/lifecycle does not match the clean candidate source')
    expected_system = {'windows-x64': ('windows', 'x86_64'), 'macos-arm64': ('macos', 'aarch64'), 'linux-x86_64': ('linux', 'x86_64')}[target]
    if (build.get('os'), build.get('architecture')) != expected_system:
        raise ValueError('Installed binary OS/architecture differs from the candidate target')
    runtime = read(ROOT / 'runtime-manifest.json')
    if evidence['runtime'] != runtime:
        raise ValueError('Installed runtime differs from the candidate manifest')
    files = {path.name: {'sha256': digest(path), 'bytes': path.stat().st_size} for path in sorted(Path(directory).iterdir())
             if path.is_file() and path.name.endswith(('.exe', '.pkg', '.run', '.tar.gz', '.sig'))}
    extension = {'windows-x64': '-setup.exe', 'macos-arm64': '-macos-arm64.pkg' if signing == 'apple-notarized' else '-unsigned.pkg', 'linux-x86_64': '.run'}[target]
    installers = [name for name in files if name.endswith(extension)]
    if len(installers) != 1 or version not in installers[0]:
        raise ValueError('Expected exactly one versioned candidate installer')
    if evidence.get('installer') != installers[0] or evidence.get('installerSha256') != files[installers[0]]['sha256']:
        raise ValueError('Candidate installer differs from the file used by the lifecycle test')
    data = {'schema': 1, 'target': target, 'version': version, 'commit': commit, 'signed': signing == 'apple-notarized', 'signing': signing,
            'artifact': artifact, 'relatedArtifacts': [f'fairway-xmp-linux-x86_64-{version}-sources'] if target == 'linux-x86_64' else [], 'installer': installers[0], 'files': files, 'runtime': runtime, 'lifecycle': evidence,
            'run': f"{os.environ.get('GITHUB_SERVER_URL', '')}/{os.environ.get('GITHUB_REPOSITORY', '')}/actions/runs/{os.environ.get('GITHUB_RUN_ID', '')}"}
    if signing != 'unsigned':
        if (target == 'macos-arm64' and signing != 'apple-notarized') or (target == 'windows-x64' and signing != 'updater-only') or target == 'linux-x86_64':
            raise ValueError('Invalid signing policy for target')
        updater = f'fairway-xmp-{version}-macos-arm64.app.tar.gz' if target == 'macos-arm64' else installers[0]
        if updater not in files or updater + '.sig' not in files:
            raise ValueError('Signed release requires the exact updater file and signature')
        if (evidence.get('updater') or {}).get('status') != 'passed':
            raise ValueError('Signed release requires the native updater lifecycle drill')
        data['updater'] = {'artifact': updater, 'signature': updater + '.sig'}
    write(output, data)
    write(Path(directory) / 'candidate-info.json', data)
    (Path(directory) / 'SHA256SUMS.txt').write_text(''.join(f'{info["sha256"]}  {name}\n' for name, info in files.items()))


def assemble(inputs, output, require_updater=False):
    candidates = [read(path) for path in Path(inputs).rglob('candidate-*.json')]
    by_target = {candidate['target']: candidate for candidate in candidates}
    if len(candidates) != 3 or set(by_target) != set(TARGETS):
        raise ValueError('Need exactly one candidate for every supported target')
    version = versions()
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    runtime = read(ROOT / 'runtime-manifest.json')
    for candidate in candidates:
        if candidate['version'] != version or candidate['commit'] != commit or candidate['runtime'] != runtime or candidate['lifecycle']['status'] != 'passed':
            raise ValueError('Candidate versions, source, runtime or lifecycle results disagree')
    if require_updater:
        if by_target['macos-arm64'].get('signing') != 'apple-notarized' or by_target['windows-x64'].get('signing') != 'updater-only':
            raise ValueError('Release candidates require signed/notarized Mac builds and signed Windows updates')
        for target in ('macos-arm64', 'windows-x64'):
            if not by_target[target].get('updater') or by_target[target]['lifecycle'].get('updater', {}).get('status') != 'passed':
                raise ValueError('Missing native updater evidence')
    data = {'schema': 1, 'version': version, 'commit': commit, 'status': 'ready-for-publication',
            'uiReview': {'artifact': f'ui-visual-review-{commit}'},
            'signing': {target: by_target[target].get('signing', 'unsigned') for target in TARGETS},
            'knownLimitations': ['Windows installer has no Authenticode signature; first downloads can show SmartScreen. Linux updates are manual.' if require_updater else 'Unsigned installers; manual updates.', 'Windows MySQL requires ANSI-compatible filesystem paths or existing short-name aliases; see docs/data-browser.md.', 'Windows non-UTF-8 system code pages require ASCII identifiers for MySQL CLI backup/export; a failed pre-update backup blocks installation and preserves data/services. See docs/data-browser.md.'],
            'candidates': by_target}
    write(output, data)
    return data


def verify_files(candidate, directory):
    for name, info in candidate['files'].items():
        if Path(name).name != name or '\\' in name or name in ('.', '..'):
            raise ValueError('Unsafe artifact filename')
        path = Path(directory) / name
        if not path.is_file() or digest(path) != info['sha256'] or path.stat().st_size != info['bytes']:
            raise ValueError(f'Artifact changed or is missing: {name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('version')
    task = commands.add_parser('record')
    for name in ('target', 'directory', 'artifact', 'lifecycle', 'output'):
        task.add_argument('--' + name, required=True)
    task.add_argument('--signing', choices=('unsigned', 'apple-notarized', 'updater-only'), default='unsigned')
    task = commands.add_parser('assemble')
    task.add_argument('--require-updater', action='store_true')
    task.add_argument('--inputs', required=True); task.add_argument('--output', required=True)
    task = commands.add_parser('verify-files')
    task.add_argument('--candidate', required=True); task.add_argument('--directory', required=True)
    args = parser.parse_args()
    if args.command == 'version': print(versions())
    elif args.command == 'record': record(args.target, args.directory, args.artifact, args.lifecycle, args.output, args.signing)
    elif args.command == 'assemble': assemble(args.inputs, args.output, args.require_updater)
    elif args.command == 'verify-files': verify_files(read(args.candidate), args.directory)


if __name__ == '__main__':
    main()
