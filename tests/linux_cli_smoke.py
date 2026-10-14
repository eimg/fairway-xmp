#!/usr/bin/env python3
"""Exercise the installed CLI/supervisor/TUI against the real bundled runtime."""
import json
import os
from pathlib import Path
import pty
import signal
import socket
import subprocess
import tempfile
import termios
import time
import urllib.request


def main(installer):
    with tempfile.TemporaryDirectory(prefix='fw-linux-') as directory:
        root = Path(directory)
        home = root / "student's home ä"
        home.mkdir()
        prefix = home / 'installation'
        data = home / '.local/share/dev.fairway.environment'
        env = {**os.environ, 'HOME': str(home), 'XDG_DATA_HOME': str(home / '.local/share'), 'ZDOTDIR': str(home), 'TERM': 'xterm-256color'}
        subprocess.run(['sh', str(installer), '--prefix', str(prefix)], env=env, check=True)
        cli = prefix / 'current/bin/fairway'
        def call(*args, check=True):
            result = subprocess.run([str(cli), *args], env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=240)
            if check and result.returncode: raise RuntimeError(result.stdout + result.stderr)
            return result
        try:
            call('shutdown')
            # Simultaneous first clients must share one supervisor.
            clients = [subprocess.Popen([str(cli), 'status', '--json'], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(4)]
            for client in clients:
                out, err = client.communicate(timeout=30)
                assert client.returncode == 0, err
                assert all(not s['running'] for s in json.loads(out)['services'])
            ports = []
            sockets = []
            for _ in range(4):
                s = socket.socket(); s.bind(('127.0.0.1', 0)); ports.append(s.getsockname()[1]); sockets.append(s)
            for s in sockets: s.close()
            call('ports', *map(str, ports))
            call('php', '-r', 'echo PHP_VERSION;')
            assert 'Composer version' in call('composer', '--version').stdout
            # Q must queue shutdown while first-time initialization is still busy.
            master, slave = pty.openpty()
            original = termios.tcgetattr(slave)
            proc = subprocess.Popen([str(cli)], env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
            import select
            rendered = b''
            deadline = time.monotonic() + 15
            while b'phpMyAdmin:' not in rendered and time.monotonic() < deadline:
                if select.select([master], [], [], .2)[0]: rendered += os.read(master, 65536)
            assert b'Fairway XMP' in rendered, rendered
            os.write(master, b'a')
            deadline = time.monotonic() + 15
            while b'Working' not in rendered and time.monotonic() < deadline:
                if select.select([master], [], [], .2)[0]: rendered += os.read(master, 65536)
            assert b'Working' in rendered, rendered
            os.write(master, b'Q')
            deadline = time.monotonic() + 120
            while proc.poll() is None and time.monotonic() < deadline:
                if select.select([master], [], [], .2)[0]: rendered += os.read(master, 65536)
            assert proc.wait(timeout=5) == 0, rendered[-8192:]
            assert termios.tcgetattr(slave) == original
            os.close(master); os.close(slave)
            assert all(not s['running'] for s in json.loads(call('status', '--json').stdout)['services'])
            call('start')
            status = json.loads(call('status', '--json').stdout)
            assert all(s['ready'] for s in status['services']), status
            assert call('ports', *map(str, ports), check=False).returncode != 0
            assert call('start', 'php', check=False).returncode != 0
            # GUI-free terminal interaction: status is rendered, q detaches, terminal modes restored.
            master, slave = pty.openpty()
            original = termios.tcgetattr(slave)
            proc = subprocess.Popen([str(cli)], env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
            import select
            rendered = b''
            deadline = time.monotonic() + 15
            while b'phpMyAdmin:' not in rendered and time.monotonic() < deadline:
                if select.select([master], [], [], .2)[0]: rendered += os.read(master, 65536)
            assert b'Fairway XMP' in rendered and b'Ready' in rendered, rendered
            os.write(master, b'jkq')
            assert proc.wait(timeout=15) == 0
            assert termios.tcgetattr(slave) == original
            os.close(master); os.close(slave)
            assert all(s['ready'] for s in json.loads(call('status', '--json').stdout)['services'])
            website = ports[0]
            assert b'Your environment' in urllib.request.urlopen(f'http://127.0.0.1:{website}/', timeout=15).read()
            probe = Path(status['documentRoot']) / 'linux-smoke.php'
            probe.write_text(f"<?php $pdo = new PDO('mysql:host=127.0.0.1;port={ports[2]};dbname=fairway','fairway','fairway'); $pdo->exec('CREATE TABLE IF NOT EXISTS linux_smoke (id INT PRIMARY KEY)'); $pdo->exec('REPLACE INTO linux_smoke VALUES (42)'); echo $pdo->query('SELECT id FROM linux_smoke')->fetchColumn();")
            assert urllib.request.urlopen(f'http://127.0.0.1:{website}/linux-smoke.php', timeout=15).read().strip() == b'42'
            backup = Path(call('backup').stdout.strip())
            assert 'linux_smoke' in backup.read_text()
            # Reject commands from another installation before any mutation.
            connection = socket.socket(socket.AF_UNIX)
            connection.connect(str(data / 'supervisor.sock'))
            connection.sendall(json.dumps({'runtime': '/another-install/runtime', 'request': {'command': 'stop', 'service': 'all'}}).encode() + b'\n')
            response = connection.makefile('rb').readline()
            assert json.loads(response)['error']
            connection.close()
            assert all(s['ready'] for s in json.loads(call('status', '--json').stdout)['services'])
            # Recover the selected older PHP-FPM, not the default PHP executable.
            call('shutdown')
            call('php-version', '8.0')
            assert call('php', '-r', 'echo PHP_VERSION;').stdout.strip().startswith('8.0.')
            assert 'Composer version' in call('composer', '--version').stdout
            call('start')
            probe.write_text('<?php echo PHP_VERSION;')
            assert urllib.request.urlopen(f'http://127.0.0.1:{website}/linux-smoke.php', timeout=15).read().startswith(b'8.0.')
            php_pid = int((data / 'php.pid').read_text())
            # Recover verified MySQL, Nginx and PHP-FPM after a supervisor crash.
            pid = int((data / 'supervisor.lock').read_text())
            os.kill(pid, signal.SIGKILL)
            for _ in range(100):
                try: os.kill(pid, 0)
                except ProcessLookupError: break
                time.sleep(.05)
            recovered = json.loads(call('status', '--json').stdout)
            assert all(s['ready'] for s in recovered['services']), recovered
            assert recovered['phpVersion'] == '8.0', recovered
            call('shutdown')
            for _ in range(100):
                state = Path(f'/proc/{php_pid}/stat')
                if not state.exists() or state.read_text().split(') ', 1)[1].startswith('Z'): break
                time.sleep(.05)
            state = Path(f'/proc/{php_pid}/stat')
            assert not state.exists() or state.read_text().split(') ', 1)[1].startswith('Z'), 'Recovered PHP-FPM survived shutdown'
            with socket.socket() as listener_probe:
                assert listener_probe.connect_ex(('127.0.0.1', ports[3])) != 0, 'PHP-FPM is still listening after shutdown'
            with socket.socket() as freed:
                # Accepted FastCGI connections can remain in TIME_WAIT after
                # the listener exits; allow their port to be reused.
                freed.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                freed.bind(('127.0.0.1', ports[3]))
            call('start')
            assert urllib.request.urlopen(f'http://127.0.0.1:{website}/linux-smoke.php', timeout=15).read().startswith(b'8.0.')
            call('stop', 'nginx')
            stopped = json.loads(call('status', '--json').stdout)
            assert all(not s['running'] for s in stopped['services'] if s['id'] in ('nginx', 'php', 'phpmyadmin'))
            assert next(s for s in stopped['services'] if s['id'] == 'mysql')['ready']
            call('start', 'nginx')
            # Reinstall follows the backup/shutdown path and preserves settings and coursework.
            subprocess.run(['sh', str(installer), '--prefix', str(prefix)], env=env, check=True)
            assert probe.exists() and json.loads(call('ports').stdout)['website'] == ports[0]
            call('start')
            assert urllib.request.urlopen(f'http://127.0.0.1:{website}/linux-smoke.php', timeout=15).read().startswith(b'8.0.')
            call('uninstall')
            assert not prefix.exists()
            assert probe.exists() and backup.exists() and (data / 'mysql/mysql').is_dir()
            assert not (data / 'supervisor.sock').exists()
            assert not (data / 'bin/fairway').exists()
            for profile in ('.bashrc', '.bash_profile', '.zshrc', '.zprofile'):
                if (home / profile).exists(): assert '>>> fairway' not in (home / profile).read_text()
            print('PASS: offline install, concurrent clients, PHP/Composer, TUI detach, PHP → MySQL → HTTP, backup, crash recovery, port protection, reinstall and data-preserving uninstall.')
        finally:
            if cli.exists(): call('shutdown', check=False)


if __name__ == '__main__':
    import sys
    main(Path(sys.argv[1]).resolve())
