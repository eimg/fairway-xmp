#!/usr/bin/env python3
"""Create update metadata pointing only at verified versioned release assets."""
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import quote
from release_candidate import verify_files
from release_versions import version_key


def generate(manifest, directories, repo, notes, published_at=None):
    version = manifest['version']
    version_key(version)
    if repo != 'eimg/fairway-xmp': raise ValueError('Unexpected release repository')
    if manifest.get('signing') != {'macos-arm64': 'apple-notarized', 'windows-x64': 'updater-only', 'linux-x86_64': 'unsigned'}:
        raise ValueError('Missing expected release signing policy')
    platforms = {}
    for target, platform in [('macos-arm64', 'darwin-aarch64'), ('windows-x64', 'windows-x86_64')]:
        candidate = manifest['candidates'][target]
        if candidate['version'] != version or candidate['commit'] != manifest['commit'] or candidate['lifecycle'].get('updater', {}).get('status') != 'passed':
            raise ValueError('Updater identity or native lifecycle evidence does not match')
        directory = Path(directories[target])
        verify_files(candidate, directory)
        artifact, signature = candidate['updater']['artifact'], candidate['updater']['signature']
        if signature != artifact + '.sig' or artifact not in candidate['files'] or signature not in candidate['files']:
            raise ValueError('Updater signature is not recorded against the tested artifact')
        platforms[platform] = {
            'url': f'https://github.com/{repo}/releases/download/v{version}/{quote(artifact)}',
            'signature': (directory / signature).read_text().strip(),
        }
        if not platforms[platform]['signature']: raise ValueError('Empty updater signature')
    return {'version': version, 'notes': notes, 'pub_date': published_at or datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z'), 'platforms': platforms}
