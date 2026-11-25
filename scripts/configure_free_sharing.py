#!/usr/bin/env python3
"""Prepare a compile-time pin and updater-free overlay for a free Mac sharing build.

No private credentials, source scripts, services, Keychain writes or tracked pin
updates. PostgreSQL's exact tree and native code are verified before pinning.
"""
import argparse
import json
from pathlib import Path
import platform
from postgresql_release import ROOT, new_stage, publish, verify_development_code, verify_payload, write


def configure(payload):
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        raise ValueError('Mac sharing builds require an Apple Silicon builder')
    payload = payload.resolve(strict=True)
    _, pin = verify_payload(payload, bundled=True)
    verify_development_code(payload)
    stage, output = new_stage(ROOT / 'artifacts/free-sharing')
    write(stage / 'runtime-pin.json', pin)
    write(stage / 'tauri.json', {
        'bundle': {'resources': [], 'createUpdaterArtifacts': False},
        'build': {'beforeBuildCommand': ''},
    })
    publish(stage, output)
    print('Prepared free sharing pin and build overlay. The committed release pin is unchanged.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--payload', type=Path, default=ROOT / 'src-tauri/resources/runtime/postgresql')
    configure(parser.parse_args().payload)
