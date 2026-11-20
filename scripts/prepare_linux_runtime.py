#!/usr/bin/env python3
"""Build the Linux runtime on Ubuntu 22.04 and relocate its native library closure."""
from __future__ import annotations
import hashlib
import concurrent.futures
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
from prepare_runtime import ROOT, MANIFEST, copy, lock_download, required_php_extensions, safe_extract, fnm_runtime, download

GLIBC_LIBS = {'libc.so.6', 'libm.so.6', 'libpthread.so.0', 'libdl.so.2', 'librt.so.1', 'libresolv.so.2', 'libutil.so.1', 'ld-linux-x86-64.so.2', 'ld-linux-aarch64.so.1'}


def run(args, **kwargs):
    print(' '.join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), check=True, **kwargs)


def elf(path):
    try:
        with path.open('rb') as stream: return stream.read(4) == b'\x7fELF'
    except (IsADirectoryError, FileNotFoundError): return False


def validate_glibc(versions):
    required = [tuple(map(int, match.split('.'))) for match in re.findall(r'\bGLIBC_(\d+\.\d+)', versions)]
    if required and max(required) > (2, 35):
        raise RuntimeError(f'Runtime requires glibc {".".join(map(str, max(required)))}; build on the glibc 2.35 baseline')


def bundle_component_libraries(target):
    # Keep MySQL's upstream OpenSSL isolated from PHP/Nginx's build libraries.
    library_dir = target / '.fairway-lib'
    library_dir.mkdir(exist_ok=True)
    queue = [file for file in target.rglob('*') if file.is_file() and not file.is_symlink() and elf(file)]
    processed = set()
    packages = set()
    # Prefer the component's own libraries for every transitive dependency.
    # Oracle ships newer Kerberos/OpenSSL than the build distro; mixing providers
    # with the same SONAME is unsafe even when they appear to link successfully.
    local_dirs = sorted({str(binary.parent) for binary in queue}, key=lambda path: (0 if path.endswith('/lib/private') else 1, path))
    loader_env = {**os.environ, 'LC_ALL': 'C', 'LD_LIBRARY_PATH': ':'.join(local_dirs)}
    while queue:
        binary = queue.pop()
        if binary in processed: continue
        processed.add(binary)
        versions = subprocess.check_output(['readelf', '--version-info', str(binary)], text=True)
        validate_glibc(versions)
        output = subprocess.run(['ldd', str(binary)], env=loader_env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        if 'not found' in output.stdout: raise RuntimeError(f'Missing libraries for {binary}:\n{output.stdout}')
        for line in output.stdout.splitlines():
            match = re.search(r'=> (/[^ ]+) \(', line)
            if not match: continue
            source = Path(match[1])
            if source.name in GLIBC_LIBS: continue
            destination = library_dir / source.name
            if destination.exists():
                if hashlib.sha256(destination.read_bytes()).digest() != hashlib.sha256(source.read_bytes()).digest():
                    raise RuntimeError(f'Conflicting native library: {source} and {destination}')
                continue
            copy(source, destination)
            queue.append(destination)
            for candidate in (source, source.resolve()):
                owner = subprocess.run(['dpkg-query', '-S', str(candidate)], text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
                if owner.returncode == 0:
                    packages.add(owner.stdout.split(': ', 1)[0].splitlines()[0])
                    break
    # Set RUNPATH only after resolving the unmodified dependency closure.
    for binary in processed:
        needed = subprocess.run(['patchelf', '--print-needed', str(binary)], text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        if needed.returncode != 0: continue
        relative = os.path.relpath(library_dir, binary.parent)
        run(['patchelf', '--set-rpath', '$ORIGIN:' + '$ORIGIN/' + relative, binary])
    return packages


def bundle_libraries(target):
    packages = set()
    php_components = [f'php-{minor}' for minor, version in MANIFEST.get('phpVersions', {}).items() if version != MANIFEST['php']]
    for component in ('php', *php_components, 'nginx', 'mysql'):
        packages.update(bundle_component_libraries(target / component))
    sources = []
    for package in sorted(packages):
        metadata = subprocess.check_output(['dpkg-query', '-W', '-f=${source:Package}\t${source:Version}\n', package], text=True).strip().split('\t')
        sources.append({'package': package, 'source': metadata[0], 'version': metadata[1]})
        copyright = Path('/usr/share/doc') / package.split(':')[0] / 'copyright'
        if copyright.is_file(): copy(copyright, target / 'licenses/linux-libraries' / (package.replace(':', '-') + '.txt'))
    (target / 'linux-library-sources.json').write_text(json.dumps(sources, indent=2) + '\n')
    return sources


def linux_arch():
    machine = platform.machine()
    if machine in ('x86_64', 'AMD64'): return 'x86_64'
    if machine in ('aarch64', 'arm64'): return 'aarch64'
    raise RuntimeError(f'Linux bundles currently support x86_64 and aarch64, not {machine}')


def reusable_php(prefix, version):
    """Do not reuse an older build that lacks newly required PHP extensions."""
    if not (prefix / 'sbin/php-fpm').is_file():
        return False
    try:
        output = subprocess.check_output(
            [str(prefix / 'bin/php'), '-n', '-r',
             'echo json_encode(["version"=>PHP_VERSION,"extensions"=>get_loaded_extensions()]);'],
            env={**os.environ, 'PHP_INI_SCAN_DIR': '', 'MIBS': ''}, text=True,
            stderr=subprocess.PIPE, timeout=15)
        info = json.loads(output)
        return info['version'] == version and required_php_extensions(MANIFEST, 'Linux').issubset(
            name.lower() for name in info['extensions'])
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        return False


def reusable_nginx(prefix, version):
    """A staging directory alone does not identify the compiled Nginx version."""
    try:
        output = subprocess.check_output([str(prefix / 'bin/nginx'), '-v'],
                                         stderr=subprocess.STDOUT, text=True, timeout=15)
        return output.strip() == f'nginx version: nginx/{version}'
    except (OSError, subprocess.SubprocessError):
        return False


def linux(target, lock, refresh):
    arch = linux_arch()
    version = os.confstr('CS_GNU_LIBC_VERSION')
    if version != 'glibc 2.35':
        raise RuntimeError(f'Build the Linux runtime on Ubuntu 22.04 / glibc 2.35, not {version}')
    jobs = str(min(len(os.sched_getaffinity(0)), 4))
    work = ROOT / '.runtime-work/linux-build'
    work.mkdir(parents=True, exist_ok=True)
    source_output = ROOT / '.runtime-work/linux-sources'
    source_output.mkdir(parents=True, exist_ok=True)
    if 'fnm' in MANIFEST:
        if arch != 'x86_64':
            raise RuntimeError('The pinned Linux fnm package currently supports x86_64 only')
        fnm_runtime(target, lock, refresh, 'linux')
        binary = target / 'fnm/bin/fnm'
        validate_glibc(subprocess.check_output(['readelf', '--version-info', str(binary)], text=True))
        # Keep upstream fnm unchanged. It must not need unbundled host libraries.
        linked = subprocess.run(['ldd', str(binary)], text=True, capture_output=True)
        if 'not found' in linked.stdout + linked.stderr:
            raise RuntimeError('The upstream fnm binary has missing native dependencies')
        from fnm_sources import source_plan
        source = lock_download('fnm-source', lock['fnm-source']['url'], refresh=refresh, lock=lock)
        plan = source_plan(source, lock['fnm-source'], MANIFEST['fnm'])
        def collect(item):
            archive, _ = download(item['url'], item['sha256'])
            copy(archive, source_output / 'fnm' / item['file'])
        with concurrent.futures.ThreadPoolExecutor(max_workers=6) as workers:
            list(workers.map(collect, plan))
        copy(ROOT / 'scripts/fnm_sources.py', source_output / 'fnm_sources.py')
        (source_output / 'fnm-source-inputs.json').write_text(json.dumps(plan, indent=2) + '\n')
    php_builds = [('php', MANIFEST['php'])]
    php_builds.extend((f'php-{minor}', version) for minor, version in MANIFEST.get('phpVersions', {}).items() if version != MANIFEST['php'])
    for name, version in [*php_builds, ('nginx', MANIFEST['nginx'])]:
        suffix = 'tar.xz' if name.startswith('php') else 'tar.gz'
        url = f'https://www.php.net/distributions/php-{version}.tar.xz' if name.startswith('php') else f'https://nginx.org/download/nginx-{version}.tar.gz'
        archive = lock_download(name + '-linux-source', url, refresh=refresh, lock=lock)
        copy(archive, source_output / f'{name}-{version}.{suffix}')
        source = work / name
        if source.exists(): shutil.rmtree(source)
        safe_extract(archive, source, 'tar')
        source, = [path for path in source.iterdir() if path.is_dir()]
        if name.startswith('php'):
            prefix = work / f'{name}-install'
            if not reusable_php(prefix, version):
                run(['./configure', f'--prefix={prefix}', '--disable-all', '--enable-cli', '--enable-phar', '--with-iconv', '--enable-fpm', '--disable-cgi', '--enable-bcmath', '--enable-ctype', '--with-curl', '--enable-dom', '--enable-fileinfo', '--enable-filter', '--with-libxml', '--enable-gd', '--with-jpeg', '--with-webp', '--with-freetype', '--enable-intl', '--with-ldap', '--enable-mbstring', '--enable-mysqlnd', '--with-mysqli=mysqlnd', '--with-openssl', '--enable-pcntl', '--enable-posix', '--enable-pdo', '--with-pdo-mysql=mysqlnd', '--with-pdo-pgsql', '--with-pdo-sqlite', '--with-pgsql', '--enable-session', '--enable-simplexml', '--enable-soap', '--enable-sockets', '--with-sodium', '--with-sqlite3', '--enable-tokenizer', '--enable-xml', '--enable-xmlreader', '--enable-xmlwriter', '--with-zip', '--with-zlib', '--without-pear'], cwd=source)
                run(['make', '-j' + jobs], cwd=source)
                run(['make', 'install'], cwd=source)
            else:
                print(f'Reusing {prefix / "bin/php"}', flush=True)
            copy(prefix / 'bin/php', target / name / 'bin/php')
            copy(prefix / 'sbin/php-fpm', target / name / 'bin/php-fpm')
            (target / name / 'ext').mkdir(parents=True, exist_ok=True)
            for notice in ('LICENSE', 'NEWS'): copy(source / notice, target / 'licenses' / name / notice)
        else:
            prefix = target / 'nginx'
            if not reusable_nginx(prefix, version):
                run(['./configure', '--prefix=/', '--sbin-path=bin/nginx', '--conf-path=conf/nginx.conf', '--pid-path=nginx.pid', '--error-log-path=logs/nginx.log', '--http-log-path=logs/access.log', '--with-http_ssl_module', '--with-http_gzip_static_module', '--with-http_realip_module', '--with-http_stub_status_module'], cwd=source)
                run(['make', '-j' + jobs], cwd=source)
                run(['make', 'install', f'DESTDIR={prefix}'], cwd=source)
            else:
                print(f'Reusing {prefix / "bin/nginx"}', flush=True)
            copy(source / 'LICENSE', target / 'licenses/nginx/LICENSE')
    series = '.'.join(MANIFEST['mysql'].split('.')[:2])
    # Oracle ships an x86_64 minimal archive. The aarch64 release only has the full generic build.
    if arch == 'x86_64':
        url = f'https://cdn.mysql.com/Downloads/MySQL-{series}/mysql-{MANIFEST["mysql"]}-linux-glibc2.28-x86_64-minimal.tar.xz'
        mysql_key = 'mysql-linux-x86_64'
    else:
        url = f'https://cdn.mysql.com/Downloads/MySQL-{series}/mysql-{MANIFEST["mysql"]}-linux-glibc2.28-aarch64.tar.xz'
        mysql_key = 'mysql-linux-aarch64'
    archive = lock_download(mysql_key, url, refresh=refresh, lock=lock)
    with tempfile.TemporaryDirectory() as folder:
        safe_extract(archive, Path(folder), 'tar')
        source, = [path for path in Path(folder).iterdir() if path.is_dir()]
        copy(source, target / 'mysql')
    for notice in ('LICENSE', 'README', 'LICENSE.router'):
        if (target / 'mysql' / notice).is_file(): copy(target / 'mysql' / notice, target / 'licenses/mysql' / notice)
    sources = bundle_libraries(target)
    # Include exact Ubuntu source packages and distribution patches for bundled libraries.
    # deb-src must be enabled on the build machine. These are download-only operations.
    for name, version in sorted({(s['source'], s['version']) for s in sources}):
        run(['apt-get', 'source', '--download-only', f'{name}={version}'], cwd=source_output)
    source_url = f'https://cdn.mysql.com/Downloads/MySQL-{series}/mysql-{MANIFEST["mysql"]}.tar.gz'
    archive = lock_download('mysql-linux-source', source_url, refresh=refresh, lock=lock)
    copy(archive, source_output / f'mysql-{MANIFEST["mysql"]}.tar.gz')
    url = f'https://codeload.github.com/axllent/mailpit/tar.gz/refs/tags/v{MANIFEST["mailpit"]}'
    archive = lock_download('mailpit-linux-source', url, refresh=refresh, lock=lock)
    copy(archive, source_output / f'mailpit-{MANIFEST["mailpit"]}.tar.gz')
    for name in ('composer', 'ca-certificates'):
        entry = lock[name + '-linux-source']
        archive = lock_download(name + '-linux-source', entry['url'], refresh=refresh, lock=lock)
        copy(archive, source_output / (name + '.tar.gz'))
    entry = lock['phpmyadmin']
    archive = lock_download('phpmyadmin', entry['url'], refresh=refresh, lock=lock)
    copy(archive, source_output / 'phpmyadmin-source.zip')
    (source_output / 'library-sources.json').write_text(json.dumps(sources, indent=2) + '\n')
