#!/usr/bin/env python3
"""Fail closed on inherited fork credentials/configuration; never read private keys."""
import argparse
import base64
import hashlib
import os
from pathlib import Path
import re
from release_versions import REPOSITORY

ROOT = Path(__file__).resolve().parents[1]
INHERITED_UPDATER_KEY_SHA256 = '290dfdd3bee63188081bf58724ad2299cec2de1d5d6f76b9851ed7377a33a10c'


def check(*, updater=False, apple=False, postgres=False, environ=None, root=ROOT):
    env = os.environ if environ is None else environ
    if env.get('GITHUB_REPOSITORY', REPOSITORY) != REPOSITORY:
        raise ValueError('Release checkout must belong to ' + REPOSITORY)
    if updater:
        key = (root / 'packaging/updater.pub').read_bytes()
        try:
            decoded = base64.b64decode(key.strip(), validate=True).decode('ascii').splitlines()
            if len(decoded) != 2 or not decoded[0].startswith('untrusted comment:'):
                raise ValueError()
            material = base64.b64decode(decoded[1], validate=True)
            if len(material) != 42 or material[:2] != b'Ed':
                raise ValueError()
        except (ValueError, UnicodeError):
            raise ValueError('packaging/updater.pub must contain a Tauri minisign public key') from None
        if hashlib.sha256(material).hexdigest() == INHERITED_UPDATER_KEY_SHA256:
            raise ValueError('Replace packaging/updater.pub with your own updater public key before releasing')
    if apple:
        team = env.get('FAIRWAY_APPLE_TEAM_ID', '')
        if not re.fullmatch(r'[A-Z0-9]{10}', team) or team == '4Q27NQ2UAF':
            raise ValueError('Set FAIRWAY_APPLE_TEAM_ID to your own Apple Developer team ID')
        for name, kind in [('APPLE_SIGNING_IDENTITY', 'Application'), ('APPLE_INSTALLER_IDENTITY', 'Installer')]:
            value = env.get(name, '')
            if not value.startswith('Developer ID ' + kind + ': ') or not value.endswith('(' + team + ')') or '\n' in value or '\r' in value:
                raise ValueError('Set ' + name + ' to your matching Developer ID identity')
    if postgres and not re.fullmatch(r'[1-9][0-9]*', env.get('FAIRWAY_POSTGRES_RUNTIME_RUN', '')):
        raise ValueError('Set FAIRWAY_POSTGRES_RUNTIME_RUN to the reviewed signed PostgreSQL runtime workflow run ID')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('updater', 'apple', 'postgres'):
        parser.add_argument('--' + name, action='store_true')
    args = parser.parse_args()
    try:
        check(**vars(args))
    except ValueError as error:
        parser.error(str(error))
    print('Fork release configuration checked. Private credentials are checked only by their signing jobs.')


if __name__ == '__main__':
    main()
