#!/usr/bin/env python3
"""Fetch the existing public alpha 3 installer and verify its published checksum."""
import argparse
import hashlib
from pathlib import Path
import re
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('platform', choices=['windows', 'macos', 'linux'])
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    suffix = {'windows': 'windows-x64-setup.exe', 'macos': 'macos-arm64-unsigned.pkg', 'linux': 'linux-x86_64.run'}[args.platform]
    name = 'Fairway-XMP-0.1.0-alpha.3-' + suffix
    base = 'https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/'
    with urllib.request.urlopen(base + 'SHA256SUMS.txt', timeout=60) as response:
        sums = response.read(1024 * 1024).decode('utf-8-sig')
    rows = [line.split(maxsplit=1) for line in sums.splitlines() if line.strip()]
    matches = [row[0] for row in rows if len(row) == 2 and row[1].lstrip('*').strip() == name]
    expected = matches[0] if len(matches) == 1 else None
    if not expected or not re.fullmatch('[0-9a-fA-F]{64}', expected):
        raise ValueError('Baseline installer has no published SHA256')
    path = Path(args.output)
    path.parent.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256()
    try:
        with urllib.request.urlopen(base + name, timeout=120) as response, path.open('wb') as stream:
            while chunk := response.read(1024 * 1024):
                digest.update(chunk); stream.write(chunk)
        if digest.hexdigest() != expected.lower():
            raise ValueError('Baseline installer checksum mismatch')
    except BaseException:
        path.unlink(missing_ok=True)
        raise
    print(path)


if __name__ == '__main__': main()
