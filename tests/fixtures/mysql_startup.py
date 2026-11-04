#!/usr/bin/env python3
"""Non-executing database fixture for Unix service-manager failure-path tests.

Copied under a temporary runtime as mysqld/mysql/mysqladmin. It models readiness
and authentication with files and never executes SQL or touches real user data.
"""
import configparser
import os
from pathlib import Path
import socket
import signal
import sys
import time


data = Path(__file__).resolve().parents[3] / 'student data'
control = data / 'tmp/fixture'
control.mkdir(exist_ok=True)
name = Path(__file__).name
args = sys.argv[1:]


def event(message):
    with (control / 'events').open('a') as stream:
        stream.write(message + '\n')


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


if name == 'nginx':
    if '-s' in args:
        fail('fixture nginx control failed')
    running = True
    def stop(*_):
        global running
        running = False
    signal.signal(signal.SIGQUIT, stop)
    (control / 'nginx-ready').touch()
    while running:
        time.sleep(.01)
    event('nginx-graceful-fallback')
    sys.exit(0)

if name == 'mysqld':
    if '--initialize' in args or '--initialize-insecure' in args:
        assert '--initialize' in args and '--initialize-insecure' not in args
        init = Path(next(arg.split('=', 1)[1] for arg in args if arg.startswith('--init-file=')))
        secret = (data / 'credentials').read_text()
        assert init.read_text() == f"ALTER USER 'root'@'localhost' IDENTIFIED BY '{secret}';\n"
        assert init.stat().st_mode & 0o777 == 0o600
        event('initialize-secure')
        if (control / 'fail-initialize').exists():
            fail('fixture initialization failure ' + secret)
        (data / 'mysql/mysql').mkdir()
        (control / 'root-secure').touch()
        sys.exit(0)
    event('spawn')
    stop = control / 'stop'
    stop.unlink(missing_ok=True)
    ready = control / 'ready'
    config = configparser.ConfigParser(interpolation=None)
    config.read(data / 'config/mysql.cnf')
    listener = None
    (data / 'mysql.pid').write_text(str(os.getpid()))
    try:
        while not stop.exists():
            if listener is None and not (control / 'never-ready').exists():
                listener = socket.socket()
                listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                listener.bind(('127.0.0.1', config.getint('mysqld', 'port')))
                listener.listen()
                listener.settimeout(.02)
                ready.touch()
            if listener:
                try:
                    connection, _ = listener.accept()
                    connection.close()
                except socket.timeout:
                    pass
            else:
                time.sleep(.01)
    finally:
        if listener:
            listener.close()
        ready.unlink(missing_ok=True)
        (data / 'mysql.pid').unlink(missing_ok=True)
    sys.exit(0)

assert '--no-login-paths' in args
secure = args[0].startswith('--defaults-file=')
if not secure:
    assert args[0] == '--no-defaults' and '--skip-password' in args
event(f'{name}:{"secure" if secure else "passwordless"}')
if not (control / 'ready').exists():
    fail('fixture server is not ready')
if secure != (control / 'root-secure').exists():
    fail('fixture authentication failed')
if name == 'mysqldump':
    if (control / 'hang-dump').exists():
        (control / 'dump-started').touch()
        deadline = time.monotonic() + 5
        while (control / 'hang-dump').exists() and time.monotonic() < deadline:
            time.sleep(.01)
    print('-- Student backup\nCREATE DATABASE fairway;')
    if (control / 'fail-dump').exists():
        fail('fixture dump failed')
    sys.exit(0)
if name == 'mysqladmin':
    assert args[-1] == 'shutdown'
    if (control / 'fail-shutdown').exists():
        fail('fixture shutdown failed')
    (control / 'stop').touch()
    sys.exit(0)
if name != 'mysql':
    fail('fixture PHP startup failed')
if '-e' in args:
    if (control / 'fail-list').exists():
        fail('fixture database listing failed')
    print('[]' if (control / 'empty-backup').exists() else '["fairway"]')
    sys.exit(0)
sql = sys.stdin.read()
if sql.startswith('ALTER USER'):
    event('alter-root')
    if (control / 'fail-alter').exists():
        fail('fixture root setup failed')
    (control / 'root-secure').touch()
elif sql.startswith('CREATE DATABASE'):
    event('provision')
    if (control / 'fail-provision').exists():
        fail('fixture teaching-account setup failed')
    (control / 'student-ready').touch()
else:
    assert sql == 'SELECT 1;'
