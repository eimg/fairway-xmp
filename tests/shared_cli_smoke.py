#!/usr/bin/env python3
"""Real runtime/CLI/controller smoke, isolated through debug-only data paths.

Never installs PATH integration, modifies hosts, or changes certificate trust.
Pass a debug desktop executable as --peer to test both native entry points.
"""
import argparse
import concurrent.futures
import json
import os
from pathlib import Path
import socket
import shutil
import tempfile
import urllib.request

from smoke_process import SmokeCommands


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--peer', type=Path)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, help='Keep command and service diagnostics here')
    args = parser.parse_args()
    cli = args.cli.resolve()
    peer = (args.peer or args.cli).resolve()
    with tempfile.TemporaryDirectory(prefix='fw-shared-') as temporary:
        root = Path(temporary)
        output = (args.output or root / 'diagnostics').resolve()
        with SmokeCommands(output) as commands:
            home = root / "student's home 日本"
            data = root / 'student data 日本'
            env = {**os.environ, 'FAIRWAY_DEV_HOME': str(home),
                   'FAIRWAY_DEV_DATA': str(data), 'FAIRWAY_RUNTIME_DIR': str(args.runtime.resolve())}

            def call(*words, executable=cli, check=True, timeout=240):
                result = commands.run([str(executable), *words], env=env, timeout=timeout)
                if check and result.returncode:
                    raise AssertionError(f'{words}: {result.stdout}\n{result.stderr}')
                return result

            def value(*words, executable=cli):
                return json.loads(call(*words, '--json', executable=executable).stdout)

            # Refuse a release binary: it ignores the isolated debug-only paths.
            for executable in {cli, peer}:
                assert json.loads(call('--build-info', executable=executable).stdout).get('debug') is True, 'Use debug binaries only'
            try:
                assert value('capabilities')['controller'] == 'shared-supervisor'
                with concurrent.futures.ThreadPoolExecutor(max_workers=4) as workers:
                    snapshots = list(workers.map(lambda _: value('status'), range(4)))
                assert all(Path(s['dataDir']).resolve() == data.resolve() for s in snapshots), 'Use debug binaries only'
                assert all(not s['running'] for s in snapshots[0]['services'])
                sockets = [socket.socket() for _ in range(6)]
                try:
                    for handle in sockets:
                        handle.bind(('127.0.0.1', 0))
                    ports = [handle.getsockname()[1] for handle in sockets]
                finally:
                    for handle in sockets:
                        handle.close()
                value('ports', *map(str, ports))
                assert value('ports', executable=peer)['website'] == ports[0]
                assert call('php', '-r', 'echo PHP_VERSION;').stdout.startswith('8.5.')
                assert 'Composer version' in call('composer', '--version').stdout
                value('start', 'all', '--wait')
                status = value('status', executable=peer)
                assert all(s['ready'] for s in status['services']), status
                assert [s['pid'] for s in status['services']] == [s['pid'] for s in value('status')['services']]
                probe = home / 'fairway/www/shared-cli.php'
                probe.write_text(f"<?php $db=new PDO('mysql:host=127.0.0.1;port={ports[2]};dbname=fairway','fairway','fairway'); echo $db->query('SELECT 42')->fetchColumn();", encoding='utf-8')
                assert urllib.request.urlopen(f'http://127.0.0.1:{ports[0]}/shared-cli.php', timeout=20).read() == b'42'
                value('db', 'create', 'cli_smoke')
                value('db', 'query', 'cli_smoke', 'CREATE TABLE coursework (id INT PRIMARY KEY); INSERT INTO coursework VALUES (42)')
                export = value('db', 'export', 'cli_smoke')
                assert Path(export['path']).is_file()
                value('db', 'import', 'cli_smoke', export['path'], '--yes')
                backup = Path(value('backup'))
                assert backup.is_file() and 'coursework' in backup.read_text(encoding='utf-8')
                # Agent mode saves a site and reports the approval handoff without touching hosts.
                site = call('site', 'add', 'fw-cli-smoke', str(probe.parent), '--index', '--non-interactive', '--json', check=False)
                assert site.returncode != 0 and json.loads(site.stdout)['error']['code'] == 'approval_required'
                assert value('site', 'list')[0]['domain'] == 'fw-cli-smoke'
                call('site', 'remove', 'fw-cli-smoke', '--non-interactive', '--json', check=False)
                assert value('site', 'list') == []
                value('stop', 'all', executable=peer)
                value('php-version', '8.2')
                assert call('php', '-r', 'echo PHP_VERSION;').stdout.startswith('8.2.')
                settings = root / 'php-settings.json'
                settings.write_text(json.dumps({'memoryLimitMb': 128, 'timezone': 'Asia/Yangon'}))
                changed = value('php-settings', '8.2', '--file', str(settings))
                assert changed['settings']['memoryLimitMb'] == 128
                assert value('php-settings', '8.2', '--reset')['customized'] is False
                value('shutdown')
                assert not (data / 'supervisor.pid').exists()
                assert all(not s['running'] for s in value('status', executable=peer)['services'])
                print('PASS: shared native CLI/controller, concurrent clients, HTTP/PDO, database tools, settings, approval handoff and restart')
            finally:
                try:
                    call('shutdown', check=False, timeout=30)
                finally:
                    if (data / 'logs').is_dir():
                        shutil.copytree(data / 'logs', output / 'services', dirs_exist_ok=True)


if __name__ == '__main__':
    main()
