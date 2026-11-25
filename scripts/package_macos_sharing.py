#!/usr/bin/env python3
"""Package an ad-hoc Mac app for manual sharing; never sign/notarize as a release.

Only a new owned copy is modified. The verified PostgreSQL payload stays byte
identical to its compiled pin. Native hosts use the existing password fallback.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
from postgresql_release import ROOT, digest, new_stage, publish, read, verify_development_code, verify_payload, write
from sign_macos import signing_command, signing_targets


def verify_identity(identity, pin, commit, version):
    if (identity.get('distribution') != 'free-sharing' or identity.get('commit') != commit
            or identity.get('version') != version or identity.get('os') != 'macos'
            or identity.get('architecture') != 'aarch64'
            or identity.get('postgresqlRuntimePin') != pin
            or identity.get('features', {}).get('nativeMacosHosts') is not False
            or identity.get('features', {}).get('nativeMacosTrust') is not True):
        raise ValueError('Sharing executable identity/features/pin differ from the prepared build')


def verify_sources(archive, pin, commit, version):
    prefix = f'fairway-xmp-{version}-sources/'
    with tarfile.open(archive, 'r:gz') as bundle:
        def content(name, limit=1024 * 1024):
            member = bundle.getmember(prefix + name)
            if not member.isfile() or member.size > limit:
                raise ValueError('Invalid sharing source archive entry')
            return bundle.extractfile(member).read(limit + 1)
        if f'Fairway XMP source commit: {commit}.' not in content('README.md').decode():
            raise ValueError('Corresponding source archive belongs to another commit')
        if json.loads(content('postgresql/runtime-pin.json')) != pin:
            raise ValueError('Corresponding source archive belongs to another PostgreSQL payload')
        if json.loads(content('runtime-manifest.json')) != read(ROOT / 'runtime-manifest.json'):
            raise ValueError('Corresponding source runtime versions differ')
        lock = read(ROOT / 'postgresql-source.lock.json')
        if json.loads(content('postgresql/postgresql-source.lock.json')) != lock:
            raise ValueError('Corresponding PostgreSQL source lock differs')
        source = bundle.getmember(prefix + f"postgresql/postgresql-{lock['version']}-source.tar.bz2")
        if not source.isfile() or hashlib.file_digest(bundle.extractfile(source), 'sha256').hexdigest() != lock['sha256']:
            raise ValueError('Corresponding PostgreSQL source archive checksum differs')
        for name in ('postgresql/PostgreSQL-COPYRIGHT', 'postgresql/timezone-README'):
            if not content(name):
                raise ValueError('Missing corresponding source notice')
        application = bundle.getmember(prefix + 'fairway-xmp-source.tar.gz')
        if not application.isfile() or application.size == 0:
            raise ValueError('Missing application source archive')


def package(app, sources, output):
    app = app.resolve(strict=True)
    sources = sources.resolve(strict=True)
    if sources.is_symlink() or not sources.is_file():
        raise ValueError('Corresponding source archive is required')
    pin = read(ROOT / 'artifacts/free-sharing/runtime-pin.json')
    version = read(ROOT / 'package.json')['version']
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    verify_sources(sources, pin, commit, version)
    # --build-info exits before app/service initialization and never opens a GUI.
    identity = json.loads(subprocess.check_output([str(app / 'Contents/MacOS/fairway'), '--build-info'], text=True))
    verify_identity(identity, pin, commit, version)
    stage, output = new_stage(output)
    staged_app = stage / 'Fairway XMP.app'
    subprocess.run(['/usr/bin/ditto', str(app), str(staged_app)], check=True)
    resources = staged_app / 'Contents/Resources'
    resources.mkdir(parents=True, exist_ok=True)
    for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md'):
        shutil.copy2(ROOT / name, resources / name)
    shutil.copy2(ROOT / 'docs/free-sharing.md', resources / 'INSTALL.md')
    runtime = resources / 'runtime'
    if runtime.exists() or runtime.is_symlink():
        raise ValueError('Build with the free sharing resource overlay; runtime is copied once')
    source_runtime = ROOT / 'src-tauri/resources/runtime'
    verify_payload(source_runtime / 'postgresql', pin, bundled=True)
    verify_development_code(source_runtime / 'postgresql')
    shutil.copytree(source_runtime, runtime, symlinks=True)
    postgres = runtime / 'postgresql'
    for path in postgres.rglob('*'):
        path.chmod(0o755 if path.is_dir() else 0o555 if path.stat().st_mode & 0o100 else 0o444)
    postgres.chmod(0o755)
    verify_payload(postgres, pin, bundled=True)

    def sign(path):
        path.chmod(path.stat().st_mode | 0o200)
        subprocess.run(signing_command(path, '-', staged_app, adhoc=True), check=True, capture_output=True)

    # Never re-sign the separately pinned PostgreSQL code, nor use --deep signing.
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(sign, signing_targets(staged_app)))
    sign(staged_app)
    subprocess.run(['/usr/bin/codesign', '--verify', '--deep', '--strict', str(staged_app)], check=True)
    verify_payload(postgres, pin, bundled=True)
    verify_development_code(postgres)
    after = json.loads(subprocess.check_output([str(staged_app / 'Contents/MacOS/fairway'), '--build-info'], text=True))
    verify_identity(after, pin, commit, version)
    identity.update({'purpose': 'Free manual sharing build', 'appleNotarized': False, 'updates': 'manual', 'signing': 'ad-hoc; no Apple Developer identity'})
    write(stage / 'BUILD-INFO.json', identity)
    write(stage / 'postgresql-runtime-pin.json', pin)
    for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'runtime-manifest.json', 'runtime-downloads.lock.json', 'postgresql-source.lock.json'):
        shutil.copy2(ROOT / name, stage / name)
    shutil.copy2(ROOT / 'docs/free-sharing.md', stage / 'INSTALL.md')
    shutil.copy2(sources, stage / sources.name)
    subprocess.run(['git', 'archive', '--format=tar.gz', '--output=' + str(stage / 'fairway-build-source.tar.gz'), commit], cwd=ROOT, check=True)
    archive = stage / f'fairway-xmp-{version}-{commit[:8]}-macos-arm64-sharing.zip'
    subprocess.run(['/usr/bin/ditto', '-c', '-k', '--keepParent', str(staged_app), str(archive)], check=True)
    names = [archive, stage / sources.name, stage / 'fairway-build-source.tar.gz', stage / 'BUILD-INFO.json', stage / 'postgresql-runtime-pin.json']
    with (stage / 'SHA256SUMS.txt').open('x') as stream:
        for path in names:
            stream.write(digest(path) + '  ' + path.name + '\n')
    publish(stage, output)
    print('Prepared ad-hoc Mac sharing package. Manual Gatekeeper/Keychain acceptance remains unverified.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--sources', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    package(args.app, args.sources, args.output)
