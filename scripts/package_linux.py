#!/usr/bin/env python3
"""Produce an offline .run installer and an equivalent manual-install tarball."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import tarfile
from prepare_linux_runtime import bundle_component_libraries

ROOT = Path(__file__).resolve().parents[1]


def package():
    version = json.loads((ROOT / 'runtime-manifest.json').read_text())
    app_version = json.loads((ROOT / 'package.json').read_text())['version']
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    build = app_version + '-' + commit[:12]
    stage = ROOT / '.runtime-work/linux-package/fairway-xmp'
    if stage.exists(): shutil.rmtree(stage)
    stage.mkdir(parents=True)
    (stage / 'bin').mkdir()
    shutil.copy2(ROOT / 'src-tauri/target/release/fairway-cli', stage / 'bin/fairway')
    shutil.copytree(ROOT / 'src-tauri/resources/runtime', stage / 'runtime', symlinks=False)
    cli_packages = bundle_component_libraries(stage / 'bin')
    library_sources = json.loads((stage / 'runtime/linux-library-sources.json').read_text())
    if not cli_packages.issubset({entry['package'] for entry in library_sources}):
        raise RuntimeError('The CLI needs additional native-library sources; include them before packaging')
    for file in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'README.md'):
        shutil.copy2(ROOT / file, stage / file)
    for file in ('install.sh', 'uninstall.sh'):
        shutil.copy2(ROOT / 'packaging/linux' / file, stage / file)
        (stage / file).chmod(0o755)
    (stage / 'BUILD').write_text(build + '\n')
    output = ROOT / 'artifacts/linux-installer'
    output.mkdir(parents=True, exist_ok=True)
    stem = f'fairway-xmp-{app_version}-linux-x86_64'
    archive = output / (stem + '.tar.gz')
    with tarfile.open(archive, 'w:gz', compresslevel=6) as bundle:
        bundle.add(stage, arcname='fairway-xmp')
    digest = hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest()
    installer = output / (stem + '.run')
    header = f'''#!/bin/sh
set -eu
TEMP=$(mktemp -d)
trap 'rm -rf "$TEMP"' EXIT HUP INT TERM
LINE=$(awk '/^__FAIRWAY_ARCHIVE_BELOW__$/ {{ print NR + 1; exit }}' "$0")
tail -n +"$LINE" "$0" > "$TEMP/payload.tar.gz"
printf '%s  %s\\n' '{digest}' "$TEMP/payload.tar.gz" | sha256sum -c - >/dev/null
tar -xzf "$TEMP/payload.tar.gz" -C "$TEMP"
"$TEMP/fairway-xmp/install.sh" "$@"
exit 0
__FAIRWAY_ARCHIVE_BELOW__
'''
    with installer.open('wb') as stream:
        stream.write(header.encode())
        with archive.open('rb') as payload: shutil.copyfileobj(payload, stream)
    installer.chmod(0o755)
    source = output / (stem + '-sources.tar.gz')
    source_stage = ROOT / '.runtime-work/linux-source-package'
    if source_stage.exists(): shutil.rmtree(source_stage)
    source_stage.mkdir()
    subprocess.run(['git', 'archive', '--format=tar.gz', f'--output={source_stage / "fairway-xmp-build-source.tar.gz"}', 'HEAD'], cwd=ROOT, check=True)
    shutil.copytree(ROOT / '.runtime-work/linux-sources', source_stage / 'runtime-sources')
    for file in ('runtime-linux-downloads.lock.json', 'runtime-manifest.json', 'THIRD_PARTY_NOTICES.md'):
        shutil.copy2(ROOT / file, source_stage / file)
    with tarfile.open(source, 'w:gz', compresslevel=1) as bundle:
        bundle.add(source_stage, arcname='fairway-xmp-sources')
    hashes = {file.name: hashlib.file_digest(file.open('rb'), 'sha256').hexdigest() for file in (installer, archive, source)}
    (output / 'SHA256SUMS.txt').write_text(''.join(f'{sha}  {name}\n' for name, sha in hashes.items()))
    (output / 'build-info.json').write_text(json.dumps({'version': app_version, 'commit': commit, 'target': 'x86_64-unknown-linux-gnu', 'minimumGlibc': '2.35', 'runtime': version, 'sha256': hashes, 'run': os.environ.get('GITHUB_SERVER_URL', '') + '/' + os.environ.get('GITHUB_REPOSITORY', '') + '/actions/runs/' + os.environ.get('GITHUB_RUN_ID', '')}, indent=2) + '\n')
    print(output)


if __name__ == '__main__': package()
