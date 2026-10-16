#!/usr/bin/env python3
"""Publish one successful unsigned candidate run without rebuilding."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run_json(*args):
    return json.loads(subprocess.check_output(args, text=True))


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def published_name(target, name):
    marker = '-build-source.tar.gz'
    if name.endswith(marker):
        platform = {'macos-arm64': 'macos', 'windows-x64': 'windows'}[target]
        return name[: -len(marker)] + f'-{platform}-build-source.tar.gz'
    return name


def main():
    repo = os.environ['GH_REPO']
    run_id = os.environ['FAIRWAY_RUN_ID']
    tag = os.environ['FAIRWAY_TAG']
    sha = os.environ['FAIRWAY_SHA']
    notes = Path(os.environ.get('FAIRWAY_NOTES', ROOT / 'releases/beta1-notes.md'))
    version = tag[1:] if tag.startswith('v') else tag
    require(notes.is_file(), 'Release notes are missing')
    run = run_json('gh', 'run', 'view', run_id, '--repo', repo, '--json', 'conclusion,headSha,url')
    require(run['conclusion'] == 'success', 'Candidate run did not succeed')
    require(run['headSha'] == sha, 'Candidate run is not the requested commit')
    releases = run_json('gh', 'api', f'repos/{repo}/releases?per_page=100')
    require(all(item['tag_name'] != tag for item in releases), f'{tag} already exists')

    with tempfile.TemporaryDirectory() as temporary:
        download = Path(temporary) / 'artifacts'
        names = [
            f'candidate-metadata-{target}' for target in ('windows-x64', 'macos-arm64', 'linux-x86_64')
        ] + [
            f'unsigned-candidate-acceptance-{sha}',
            f'fairway-xmp-windows-x64-{version}',
            f'fairway-xmp-macos-arm64-{version}',
            f'fairway-xmp-linux-x86_64-{version}',
            f'fairway-xmp-linux-x86_64-{version}-sources',
        ]
        command = ['gh', 'run', 'download', run_id, '--repo', repo, '--dir', str(download)]
        for name in names:
            command.extend(['--name', name])
        subprocess.check_call(command)
        linux = download / f'fairway-xmp-linux-x86_64-{version}'
        for path in (download / f'fairway-xmp-linux-x86_64-{version}-sources').glob('*.tar.gz'):
            shutil.copy2(path, linux / path.name)
        directories = {
            'windows-x64': download / f'fairway-xmp-windows-x64-{version}',
            'macos-arm64': download / f'fairway-xmp-macos-arm64-{version}',
            'linux-x86_64': linux,
        }
        for target, directory in directories.items():
            subprocess.check_call([
                'python', 'scripts/release_candidate.py', 'verify-files',
                '--candidate', str(download / f'candidate-metadata-{target}' / f'candidate-{target}.json'),
                '--directory', str(directory),
            ], cwd=ROOT)
        manifest_path = download / f'unsigned-candidate-acceptance-{sha}' / 'candidate-manifest.json'
        manifest = json.loads(manifest_path.read_text())
        require(manifest['commit'] == sha and manifest['version'] == version, 'Candidate manifest does not match this release')
        stage = Path(temporary) / 'release'
        stage.mkdir()
        published = {}
        for target, directory in directories.items():
            candidate = json.loads((download / f'candidate-metadata-{target}' / f'candidate-{target}.json').read_text())
            for name, info in candidate['files'].items():
                source = directory / name
                destination_name = published_name(target, name)
                require(destination_name not in published, f'Duplicate release asset: {destination_name}')
                destination = stage / destination_name
                os.link(source, destination)
                published[destination_name] = {
                    'sha256': info['sha256'], 'bytes': info['bytes'], 'candidateName': name, 'target': target,
                }
                require(sha256(destination) == info['sha256'], f'Published file changed: {destination_name}')
        macos = directories['macos-arm64']
        for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'runtime-manifest.json', 'runtime-downloads.lock.json'):
            os.link(macos / name, stage / name)
        for name in ('runtime-linux-downloads.lock.json', 'runtime-sources.lock.json'):
            content = subprocess.check_output(['git', 'show', f'{sha}:{name}'], cwd=ROOT)
            (stage / name).write_bytes(content)
        shutil.copy2(manifest_path, stage / 'candidate-manifest.json')
        files = {
            path.name: {'sha256': sha256(path), 'bytes': path.stat().st_size, **published.get(path.name, {})}
            for path in stage.iterdir() if path.is_file()
        }
        build_info = {
            'schema': 1, 'version': version, 'tag': tag, 'commit': sha, 'run': run['url'],
            'signed': False, 'prerelease': True, 'latest': False, 'rebuilt': False, 'files': files,
        }
        (stage / 'BUILD-INFO.json').write_text(json.dumps(build_info, indent=2) + '\n')
        files['BUILD-INFO.json'] = {'sha256': sha256(stage / 'BUILD-INFO.json'), 'bytes': (stage / 'BUILD-INFO.json').stat().st_size}
        sums = ''.join(f"{files[name]['sha256']}  {name}\n" for name in sorted(files))
        (stage / 'SHA256SUMS.txt').write_text(sums)
        subprocess.check_call([
            'gh', 'release', 'create', tag, '--repo', repo, '--target', sha, '--prerelease', '--latest=false',
            '--title', f'Fairway XMP {version}', '--notes-file', str(notes),
            *sorted(stage.iterdir(), key=lambda path: path.name),
        ])
        print(f'Published {tag} from {run["url"]}')


if __name__ == '__main__':
    main()
