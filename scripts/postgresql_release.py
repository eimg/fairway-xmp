#!/usr/bin/env python3
"""Sign a private PostgreSQL copy, or stage an exact reviewed signed artifact.

Never executes payload tools, changes the source payload/pin, or overwrites output.
Run signing only on a disposable credential runner, after verifying source ancestry.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
from prepare_postgresql_runtime import TOOLS

ROOT = Path(__file__).resolve().parents[1]
PIN = ROOT / 'src-tauri/src/postgresql/managed/runtime-pin.json'
MAX_FILE = 128 * 1024 * 1024
MAX_TOTAL = 512 * 1024 * 1024


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    with path.open('x') as stream:
        stream.write(json.dumps(value, sort_keys=True, indent=2) + '\n')
        stream.flush()
        os.fsync(stream.fileno())
    path.chmod(0o400)


def safe_name(name):
    path = PurePosixPath(name)
    if not name or path.is_absolute() or any(p in ('', '.', '..') for p in name.split('/')) or any(c in name for c in '\\:\x00') or any(ord(c) < 32 for c in name):
        raise ValueError('Unsafe PostgreSQL artifact path')
    return path


def inventory(payload, bundled=False):
    """Reject aliases, writable/shared ownership and unexpected filesystem objects."""
    files = {}
    root = payload.lstat()
    owners = (os.getuid(), 0) if bundled else (os.getuid(),)
    forbidden = 0o7022 if bundled else 0o7077
    if not stat.S_ISDIR(root.st_mode) or root.st_uid not in owners or root.st_mode & forbidden:
        raise ValueError('PostgreSQL payload must be an owned private directory')
    total = 0
    for path in sorted(payload.rglob('*')):
        relative = path.relative_to(payload).as_posix()
        safe_name(relative)
        info = path.lstat()
        if info.st_uid not in owners or (info.st_mode & (0o7022 if relative == 'manifest.json' else forbidden)):
            raise ValueError('Unsafe PostgreSQL payload ownership/permissions')
        if stat.S_ISDIR(info.st_mode):
            continue
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > MAX_FILE:
            raise ValueError('PostgreSQL links, aliases and special/oversized files are forbidden')
        total += info.st_size
        if total > MAX_TOTAL or len(files) >= 4096:
            raise ValueError('Oversized PostgreSQL payload')
        if relative != 'manifest.json':
            files[relative] = {'sha256': digest(path), 'bytes': info.st_size, 'executable': bool(info.st_mode & 0o100)}
    return files


def verify_payload(payload, pin=None, bundled=False):
    files = inventory(payload, bundled)
    manifest = read(payload / 'manifest.json')
    lock = read(ROOT / 'postgresql-source.lock.json')
    expected = {'schemaVersion': 1, 'version': lock['version'], 'platform': lock['platform'], 'files': files}
    if manifest != expected or not files:
        raise ValueError('PostgreSQL payload manifest/tree mismatch')
    if any('bin/' + tool not in files or not files['bin/' + tool]['executable'] for tool in TOOLS):
        raise ValueError('Missing PostgreSQL runtime tool')
    if digest(payload / 'licenses' / ('postgresql-' + lock['version'] + '-source.tar.bz2')) != lock['sha256']:
        raise ValueError('PostgreSQL corresponding source checksum mismatch')
    for name in ('PostgreSQL-COPYRIGHT', 'timezone-README'):
        if 'licenses/' + name not in files:
            raise ValueError('Missing PostgreSQL notice')
    actual_pin = {'version': manifest['version'], 'platform': manifest['platform'], 'manifestSha256': digest(payload / 'manifest.json')}
    if pin is not None and actual_pin != pin:
        raise ValueError('Signed PostgreSQL artifact does not match the reviewed application pin')
    return manifest, actual_pin


def verify_code(payload, team):
    if not re.fullmatch(r'[A-Z0-9]{10}', team):
        raise ValueError('Expected Apple Developer team ID')
    requirement = (f'anchor apple generic and certificate leaf[subject.OU] = "{team}" and '
                   'certificate 1[field.1.2.840.113635.100.6.2.6] exists and '
                   'certificate leaf[field.1.2.840.113635.100.6.1.13] exists')
    return verify_native_code(payload, requirement)


def verify_development_code(payload):
    """Free sharing accepts valid ad-hoc code, without claiming Developer ID trust."""
    return verify_native_code(payload, None)


def verify_native_code(payload, requirement):
    binaries = []
    for path in sorted(payload.rglob('*')):
        if not path.is_file():
            continue
        with path.open('rb') as stream:
            magic = stream.read(4)
        if magic != b'\xcf\xfa\xed\xfe':
            continue
        binaries.append(path)
        command = ['/usr/bin/codesign', '--verify', '--strict']
        if requirement is not None:
            command.append('-R=' + requirement)
        subprocess.run([*command, str(path)], check=True, capture_output=True)
        details = subprocess.check_output(['/usr/bin/codesign', '-d', '--verbose=4', str(path)], stderr=subprocess.STDOUT, text=True)
        if requirement is not None and not re.search(r'flags=.*\bruntime\b', details):
            raise ValueError('PostgreSQL code must use hardened runtime signing')
        if requirement is None and 'Signature=adhoc' not in details:
            raise ValueError('Free sharing expects the verified ad-hoc development payload')
        if subprocess.check_output(['/usr/bin/lipo', '-archs', str(path)], text=True).strip() != 'arm64':
            raise ValueError('Unexpected PostgreSQL architecture')
        for line in subprocess.check_output(['/usr/bin/otool', '-L', str(path)], text=True).splitlines()[1:]:
            dependency = line.strip().split(' (')[0]
            if dependency.startswith(('/usr/lib/', '/System/Library/')):
                continue
            if not dependency.startswith('@loader_path/'):
                raise ValueError('Non-private PostgreSQL library dependency')
            target = (path.parent / dependency.removeprefix('@loader_path/')).resolve(strict=True)
            if not target.is_relative_to(payload.resolve()):
                raise ValueError('PostgreSQL library escaped payload')
    if not binaries or any(payload / 'bin' / tool not in binaries for tool in TOOLS):
        raise ValueError('PostgreSQL tools must be signed arm64 Mach-O files')
    return binaries


def publish(stage, output):
    """macOS exclusive directory rename; fsync both tree and publication parent."""
    for path in [*stage.rglob('*'), stage]:
        if path.is_symlink():
            raise ValueError('Publication may not follow symlinks')
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.renamex_np(os.fsencode(stage), os.fsencode(output), 0x4) != 0:
        raise OSError(ctypes.get_errno(), 'Exclusive PostgreSQL publication failed')
    descriptor = os.open(output.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def new_stage(output):
    output = output.absolute()
    parent = output.parent.resolve(strict=True)
    if parent != output.parent or output.exists() or output.is_symlink():
        raise ValueError('Use a new output under a canonical existing parent')
    stage = Path(tempfile.mkdtemp(prefix='.fw-pg-release-', dir=parent))
    return stage, output


def sign(source, output, identity, team):
    verify_payload(source)
    if not re.fullmatch(r'[A-Z0-9]{10}', team) or not identity.startswith('Developer ID Application: ') or not identity.endswith('(' + team + ')') or any(c in identity for c in '\r\n'):
        raise ValueError('Signing identity must match the configured Apple team')
    stage, output = new_stage(output)
    payload = stage / 'postgresql'
    shutil.copytree(source, payload)
    # Only this newly owned copy is mutable. The original development payload stays intact.
    for path in sorted(payload.rglob('*')):
        if not path.is_file():
            continue
        with path.open('rb') as stream:
            magic = stream.read(4)
        if magic == b'\xcf\xfa\xed\xfe':
            path.chmod(0o700)
            subprocess.run(['/usr/bin/codesign', '--force', '--options', 'runtime', '--timestamp', '--sign', identity, str(path)], check=True, capture_output=True)
            path.chmod(0o500)
    verify_code(payload, team)
    manifest = read(payload / 'manifest.json')
    (payload / 'manifest.json').unlink()
    manifest['files'] = inventory(payload)
    write(payload / 'manifest.json', manifest)
    (payload / 'manifest.json').chmod(0o400)
    _, pin = verify_payload(payload)
    write(stage / 'runtime-pin.json', pin)
    write(stage / 'provenance.json', {'teamId': team, 'sourceLockSha256': digest(ROOT / 'postgresql-source.lock.json'), 'recipeSha256': digest(ROOT / 'scripts/prepare_postgresql_runtime.py')})
    publish(stage, output)
    print('Signed PostgreSQL artifact prepared. Review runtime-pin.json before committing the new application pin.')


def unpack(archive, stage):
    # Inspect the entire archive before creating any members. Never extract links.
    with tarfile.open(archive, 'r:gz') as bundle:
        members = bundle.getmembers()
        names, total = set(), 0
        for member in members:
            safe_name(member.name)
            if member.name in names or not (member.isdir() or member.isfile()) or member.mode & 0o077:
                raise ValueError('Unsafe/duplicate PostgreSQL archive member')
            names.add(member.name)
            if member.name.split('/')[0] not in ('postgresql', 'runtime-pin.json', 'provenance.json'):
                raise ValueError('Unexpected PostgreSQL archive member')
            total += member.size
            if len(names) > 8192 or member.size > MAX_FILE or total > MAX_TOTAL:
                raise ValueError('Oversized PostgreSQL archive')
        bundle.extractall(stage, members=members, filter='data')
        # Python's data filter intentionally clears directory modes and may add
        # owner write permission. Restore private staging modes explicitly.
        for member in members:
            (stage / member.name).chmod(0o700 if member.isdir() else 0o500 if member.mode & 0o100 else 0o400)


def stage_archive(archive, output, team):
    stage, output = new_stage(output)
    unpack(archive, stage)
    if read(stage / 'provenance.json') != {'teamId': team, 'sourceLockSha256': digest(ROOT / 'postgresql-source.lock.json'), 'recipeSha256': digest(ROOT / 'scripts/prepare_postgresql_runtime.py')}:
        raise ValueError('PostgreSQL artifact provenance differs from the reviewed source recipe/team')
    pin = read(PIN)
    if read(stage / 'runtime-pin.json') != pin:
        raise ValueError('Commit the reviewed signed runtime pin before building a release')
    verify_payload(stage / 'postgresql', pin)
    verify_code(stage / 'postgresql', team)
    # Installed bundle resources must be readable by the student even when the
    # package installer owns them as root. Hashes and executable flags stay exact.
    payload = stage / 'postgresql'
    for path in payload.rglob('*'):
        path.chmod(0o755 if path.is_dir() else 0o555 if path.stat().st_mode & 0o100 else 0o444)
    payload.chmod(0o755)
    verify_payload(payload, pin, bundled=True)
    publish(stage / 'postgresql', output)
    print('Staged exact reviewed signed PostgreSQL runtime; no cluster created.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    signer = commands.add_parser('sign')
    signer.add_argument('--source', type=Path, required=True)
    signer.add_argument('--output', type=Path, required=True)
    signer.add_argument('--team-id', required=True)
    signer.add_argument('--identity', required=True)
    stage = commands.add_parser('stage')
    stage.add_argument('--archive', type=Path, required=True)
    stage.add_argument('--output', type=Path, required=True)
    stage.add_argument('--team-id', required=True)
    verifier = commands.add_parser('verify')
    verifier.add_argument('--payload', type=Path, required=True)
    verifier.add_argument('--team-id', required=True)
    args = parser.parse_args()
    if args.command == 'sign':
        sign(args.source, args.output, args.identity, args.team_id)
    elif args.command == 'stage':
        stage_archive(args.archive, args.output, args.team_id)
    else:
        verify_payload(args.payload, read(PIN), bundled=True)
        verify_code(args.payload, args.team_id)
        print('Verified PostgreSQL payload pin, files, notices, signatures and private dependencies.')


if __name__ == '__main__':
    main()
