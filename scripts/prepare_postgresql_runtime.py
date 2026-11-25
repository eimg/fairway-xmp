#!/usr/bin/env python3
"""Prepare the separately pinned PostgreSQL runtime in an exclusive scratch directory.

Requires explicit source-build authorization. Never installs globally, calls brew,
starts services, initializes clusters or updates the application's reviewed pin.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / 'postgresql-source.lock.json'
TOOLS = ('postgres', 'initdb', 'pg_ctl', 'psql', 'pg_isready', 'pg_dump', 'pg_restore', 'pg_dumpall', 'pg_controldata')


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def relocate(payload, prefix, env):
    """Accept only system dylibs or this source build's own prefix."""
    binaries = []
    for path in payload.rglob('*'):
        if path.is_file() and path.read_bytes()[:4] == b'\xcf\xfa\xed\xfe':
            binaries.append(path)
    for binary in binaries:
        lines = subprocess.check_output(['/usr/bin/otool', '-L', binary], text=True, env=env).splitlines()[1:]
        for line in lines:
            original = line.strip().split(' (')[0]
            if original.startswith(('/usr/lib/', '/System/Library/')):
                continue
            if not original.startswith(prefix + '/'):
                raise RuntimeError(f'Unresolved non-system dependency: {original}')
            target = payload / original.removeprefix(prefix + '/')
            if not target.is_file():
                raise RuntimeError(f'Missing private dependency: {original}')
            relative = os.path.relpath(target, binary.parent)
            subprocess.run(['/usr/bin/install_name_tool', '-change', original, '@loader_path/' + relative, binary], check=True, env=env)
        if binary.suffix in ('.dylib', '.so'):
            subprocess.run(['/usr/bin/install_name_tool', '-id', '@loader_path/' + binary.name, binary], check=True, env=env)
        subprocess.run(['/usr/bin/codesign', '--force', '--sign', '-', binary], check=True, capture_output=True, env=env)
    for binary in binaries:
        subprocess.run(['/usr/bin/codesign', '--verify', '--strict', binary], check=True, env=env)
        arch = subprocess.check_output(['/usr/bin/lipo', '-archs', binary], text=True, env=env).strip()
        if arch != 'arm64':
            raise RuntimeError('Unexpected runtime architecture')
        for line in subprocess.check_output(['/usr/bin/otool', '-L', binary], text=True, env=env).splitlines()[1:]:
            dependency = line.strip().split(' (')[0]
            if dependency.startswith(('/usr/lib/', '/System/Library/')):
                continue
            if not dependency.startswith('@loader_path/'):
                raise RuntimeError('Non-relocatable dependency remains')
            target = (binary.parent / dependency.removeprefix('@loader_path/')).resolve(strict=True)
            if not target.is_relative_to(payload.resolve()):
                raise RuntimeError('Library escaped payload')
    return len(binaries)


def verify_source(archive, expected):
    if digest(archive) != expected:
        raise RuntimeError('Official PostgreSQL source checksum mismatch; nothing executed')


def extract_source(archive, source):
    with tarfile.open(archive) as bundle:
        for member in bundle.getmembers():
            if not (member.isdir() or member.isfile()) or Path(member.name).is_absolute() or '..' in Path(member.name).parts:
                raise RuntimeError('Unsafe source archive member')
        bundle.extractall(source, filter='data')


def build(output):
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        raise RuntimeError('Managed payload currently requires macOS arm64')
    output = Path(output).absolute()
    if output.exists() or output.is_symlink():
        raise RuntimeError('Use a new output path; existing data is never replaced')
    parent = output.parent.resolve(strict=True)
    if parent != output.parent:
        raise RuntimeError('Use a canonical parent without symlinks')
    lock = json.loads(LOCK.read_text())
    work = Path(tempfile.mkdtemp(prefix='.fw-pg-source-', dir=parent))
    print(f'Owned build staging retained for diagnosis: {work}', flush=True)
    env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LC_ALL': 'C', 'HOME': str(work), 'MACOSX_DEPLOYMENT_TARGET': '15.0'}
    archive = work / 'postgresql.tar.bz2'
    with urllib.request.urlopen(lock['url'], timeout=60) as response, archive.open('xb') as dest:
        shutil.copyfileobj(response, dest)
    verify_source(archive, lock['sha256'])
    print('Official source SHA256 verified: ' + lock['sha256'], flush=True)
    source = work / 'source'
    source.mkdir(mode=0o700)
    extract_source(archive, source)
    source = source / ('postgresql-' + lock['version'])
    # Upstream configure/make scripts are executed only after checksum verification.
    # DESTDIR confines make install to this owned stage, despite the virtual prefix.
    subprocess.run([str(source / 'configure'), '--prefix=' + lock['prefix'], *lock['configure'], 'CFLAGS=-O2', 'LDFLAGS=-Wl,-headerpad_max_install_names'], cwd=source, env=env, check=True)
    subprocess.run(['/usr/bin/make', '-j4'], cwd=source, env=env, check=True)
    dest = work / 'dest'
    subprocess.run(['/usr/bin/make', 'install', 'DESTDIR=' + str(dest)], cwd=source, env=env, check=True)
    package(work, dest / lock['prefix'].lstrip('/'), source, lock, env, output)


def package(work, installed, source, lock, env, output):
    payload = work / 'payload'
    payload.mkdir(mode=0o700)
    (payload / 'bin').mkdir()
    for name in TOOLS:
        shutil.copy2(installed / 'bin' / name, payload / 'bin' / name)
    # Keep server modules, libpq and relocation-relative timezone/catalog resources.
    # Dereference only links originating from this freshly built private tree.
    shutil.copytree(installed / 'lib', payload / 'lib', symlinks=False)
    shutil.copytree(installed / 'share', payload / 'share', symlinks=False)
    for path in list((payload / 'lib').rglob('*')):
        if path.is_file() and path.suffix in ('.a', '.pc'):
            path.unlink()
    # Developer build rules/headers are not part of the managed runtime.
    shutil.rmtree(payload / 'lib/pgxs')
    (payload / 'lib/pkgconfig').rmdir()
    (payload / 'licenses').mkdir()
    shutil.copy2(source / 'COPYRIGHT', payload / 'licenses/PostgreSQL-COPYRIGHT')
    # Preserve the complete upstream notices and corresponding sources unchanged.
    shutil.copy2(work / 'postgresql.tar.bz2', payload / ('licenses/postgresql-' + lock['version'] + '-source.tar.bz2'))
    shutil.copy2(source / 'src/timezone/README', payload / 'licenses/timezone-README')
    count = relocate(payload, lock['prefix'], env)
    for name in TOOLS:
        result = subprocess.check_output([payload / 'bin' / name, '--version'], env=env, text=True, timeout=30)
        if '(PostgreSQL) ' + lock['version'] not in result:
            raise RuntimeError('Unexpected tool version: ' + name)
    files = {}
    for path in sorted(payload.rglob('*')):
        if path.is_symlink():
            raise RuntimeError('Payload may not contain symbolic links')
        if path.is_dir():
            path.chmod(0o700)
        elif path.is_file():
            executable = bool(path.stat().st_mode & 0o111)
            path.chmod(0o500 if executable else 0o400)
            files[path.relative_to(payload).as_posix()] = {'sha256': digest(path), 'bytes': path.stat().st_size, 'executable': executable}
    manifest = {'schemaVersion': 1, 'version': lock['version'], 'platform': 'macos-arm64', 'files': files}
    (payload / 'manifest.json').write_text(json.dumps(manifest, sort_keys=True, indent=2) + '\n')
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.renamex_np(os.fsencode(payload), os.fsencode(output), 0x4) != 0:
        raise OSError(ctypes.get_errno(), 'Exclusive payload publication failed')
    print(f'Validated {count} arm64 Mach-O files and dependency closure', flush=True)
    print('Payload manifest SHA256: ' + digest(output / 'manifest.json'), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--authorize-source-build', action='store_true', required=True)
    args = parser.parse_args()
    build(args.output)
