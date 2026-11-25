#!/usr/bin/env python3
"""Read-only Summit check using Fairway's native explorer, never the UI simulator.

Build postgresql_external_smoke first. Reads only literal POSTGRES_USER and
POSTGRES_PASSWORD from a dotenv file (never sources it or exports its contents).
No credentials, account values or full server diagnostics are printed or saved.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def credentials(path):
    result = {}
    for line in path.read_text().splitlines():
        match = re.match(r'^\s*(?:export\s+)?(POSTGRES_USER|POSTGRES_PASSWORD)\s*=\s*(.*)$', line)
        if not match:
            continue
        key, value = match.groups()
        if value.startswith(("'", '"')):
            # Literal single-line dotenv values only; reject ambiguous syntax.
            quote = value[0]
            if len(value) < 2 or value[-1] != quote:
                raise ValueError('Unsupported credential quoting')
            value = value[1:-1]
            if quote == '"':
                if '\\' in value:
                    raise ValueError('Escaped credential values require manual input')
        else:
            value = re.split(r'\s+#', value, maxsplit=1)[0].rstrip()
        if '${' in value or '\x00' in value or key in result:
            raise ValueError('Ambiguous credential value')
        result[key] = value
    if not result.get('POSTGRES_USER') or 'POSTGRES_PASSWORD' not in result:
        raise ValueError('Required literal PostgreSQL credentials unavailable')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--credentials-file', type=Path, required=True)
    args = parser.parse_args()
    secret = credentials(args.credentials_file)
    request = {
        'connection': {
            'confirmed': True, 'database': 'vector_one_auth',
            'target': {'kind': 'external', 'host': '127.0.0.1', 'port': 55432,
                       'user': secret['POSTGRES_USER'], 'password': secret['POSTGRES_PASSWORD'],
                       'transport': 'loopback'},
        },
        'schema': 'vector_one', 'relation': 'app_user',
    }
    with tempfile.TemporaryDirectory(prefix='fw-pg-external-') as scratch:
        completed = subprocess.run([
            str(ROOT / 'src-tauri/target/debug/examples/postgresql_external_smoke'),
            str(ROOT / 'src-tauri/resources/runtime'), str(Path(scratch) / 'profile'),
        ], input=json.dumps(request), text=True, timeout=90, check=False)
    return completed.returncode


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError):
        raise SystemExit('Native external check could not complete; sensitive details withheld') from None
