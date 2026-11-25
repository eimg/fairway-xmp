#!/usr/bin/env python3
"""Promote immutable release metadata without moving an update feed backwards."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
from release_versions import REPOSITORY, channel, version_key


def promote_feed(repo, tag, root, feed_tag):
    if repo != REPOSITORY:
        raise ValueError('Feed promotion is restricted to the personal fork')
    version = tag.removeprefix('v')
    release_channel = channel(version)
    if feed_tag not in ('beta-feed', 'stable-feed') or (feed_tag == 'stable-feed' and release_channel != 'stable'):
        raise ValueError('Only stable releases may enter the stable feed')
    existing = subprocess.run(['gh', 'release', 'view', feed_tag, '--repo', repo], capture_output=True)
    if existing.returncode != 0:
        # Distinguish a missing release from auth, rate-limit and network errors.
        releases = json.loads(subprocess.check_output(['gh', 'api', f'repos/{repo}/releases?per_page=100'], text=True))
        if any(release['tag_name'] == feed_tag for release in releases):
            raise ValueError(f'Could not read the existing {feed_tag} release')
        subprocess.check_call(['gh', 'release', 'create', feed_tag, '--repo', repo, '--prerelease', '--latest=false',
            '--title', 'Stable update feed' if feed_tag == 'stable-feed' else 'Preview update feed',
            '--notes', 'Signed update metadata. Download installers from numbered Fairway XMP releases.', str(root / 'latest.json')])
        return
    previous = root / feed_tag
    previous.mkdir()
    subprocess.check_call(['gh', 'release', 'download', feed_tag, '--repo', repo, '--pattern', 'latest.json', '--dir', str(previous)])
    old = json.loads((previous / 'latest.json').read_text())
    if version_key(old['version']) > version_key(version):
        if feed_tag == 'beta-feed' and release_channel == 'stable':
            print(f'Keeping newer preview {old["version"]} in {feed_tag}.')
            return
        raise ValueError(f'Refusing to move {feed_tag} backwards')
    subprocess.check_call(['gh', 'release', 'upload', feed_tag, str(root / 'latest.json'), '--repo', repo, '--clobber'])


def main():
    repo, tag = os.environ['GH_REPO'], os.environ['FAIRWAY_TAG']
    if repo != REPOSITORY or not tag.startswith('v'):
        raise ValueError('Unexpected repository or release tag')
    version = tag.removeprefix('v')
    prerelease = channel(version) != 'stable'
    release = json.loads(subprocess.check_output(['gh', 'release', 'view', tag, '--repo', repo, '--json', 'isDraft,isPrerelease'], text=True))
    if release['isDraft'] or release['isPrerelease'] != prerelease:
        raise ValueError('Publish the versioned release with the correct prerelease status before promoting its feed')
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        subprocess.check_call(['gh', 'release', 'download', tag, '--repo', repo, '--pattern', 'latest.json', '--dir', str(root)])
        feed = json.loads((root / 'latest.json').read_text())
        if feed['version'] != version or set(feed['platforms']) != {'darwin-aarch64', 'windows-x86_64'}:
            raise ValueError('Published update metadata does not match the requested release')
        for platform in feed['platforms'].values():
            if not platform['url'].startswith(f'https://github.com/{repo}/releases/download/{tag}/') or not platform['signature']:
                raise ValueError('Unexpected updater asset URL or missing signature')
        for feed_tag in (['beta-feed'] if prerelease else ['stable-feed', 'beta-feed']):
            promote_feed(repo, tag, root, feed_tag)
    print(f'Update feed promotion complete for {tag}.')


if __name__ == '__main__': main()
