#!/usr/bin/env python3
"""Stage a self-contained, version-pinned native runtime. Never installs system software."""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / '.runtime-cache'
DEST = ROOT / 'src-tauri/resources/runtime'
MANIFEST = json.loads((ROOT / 'runtime-manifest.json').read_text())
LOCK_PATH = ROOT / 'runtime-downloads.lock.json'
HEADERS = {'User-Agent': 'Fairway-runtime-builder/0.1'}
RESUME_LOCK = False


def request(url, headers=None):
    return urllib.request.urlopen(urllib.request.Request(url, headers={**HEADERS, **(headers or {})}), timeout=90)


def read_json(url):
    for attempt in range(3):
        try:
            with request(url) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code not in (408, 429, 500, 502, 503, 504): raise
            if attempt == 2: raise
        except (OSError, urllib.error.URLError, http.client.HTTPException):
            if attempt == 2: raise
        print(f'Retrying interrupted metadata request ({attempt + 1}/2)', flush=True)
        time.sleep(2 * (attempt + 1))


def download(url, expected=None, algorithm='sha256', headers=None):
    CACHE.mkdir(exist_ok=True)
    target = CACHE / hashlib.sha256(url.encode()).hexdigest()
    if not target.exists():
        print(f'Downloading {url}', flush=True)
        partial = target.with_suffix('.partial')
        try:
            for attempt in range(3):
                try:
                    with request(url, headers) as response, partial.open('wb') as output:
                        shutil.copyfileobj(response, output)
                    partial.replace(target)
                    break
                except (OSError, urllib.error.URLError):
                    if attempt == 2: raise
                    print(f'Retrying interrupted download ({attempt + 1}/2)', flush=True)
                    time.sleep(2)
        finally:
            partial.unlink(missing_ok=True)
    with target.open('rb') as stream:
        digest = hashlib.file_digest(stream, algorithm).hexdigest()
    if expected and digest.lower() != expected.lower():
        target.unlink()
        raise RuntimeError(f'Checksum mismatch for {url}; refused to extract')
    if algorithm == 'sha256':
        return target, digest
    with target.open('rb') as stream:
        return target, hashlib.file_digest(stream, 'sha256').hexdigest()


def safe_extract(archive: Path, target: Path, kind: str, skip_external_links=False):
    """Reject traversal and archive links before unpacking third-party releases."""
    target.mkdir(parents=True, exist_ok=True)
    base = target.resolve()
    if kind == 'zip':
        with zipfile.ZipFile(archive) as bundle:
            for item in bundle.infolist():
                path = Path(item.filename.replace('\\', '/'))
                if path.is_absolute() or '..' in path.parts or ':' in item.filename:
                    raise ValueError('Unsafe archive path')
                if (item.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError('Archive symlinks are not allowed')
                if not (base / path).resolve().is_relative_to(base):
                    raise ValueError('Archive escapes destination')
            bundle.extractall(target)
    else:
        with tarfile.open(archive, 'r:*') as bundle:
            # Python's data filter handles harmless internal Homebrew symlinks,
            # rejects devices and links escaping the extraction root.
            def safe_filter(member, destination):
                try:
                    return tarfile.data_filter(member, destination)
                except (tarfile.LinkOutsideDestinationError, tarfile.AbsoluteLinkError):
                    # Some bottles contain optional links to Homebrew's global www
                    # folder. Never extract those links; Fairway has its own roots.
                    if skip_external_links and member.issym(): return None
                    raise
            bundle.extractall(target, filter=safe_filter)


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source.is_dir():
        shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=False)
    else:
        shutil.copy2(source, destination)


def lock_download(name, url, expected=None, algorithm='sha256', *, refresh=False, lock=None, headers=None):
    lock = lock if lock is not None else {}
    entry = lock.get(name)
    if entry and entry['url'] != url and not refresh:
        raise RuntimeError(f'{name} changed upstream; refresh and review the runtime download lock')
    checksum = None if refresh else (entry or {}).get('sha256')
    if not checksum and not refresh:
        raise RuntimeError(f'No reviewed checksum for {name}. Run --refresh-lock once, review it, then commit the lock.')
    path, sha = download(url, checksum or expected, 'sha256' if checksum else algorithm, headers)
    lock[name] = {'url': url, 'sha256': sha}
    return path


def common(target, lock, refresh):
    version = MANIFEST['composer']
    url = f'https://getcomposer.org/download/{version}/composer.phar'
    if lock.get('composer', {}).get('url') == url:
        sha = lock['composer']['sha256']
    else:
        with request(url + '.sha256sum') as response:
            sha = response.read().decode().split()[0]
    copy(lock_download('composer', url, sha, refresh=refresh, lock=lock), target / 'composer.phar')
    version = MANIFEST['phpmyadmin']
    url = f'https://files.phpmyadmin.net/phpMyAdmin/{version}/phpMyAdmin-{version}-all-languages.zip'
    if lock.get('phpmyadmin', {}).get('url') == url:
        sha = lock['phpmyadmin']['sha256']
    else:
        with request(url + '.sha256') as response:
            sha = response.read().decode().split()[0]
    archive = lock_download('phpmyadmin', url, sha, refresh=refresh, lock=lock)
    with tempfile.TemporaryDirectory() as folder:
        safe_extract(archive, Path(folder), 'zip')
        copy(Path(folder) / f'phpMyAdmin-{version}-all-languages', target / 'phpmyadmin')
    # Certifi packages Mozilla's trust roots and publishes immutable wheel URLs.
    # Pin the wheel rather than relying on a mutable certificate download URL.
    entry = lock.get('ca-certificates')
    if entry:
        url, sha = entry['url'], entry['sha256']
    else:
        package = read_json('https://pypi.org/pypi/certifi/json')
        wheel = next(file for file in package['urls'] if file['filename'].endswith('.whl'))
        url, sha = wheel['url'], wheel['digests']['sha256']
    archive = lock_download('ca-certificates', url, sha, refresh=refresh, lock=lock)
    with zipfile.ZipFile(archive) as wheel:
        (target / 'cacert.pem').write_bytes(wheel.read('certifi/cacert.pem'))
        for name in wheel.namelist():
            if 'LICENSE' in name.upper():
                path = target / 'licenses/ca-certificates' / Path(name).name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(wheel.read(name))


def mailpit(target, lock, refresh):
    """Stage Mailpit's official release build; it catches mail sent by local PHP apps."""
    version = MANIFEST['mailpit']
    asset, key = {
        'Darwin': ('darwin-arm64.tar.gz', 'mailpit-macos'),
        'Windows': ('windows-amd64.zip', 'mailpit-windows'),
        'Linux': ('linux-arm64.tar.gz', 'mailpit-linux-arm64') if platform.machine() in ('aarch64', 'arm64') else ('linux-amd64.tar.gz', 'mailpit-linux'),
    }[platform.system()]
    url = f'https://github.com/axllent/mailpit/releases/download/v{version}/mailpit-{asset}'
    expected = None
    if refresh:
        # GitHub publishes a SHA256 digest for each release asset.
        release = read_json(f'https://api.github.com/repos/axllent/mailpit/releases/tags/v{version}')
        digest = next(item['digest'] for item in release['assets'] if item['name'] == f'mailpit-{asset}')
        expected = digest.removeprefix('sha256:')
    archive = lock_download(key, url, expected, refresh=refresh, lock=lock)
    with tempfile.TemporaryDirectory() as folder:
        safe_extract(archive, Path(folder), 'zip' if asset.endswith('.zip') else 'tar')
        executable = 'mailpit.exe' if asset.endswith('.zip') else 'mailpit'
        copy(Path(folder) / executable, target / 'mailpit/bin' / executable)
        copy(Path(folder) / 'LICENSE', target / 'licenses/mailpit/LICENSE')


WINDOWS_PHP_TOOLSETS = {'8.2': 'vs16', '8.3': 'vs16', '8.4': 'vs17', '8.5': 'vs17', '8.6': 'vs18'}
VC_RUNTIME_URL = 'https://aka.ms/vc14/vc_redist.x64.exe'


def windows_php_url(version):
    minor = '.'.join(version.split('.')[:2])
    try:
        toolset = WINDOWS_PHP_TOOLSETS[minor]
    except KeyError:
        raise RuntimeError(f'No reviewed Windows toolset for PHP {minor}') from None
    return f'https://downloads.php.net/~windows/releases/php-{version}-nts-Win32-{toolset}-x64.zip'


def windows_php_sources(manifest):
    sources = {'php-windows': (windows_php_url(manifest['php']), 'php')}
    for minor, version in manifest.get('phpVersions', {}).items():
        if version != manifest['php']:
            sources[f'php-{minor}-windows'] = (windows_php_url(version), f'php-{minor}')
    return sources


def windows_vc_runtime(lock, refresh=False):
    # Historical upgrade fixtures keep their reviewed installer URL and checksum.
    url = lock.get('vc-runtime', {}).get('url', VC_RUNTIME_URL)
    return lock_download('vc-runtime', url, refresh=refresh, lock=lock)


def install_windows_prerequisites(lock, refresh=False):
    if platform.system() != 'Windows':
        raise RuntimeError('Windows prerequisites can only be installed on Windows')
    verified = windows_vc_runtime(lock, refresh)
    # The cache has extensionless names; ShellExecute needs an executable name.
    with tempfile.TemporaryDirectory(prefix='fairway-vc-') as folder:
        installer = Path(folder) / 'vc_redist.x64.exe'
        copy(verified, installer)
        subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-File',
                        str(ROOT / 'scripts/install_windows_vc_runtime.ps1'), '-Installer', str(installer)], check=True)


def macos_default_php_formula(lock, refresh=False):
    # Reuse the existing unversioned bottle while its pin matches the default.
    # New pins use a versioned formula so a Homebrew major switch cannot move it.
    stored = lock.get('brew-php-arm64-sequoia')
    if stored and stored['version'] == MANIFEST['php'] and (not refresh or RESUME_LOCK):
        return 'php'
    return 'php@' + '.'.join(MANIFEST['php'].split('.')[:2])


def windows(target, lock, refresh):
    mysql_series = '.'.join(MANIFEST['mysql'].split('.')[:2])
    sources = {
        **windows_php_sources(MANIFEST),
        'mysql-windows': (f'https://cdn.mysql.com/Downloads/MySQL-{mysql_series}/mysql-{MANIFEST["mysql"]}-winx64.zip', 'mysql'),
        'nginx-windows': (f'https://nginx.org/download/nginx-{MANIFEST["nginx"]}.zip', 'nginx'),
    }
    for name, (url, service) in sources.items():
        archive = lock_download(name, url, refresh=refresh, lock=lock)
        with tempfile.TemporaryDirectory() as folder:
            unpacked = Path(folder)
            safe_extract(archive, unpacked, 'zip')
            if service.startswith('php'):
                # DLLs must be alongside PHP executables; extensions remain separate.
                for file in unpacked.iterdir():
                    if file.suffix.lower() in ('.exe', '.dll'): copy(file, target / service / 'bin' / file.name)
                copy(unpacked / 'ext', target / service / 'ext')
                for file in unpacked.glob('*LICENSE*'): copy(file, target / 'licenses' / service / file.name)
                for file in unpacked.glob('*license*'): copy(file, target / 'licenses' / service / file.name)
            elif service == 'nginx':
                base = next(unpacked.iterdir())
                copy(base / 'nginx.exe', target / 'nginx/bin/nginx.exe')
                copy(base / 'conf/mime.types', target / 'nginx/conf/mime.types')
                for file in base.glob('*LICENSE*'): copy(file, target / 'licenses/nginx' / file.name)
            else:
                base = next(unpacked.iterdir())
                # PDB debug symbols and .lib linker inputs are not needed by the
                # shipped executables; together they add almost 900 MiB and can
                # push the NSIS data block past its size limit.
                for directory in ('bin', 'lib', 'share'):
                    shutil.copytree(base / directory, target / 'mysql' / directory,
                                    dirs_exist_ok=True, ignore=shutil.ignore_patterns('*.pdb', '*.lib'))
                for file in base.glob('*LICENSE*'): copy(file, target / 'licenses/mysql' / file.name)
    if 'fnm' in MANIFEST: fnm_runtime(target, lock, refresh, 'windows')
    # The Microsoft redistributable is installed by our NSIS hook if required.
    copy(windows_vc_runtime(lock, refresh), target / 'vc_redist.x64.exe')


class Bottles:
    """Download build inputs without installing Homebrew on the builder or student machine."""
    def __init__(self, work, lock, refresh):
        self.work, self.lock, self.refresh = work, lock, refresh
        self.kegs = {}
        self.metadata = {}
        self._indexed_kegs = set()
        self._files_by_name = {}

    def fetch(self, name, required_version=None):
        if name in self.kegs: return self.kegs[name]
        key = f'brew-{name}-arm64-sequoia'
        stored = self.lock.get(key)
        if stored and (not self.refresh or RESUME_LOCK):
            version, url, checksum = stored['version'], stored['url'], stored['sha256']
        else:
            data = read_json(f'https://formulae.brew.sh/api/formula/{urllib.parse.quote(name, safe="")}.json')
            self.metadata[name] = data
            version = data['versions']['stable']
            bottle = data['bottle']['stable']['files'].get('arm64_sequoia')
            if not bottle:
                raise RuntimeError(f'No macOS 15 ARM bottle for {name}; a source build is required')
            url, checksum = bottle['url'], bottle['sha256']
        if required_version and version != required_version:
            raise RuntimeError(f'{name}: manifest pins {required_version}, but input is {version}; review versions before refreshing')
        headers = None
        cached = CACHE / hashlib.sha256(url.encode()).hexdigest()
        if not cached.exists():
            repo = urllib.parse.urlsplit(url).path.split('/blobs/')[0].removeprefix('/v2/')
            token = read_json(f'https://ghcr.io/token?service=ghcr.io&scope=repository:{repo}:pull')['token']
            headers = {'Authorization': f'Bearer {token}'}
        archive = lock_download(key, url, checksum, refresh=self.refresh, lock=self.lock,
                                headers=headers)
        self.lock[key]['version'] = version
        safe_extract(archive, self.work, 'tar', skip_external_links=True)
        folder = self.work / name
        if not folder.exists():
            # Bottle formula directory occasionally differs from the API alias.
            folder = self.work / name.split('@')[0]
        kegs = [path for path in folder.iterdir() if path.is_dir()]
        if len(kegs) != 1: raise RuntimeError(f'Cannot identify extracted keg for {name}')
        self.kegs[name] = kegs[0]
        return kegs[0]

    def dependency(self, original):
        for prefix in ('/opt/homebrew/opt/', '/opt/homebrew/Cellar/'):
            if original.startswith(prefix):
                rest = original.removeprefix(prefix).split('/')
                formula = rest[0]
                keg = self.fetch(formula)
                suffix = rest[1:] if prefix.endswith('/opt/') else rest[2:]
                candidate = keg.joinpath(*suffix)
                if not candidate.exists(): raise RuntimeError(f'Missing dependency {original}')
                return candidate.resolve()
        raise RuntimeError(f'Unresolved non-system dependency {original}')

    def relative_dependency(self, dependency, origin=None):
        suffix = dependency.split('/', 1)[-1]
        if origin and (origin.parent / suffix).exists():
            return (origin.parent / suffix).resolve()
        name = Path(dependency).name
        # Index each bottle once. Repeating rglob for every load command is
        # prohibitively slow with the bundled PHP builds and MySQL's large share tree.
        for formula, keg in self.kegs.items():
            if formula not in self._indexed_kegs:
                for file in keg.rglob('*'):
                    if file.is_file():
                        self._files_by_name.setdefault(file.name, set()).add(file.resolve())
                self._indexed_kegs.add(formula)
        matches = self._files_by_name.get(name, set())
        if len(matches) != 1:
            raise RuntimeError(f'Cannot resolve relative library {dependency}: {len(matches)} candidates')
        return next(iter(matches))


def macho(path):
    if not path.is_file(): return False
    with path.open('rb') as file: magic = file.read(4)
    return magic in (b'\xcf\xfa\xed\xfe', b'\xce\xfa\xed\xfe', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca')


def relocate(target, bottles):
    libraries = target / 'lib'
    libraries.mkdir(exist_ok=True)
    queue = [path for path in target.rglob('*') if macho(path)]
    seen = set()
    sources = {}
    while queue:
        binary = queue.pop()
        if binary in seen: continue
        seen.add(binary)
        # Keep the existing signature until relocation finishes. Removing it
        # first can leave a __LINKEDIT layout older macOS tools cannot process.
        # Load-command edits invalidate it; the final ad-hoc signature replaces it.
        output = subprocess.check_output(['otool', '-L', str(binary)], text=True)
        for line in output.splitlines()[1:]:
            original = line.strip().split(' (')[0]
            dependency = original.replace('@@HOMEBREW_PREFIX@@', '/opt/homebrew').replace('@@HOMEBREW_CELLAR@@', '/opt/homebrew/Cellar')
            if dependency.startswith(('/usr/lib/', '/System/Library/')): continue
            if dependency.startswith('@loader_path/') and (binary.parent / dependency.removeprefix('@loader_path/')).exists():
                continue
            if dependency.startswith(('@loader_path/', '@rpath/')):
                source = bottles.relative_dependency(dependency, sources.get(binary))
            elif dependency.startswith('/opt/homebrew/'):
                source = bottles.dependency(dependency)
            else:
                raise RuntimeError(f'Unexpected library dependency: {dependency}')
            destination = libraries / source.name
            if destination in sources and sources[destination] != source:
                raise RuntimeError(f'Library name collision: {source.name}')
            if destination not in sources:
                copy(source, destination); sources[destination] = source; queue.append(destination)
            relative = os.path.relpath(destination, binary.parent)
            subprocess.run(['install_name_tool', '-change', original, '@loader_path/' + relative, str(binary)], check=True)
        if binary.suffix in ('.dylib', '.so'):
            subprocess.run(['install_name_tool', '-id', '@loader_path/' + binary.name, str(binary)], check=True)
        subprocess.run(['codesign', '--force', '--sign', '-', str(binary)], check=True, capture_output=True)
    # All load commands must now resolve without the builder's Homebrew tree.
    for binary in seen:
        output = subprocess.check_output(['otool', '-L', str(binary)], text=True)
        if '/opt/homebrew/' in output or '@@HOMEBREW_' in output:
            raise RuntimeError(f'Non-relocatable binary: {binary}')


def fnm_runtime(target, lock, refresh, system):
    """Ship upstream fnm unchanged. Node versions are installed by the user."""
    version = MANIFEST['fnm']
    archive = lock_download(f'fnm-{system}', f'https://github.com/Schniz/fnm/releases/download/v{version}/fnm-{system}.zip', refresh=refresh, lock=lock)
    source = lock_download('fnm-source', f'https://codeload.github.com/Schniz/fnm/tar.gz/refs/tags/v{version}', refresh=refresh, lock=lock)
    with tempfile.TemporaryDirectory() as folder:
        root = Path(folder)
        safe_extract(archive, root / 'binary', 'zip')
        name = 'fnm.exe' if system == 'windows' else 'fnm'
        copy(root / 'binary' / name, target / 'fnm/bin' / name)
        (target / 'fnm/bin' / name).chmod(0o755)
        safe_extract(source, root / 'source', 'tar')
        copy(root / f'source/fnm-{version}/LICENSE', target / 'licenses/fnm/LICENSE')


def macos(target, lock, refresh):
    if platform.machine() != 'arm64': raise RuntimeError('Only Apple Silicon builders are supported')
    with tempfile.TemporaryDirectory(prefix='fairway-bottles-') as folder:
        bottles = Bottles(Path(folder), lock, refresh)
        php = bottles.fetch(macos_default_php_formula(lock, refresh), MANIFEST['php'])
        mysql = bottles.fetch('mysql@9.7', MANIFEST['mysql'])
        php_builds = [('php', php)]
        for minor, version in MANIFEST.get('phpVersions', {}).items():
            if version != MANIFEST['php']:
                php_builds.append((f'php-{minor}', bottles.fetch(f'php@{minor}', version)))
        for folder, keg in php_builds:
            for executable in ('php', 'php-cgi'):
                copy(keg / 'bin' / executable, target / folder / 'bin' / executable)
            copy(keg / 'sbin/php-fpm', target / folder / 'bin/php-fpm')
            (target / folder / 'ext').mkdir(parents=True, exist_ok=True)
            for extension in keg.rglob('*.so'):
                if 'httpd' not in str(extension): copy(extension, target / folder / 'ext' / extension.name)
        for directory in ('bin', 'lib', 'share'): copy(mysql / directory, target / 'mysql' / directory)
        build_nginx(target, bottles, lock, refresh)
        relocate(target, bottles)
        for name, keg in bottles.kegs.items():
            for file in keg.rglob('*'):
                if file.is_file() and any(word in file.name.upper() for word in ('LICENSE', 'COPYING', 'NOTICE')):
                    copy(file, target / 'licenses' / name.replace('@', '-') / file.relative_to(keg))
        # Keep only bottles actually used by this Mac runtime, retaining Windows pins.
        for key in list(lock):
            if key.startswith('brew-') and key not in {
                f'brew-{name}-arm64-sequoia' for name in bottles.kegs
            }:
                del lock[key]
    # Upstream fnm is a universal, system-linked executable. Keep it out of the
    # Homebrew relocator, whose input binaries contain one ARM64 architecture.
    if 'fnm' in MANIFEST: fnm_runtime(target, lock, refresh, 'macos')


def build_nginx(target, bottles, lock, refresh):
    """Build upstream's stable branch with pinned libraries, without installing it."""
    version = MANIFEST['nginx']
    url = f'https://nginx.org/download/nginx-{version}.tar.gz'
    archive = lock_download('nginx-macos-source', url, refresh=refresh, lock=lock)
    pcre = bottles.fetch('pcre2')
    openssl = bottles.fetch('openssl@3')
    with tempfile.TemporaryDirectory(prefix='fairway-nginx-') as folder:
        safe_extract(archive, Path(folder), 'tar')
        source = Path(folder) / f'nginx-{version}'
        cc_options = ' '.join(f'-I{shlex.quote(str(keg / "include"))}' for keg in (pcre, openssl))
        ld_options = ' '.join(f'-L{shlex.quote(str(keg / "lib"))}' for keg in (pcre, openssl))
        args = [
            './configure', '--prefix=.', '--sbin-path=bin/nginx',
            '--with-http_ssl_module', '--with-http_v2_module',
            '--with-http_gzip_static_module', '--with-http_stub_status_module',
            '--with-pcre-jit', '--with-compat',
            f'--with-cc-opt={cc_options} -mmacosx-version-min=15.0',
            f'--with-ld-opt={ld_options} -mmacosx-version-min=15.0',
        ]
        env = {**os.environ, 'MACOSX_DEPLOYMENT_TARGET': '15.0'}
        subprocess.run(args, cwd=source, env=env, check=True)
        subprocess.run(['make', f'-j{min(os.cpu_count() or 2, 8)}'], cwd=source, env=env, check=True)
        copy(source / 'objs/nginx', target / 'nginx/bin/nginx')
        copy(source / 'conf/mime.types', target / 'nginx/conf/mime.types')
        copy(source / 'LICENSE', target / 'licenses/nginx/LICENSE')
        (target / 'nginx/build.json').write_text(json.dumps({
            'version': version, 'source': lock['nginx-macos-source'],
            'configure': args, 'deploymentTarget': '15.0',
            'libraries': {name: entry for name, entry in lock.items()
                          if name in ('brew-pcre2-arm64-sequoia', 'brew-openssl@3-arm64-sequoia')},
        }, indent=2) + '\n')


def required_php_extensions(manifest, system=None):
    system = system or platform.system()
    target = {'Darwin': 'macos', 'Linux': 'linux', 'Windows': 'windows'}[system]
    return set(manifest['extensions']) | set(manifest.get('platformExtensions', {}).get(target, []))


def verify(target):
    try:
        staged_manifest = json.loads((target / 'manifest.json').read_text())
    except (OSError, ValueError) as error:
        raise RuntimeError('The staged runtime manifest is missing or invalid. Run full runtime preparation before building.') from error
    if staged_manifest != MANIFEST:
        raise RuntimeError('The staged runtime manifest is out of date. Run full runtime preparation before building.')
    expected_php = {'php', *(f'php-{minor}' for minor, version in MANIFEST.get('phpVersions', {}).items() if version != MANIFEST['php'])}
    extra_php = sorted(path.name for path in target.glob('php-[0-9]*') if path.is_dir() and path.name not in expected_php)
    if extra_php:
        raise RuntimeError(f'The staged runtime contains unexpected PHP folders: {", ".join(extra_php)}. Run full runtime preparation before building.')
    suffix = '.exe' if platform.system() == 'Windows' else ''
    env = {**os.environ, 'PHP_INI_SCAN_DIR': '', 'MIBS': ''}
    code = 'echo json_encode(["version"=>PHP_VERSION,"extensions"=>get_loaded_extensions()]);'
    php_builds = {'php': MANIFEST['php']}
    php_builds.update({f'php-{minor}': version for minor, version in MANIFEST.get('phpVersions', {}).items() if version != MANIFEST['php']})
    for folder, version in php_builds.items():
        php = target / folder / 'bin' / ('php' + suffix)
        ini = target / f'verify-{folder}.ini'
        if suffix:
            ext = (target / folder / 'ext').as_posix()
            extensions = ('curl', 'fileinfo', 'gd', 'intl', 'ldap', 'mbstring', 'mysqli', 'openssl',
                          'pdo_mysql', 'pdo_pgsql', 'pdo_sqlite', 'pgsql', 'soap', 'sockets', 'sodium', 'sqlite3', 'zip')
            ini.write_text(f'extension_dir="{ext}"\n' + '\n'.join(
                f'extension={name}' for name in extensions
                if (target / folder / 'ext' / f'php_{name}.dll').is_file()))
        else: ini.write_text('')
        output = subprocess.check_output([str(php), '-c', str(ini), '-r', code], env=env, text=True)
        info = json.loads(output)
        missing = required_php_extensions(MANIFEST) - {item.lower() for item in info['extensions']}
        if info['version'] != version or missing:
            raise RuntimeError(f'PHP validation failed for {folder}: version={info["version"]}, missing={sorted(missing)}')
        ini.unlink()
    php = target / 'php/bin' / ('php' + suffix)
    ini = target / 'verify.ini'
    ini.write_text('')
    commands = [
        ([str(php), '-c', str(ini), str(target / 'composer.phar'), '--version'], f'Composer version {MANIFEST["composer"]} '),
        ([str(target / 'nginx/bin' / ('nginx' + suffix)), '-v'], f'nginx/{MANIFEST["nginx"]}'),
        ([str(target / 'mysql/bin' / ('mysqld' + suffix)), '--no-defaults', '--version'], f'Ver {MANIFEST["mysql"]} '),
    ]
    if 'mailpit' in MANIFEST:
        commands.append(([str(target / 'mailpit/bin' / ('mailpit' + suffix)), 'version', '--no-release-check'], f' v{MANIFEST["mailpit"]} '))
    if 'fnm' in MANIFEST:
        commands.append(([str(target / 'fnm/bin' / ('fnm' + suffix)), '--version'], f'fnm {MANIFEST["fnm"]}'))
    for command, expected in commands:
        output = subprocess.check_output(command, env=env, stderr=subprocess.STDOUT, text=True)
        if expected not in output:
            raise RuntimeError(f'Runtime version mismatch: expected {expected!r}, received {output.strip()!r}')
        print(output.strip(), flush=True)
    ini.unlink()


def main():
    global RESUME_LOCK, MANIFEST, LOCK_PATH, DEST
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--refresh-lock', action='store_true', help='Trust upstream HTTPS once; review and commit resulting SHA256 lock before release')
    parser.add_argument('--resume-lock', action='store_true', help='Resume interrupted preparation, retaining existing pins and recording missing inputs')
    parser.add_argument('--verify-only', action='store_true')
    parser.add_argument('--windows-prerequisites', action='store_true', help='Install the pinned VC++ runtime before Windows staging executes PHP')
    parser.add_argument('--manifest', type=Path, help='Version manifest for an isolated historical runtime')
    parser.add_argument('--lockfile', type=Path, help='Download lock for an isolated historical runtime')
    parser.add_argument('--destination', type=Path, help='Stage into a separate runtime folder')
    args = parser.parse_args()
    if args.windows_prerequisites and args.verify_only:
        parser.error('--windows-prerequisites cannot be combined with --verify-only')
    if args.manifest: MANIFEST = json.loads(args.manifest.read_text())
    if args.lockfile: LOCK_PATH = args.lockfile.resolve()
    if args.destination: DEST = args.destination.resolve()
    RESUME_LOCK = args.resume_lock
    refresh = args.refresh_lock or args.resume_lock
    if args.verify_only: verify(DEST); return
    lock = json.loads(LOCK_PATH.read_text()) if LOCK_PATH.exists() else {}
    if args.windows_prerequisites:
        install_windows_prerequisites(lock, refresh)
        if refresh: LOCK_PATH.write_text(json.dumps(lock, indent=2, sort_keys=True) + '\n')
        return
    work = ROOT / '.runtime-work/staged'
    if work.exists(): shutil.rmtree(work)
    work.mkdir(parents=True)
    try:
        common(work, lock, refresh)
        # Historical runtimes (such as the alpha 1 upgrade fixture) predate Mailpit.
        if 'mailpit' in MANIFEST: mailpit(work, lock, refresh)
        if platform.system() == 'Darwin': macos(work, lock, refresh)
        elif platform.system() == 'Windows': windows(work, lock, refresh)
        elif platform.system() == 'Linux':
            from prepare_linux_runtime import linux
            linux(work, lock, refresh)
        else: raise RuntimeError('Runtime packages are supported only on Windows, Apple Silicon macOS and Linux x86_64')
        (work / 'manifest.json').write_text(json.dumps(MANIFEST, indent=2) + '\n')
        copy(ROOT / 'THIRD_PARTY_NOTICES.md', work / 'THIRD_PARTY_NOTICES.md')
        verify(work)
        if DEST.exists(): shutil.rmtree(DEST)
        shutil.move(work, DEST)
        (DEST / '.gitkeep').touch()
        if refresh: LOCK_PATH.write_text(json.dumps(lock, indent=2, sort_keys=True) + '\n')
        print(f'Runtime prepared at {DEST}', flush=True)
    except Exception:
        # Retain verified download records so interrupted staging can resume.
        if refresh: LOCK_PATH.write_text(json.dumps(lock, indent=2, sort_keys=True) + '\n')
        raise


if __name__ == '__main__':
    # Linux runtime helpers import this module. Share the same overridden
    # manifest and lock settings instead of loading a second copy of the script.
    import sys
    sys.modules['prepare_runtime'] = sys.modules[__name__]
    main()
