"""Version ordering and update channels for numbered beta, RC and stable releases."""
import re

NUMBER = r'(0|[1-9][0-9]*)'
VERSION = re.compile(rf'{NUMBER}\.{NUMBER}\.{NUMBER}(?:-(beta|rc)\.{NUMBER})?')
REPOSITORY = 'RizzRioo06/fairway-xmp'


def version_key(value):
    match = VERSION.fullmatch(value)
    if not match:
        raise ValueError('Expected MAJOR.MINOR.PATCH, MAJOR.MINOR.PATCH-beta.N or MAJOR.MINOR.PATCH-rc.N')
    major, minor, patch, stage, number = match.groups()
    return (int(major), int(minor), int(patch), {'beta': 0, 'rc': 1, None: 2}[stage], int(number or 0))


def channel(value):
    return ('beta', 'rc', 'stable')[version_key(value)[3]]


def update_endpoint(value):
    feed = 'stable-feed' if channel(value) == 'stable' else 'beta-feed'
    return f'https://github.com/{REPOSITORY}/releases/download/{feed}/latest.json'
