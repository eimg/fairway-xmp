#!/usr/bin/env python3
"""Package a Windows test app and its complete runtime without running an installer."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    executable = args.executable.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    stage = output / 'Fairway XMP Permissions Test'
    stage.mkdir()  # Refuse to mix an old build with new files.
    shutil.copy2(executable, stage / 'fairway.exe')
    console = ROOT / 'src-tauri/resources/cli/fairway-cli.exe'
    if not console.is_file():
        raise RuntimeError('Build the console entry point with node scripts/build_cli.mjs before packaging')
    (stage / 'cli').mkdir()
    shutil.copy2(console, stage / 'cli/fairway-cli.exe')
    loader = executable.with_name('WebView2Loader.dll')
    if loader.is_file():
        shutil.copy2(loader, stage / loader.name)
    shutil.copytree(ROOT / 'src-tauri/resources/runtime', stage / 'runtime')
    for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'runtime-manifest.json', 'runtime-downloads.lock.json']:
        shutil.copy2(ROOT / name, stage / name)
    shutil.copy2(ROOT / 'docs/testing.md', stage / 'TESTING.md')
    identity = json.loads(subprocess.check_output(
        [str(stage / 'fairway.exe'), '--build-info'], creationflags=subprocess.CREATE_NO_WINDOW))
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if identity['commit'] != commit or identity['modifiedSources']:
        raise RuntimeError('Standalone executable does not match the clean workflow source')
    console_identity = json.loads(subprocess.check_output([str(stage / 'cli/fairway-cli.exe'), '--build-info'], creationflags=subprocess.CREATE_NO_WINDOW))
    if console_identity['commit'] != commit or console_identity['modifiedSources'] or console_identity['version'] != identity['version']:
        raise RuntimeError('Console executable does not match the clean desktop workflow source')
    identity.update({'purpose': 'Standalone permissions test; no installer', 'authenticodeSigned': False})
    (stage / 'BUILD-INFO.json').write_text(json.dumps(identity, indent=2) + '\n', encoding='utf-8')
    archive = output / f'fairway-xmp-permissions-{commit[:8]}-windows-x64.zip'
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
        for file in sorted(stage.rglob('*')):
            if file.is_file():
                bundle.write(file, file.relative_to(output))
    with archive.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    (output / 'SHA256SUMS.txt').write_text(f'{digest}  {archive.name}\n', encoding='utf-8')
    shutil.copy2(stage / 'BUILD-INFO.json', output / 'BUILD-INFO.json')
    shutil.copy2(stage / 'TESTING.md', output / 'TESTING.md')
    shutil.rmtree(stage)  # Publish the compressed package once, not the duplicate tree.
    print(f'PASS: {archive.name}, SHA256 {digest}')


if __name__ == '__main__':
    main()
