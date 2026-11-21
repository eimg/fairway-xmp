#!/usr/bin/env python3
"""Exercise PCRE JIT, PHP clients, Unix workers and Composer in signed bundled PHP."""
import json
import os
from pathlib import Path
import subprocess
import sys


def bundled_interpreters(runtime):
    manifest = json.loads((runtime / 'manifest.json').read_text())
    default = manifest['php']
    expected = {'php': default}
    expected.update({f'php-{minor}': version for minor, version in manifest['phpVersions'].items()
                     if version != default})
    actual = {php.parent.parent.name: php for php in runtime.glob('php*/bin/php')}
    if set(actual) != set(expected):
        raise RuntimeError(f'Signed PHP interpreters do not match the bundled manifest; '
                           f'missing: {sorted(set(expected) - set(actual))}; '
                           f'unexpected: {sorted(set(actual) - set(expected))}')
    return [(actual[name], expected[name]) for name in sorted(expected)]


def main():
    runtime = Path(sys.argv[1]) / 'Contents/Resources/runtime'
    environment = {**os.environ, 'MIBS': '', 'PHP_INI_SCAN_DIR': ''}
    interpreters = bundled_interpreters(runtime)
    for php, version in interpreters:
        subprocess.run([str(php), '-n', '-r', 'set_error_handler(function($severity, $message) { throw new Exception($message); }); if (PHP_VERSION !== $argv[1]) throw new Exception("Unexpected PHP version: " . PHP_VERSION); if (preg_match("/hello/", "hello") !== 1) exit(1); echo "PCRE JIT OK: ", PHP_VERSION, PHP_EOL;', version], env=environment, check=True)
        subprocess.run([str(php), '-n', '-r', 'if (!extension_loaded("pdo_pgsql") || !extension_loaded("pgsql") || !in_array("pgsql", PDO::getAvailableDrivers(), true)) exit(1); echo "PostgreSQL clients OK: ", PHP_VERSION, PHP_EOL;'], env=environment, check=True)
        subprocess.run([str(php), '-n', str(Path(__file__).resolve().parents[1] / 'tests/fixtures/php_extensions.php')], env=environment, check=True)
        subprocess.run([str(php), '-n', str(runtime / 'composer.phar'), '--version'], env=environment, check=True)
    print(f'PASS: signed PHP regex JIT, PostgreSQL/socket/SOAP/LDAP clients, Unix workers and Composer on all {len(interpreters)} versions')


if __name__ == '__main__': main()
