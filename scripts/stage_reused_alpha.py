#!/usr/bin/env python3
"""Verify and stage alpha 3 from existing builds; leave publication to the owner."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verified_copy(source, destination, expected):
    actual = sha256(source)
    if actual != expected:
        raise RuntimeError(f'Checksum mismatch: {source.name}')
    shutil.copy2(source, destination)
    return actual


def main():
    manifest = json.loads((ROOT / 'releases/alpha3-builds.json').read_text())
    version = manifest['releaseVersion']
    tag = 'v' + version
    repo = os.environ['GH_REPO']
    # The release-by-tag endpoint excludes drafts; the authenticated list includes them.
    releases = json.loads(run('gh', 'api', f'repos/{repo}/releases?per_page=100'))
    release = next(item for item in releases if item['tag_name'] == tag)
    if not release['draft'] or not release['prerelease']:
        raise RuntimeError('Staging requires an existing draft prerelease')
    output = ROOT / 'artifacts' / ('alpha-' + version)
    inputs = ROOT / '.runtime-work' / ('release-inputs-' + version)
    output.mkdir(parents=True, exist_ok=True)
    inputs.mkdir(parents=True, exist_ok=True)
    builds = {}
    for platform in ('windows', 'linux'):
        plan = manifest[platform]
        result = json.loads(run('gh', 'run', 'view', str(plan['runId']),
                                '--json', 'conclusion,headSha,url'))
        if result['conclusion'] != 'success' or result['headSha'] != plan['commit']:
            raise RuntimeError(f'Unexpected {platform} build provenance')
        directory = inputs / platform
        run('gh', 'run', 'download', str(plan['runId']), '--name', plan['artifact'],
            '--dir', str(directory))
        info = json.loads((directory / 'build-info.json').read_text(encoding='utf-8-sig'))
        if info['commit'] != plan['commit'] or info['version'] != manifest['embeddedVersion']:
            raise RuntimeError(f'Unexpected {platform} embedded version/commit')
        builds[platform] = {'originalBuild': info, 'verifiedRun': result, 'assets': {}}
        shutil.copy2(directory / 'build-info.json', output / f'{platform}-build-info.json')
    windows = inputs / 'windows'
    info = builds['windows']['originalBuild']
    name = f'Fairway-XMP-{version}-windows-x64-setup.exe'
    builds['windows']['assets'][name] = verified_copy(windows / info['installer'], output / name, info['sha256'])
    shutil.copy2(windows / 'fairway-xmp-build-source.tar.gz', output / f'Fairway-XMP-{version}-windows-app-source.tar.gz')
    linux = inputs / 'linux'
    plan = manifest['linux']
    run('gh', 'run', 'download', str(plan['runId']), '--name', plan['sourcesArtifact'], '--dir', str(linux))
    for original, digest in builds['linux']['originalBuild']['sha256'].items():
        name = original.replace(manifest['embeddedVersion'], version)
        builds['linux']['assets'][name] = verified_copy(linux / original, output / name, digest)
    sources = manifest['bundledToolSources']
    source_dir = inputs / 'bundled-sources'
    run('gh', 'release', 'download', sources['tag'], '--pattern', sources['asset'], '--dir', str(source_dir))
    name = f'Fairway-XMP-{version}-bundled-tool-sources.tar.gz'
    verified_copy(source_dir / sources['asset'], output / name, sources['sha256'])
    mac = manifest['macos']
    run('gh', 'release', 'download', tag, '--pattern', mac['asset'], '--dir', str(output), '--skip-existing')
    if sha256(output / mac['asset']) != mac['sha256']:
        raise RuntimeError('Mac package checksum mismatch')
    run('git', 'archive', '--format=tar.gz', '--output=' + str(output / f'Fairway-XMP-{version}-macos-app-source.tar.gz'), mac['associatedSourceCommit'])
    for filename in ('runtime-downloads.lock.json', 'runtime-linux-downloads.lock.json',
                     'runtime-sources.lock.json', 'runtime-manifest.json', 'LICENSE', 'THIRD_PARTY_NOTICES.md'):
        shutil.copy2(ROOT / filename, output / filename)
    build_info = {'releaseVersion': version, 'embeddedAppVersion': manifest['embeddedVersion'],
                  'releaseSourceCommit': run('git', 'rev-parse', tag),
                  'stagingSourceCommit': run('git', 'rev-parse', 'HEAD'),
                  'applicationRebuilt': False, 'macosInstallerRepackaged': True,
                  'macos': mac, **builds, 'reusedBundledToolSources': sources,
                  'runtime': json.loads((ROOT / 'runtime-manifest.json').read_text())}
    (output / 'BUILD-INFO.json').write_text(json.dumps(build_info, indent=2) + '\n')
    hashes = {p.name: sha256(p) for p in sorted(output.iterdir()) if p.is_file() and p.name != 'SHA256SUMS.txt'}
    (output / 'SHA256SUMS.txt').write_text(''.join(f'{digest}  {name}\n' for name, digest in hashes.items()))
    hashes['SHA256SUMS.txt'] = sha256(output / 'SHA256SUMS.txt')
    # Mac was uploaded locally. Transfer the other files entirely inside GitHub.
    for name in hashes:
        if name != mac['asset']:
            print(f'Uploading {name}', flush=True)
            run('gh', 'release', 'upload', tag, str(output / name), '--clobber')
    assets = json.loads(run('gh', 'api', f'repos/{repo}/releases/{release["id"]}/assets?per_page=100'))
    published = {a['name']: a for a in assets}
    if set(published) != set(hashes):
        raise RuntimeError('Draft assets do not match the staged file list')
    for name, digest in hashes.items():
        if published[name].get('digest') != 'sha256:' + digest:
            raise RuntimeError(f'GitHub asset digest mismatch: {name}')
    print(f'Verified {len(hashes)} draft assets. Ready for publication.', flush=True)


if __name__ == '__main__':
    main()
