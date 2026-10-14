#!/usr/bin/env python3
"""Build Linux x86_64 on Ubuntu 22.04 and relocate its native library closure."""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
from prepare_runtime import ROOT, MANIFEST, copy, lock_download, safe_extract

GLIBC_LIBS = {'libc.so.6', 'libm.so.6', 'libpthread.so.0', 'libdl.so.2', 'librt.so.1', 'libresolv.so.2', 'libutil.so.1', 'ld-linux-x86-64.so.2'}


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


def linux(target, lock, refresh):
    if platform.machine() not in ('x86_64', 'AMD64'):
        raise RuntimeError('Linux bundles currently support x86_64 only')
    version = os.confstr('CS_GNU_LIBC_VERSION')
    if version != 'glibc 2.35':
        raise RuntimeError(f'Build the Linux runtime on Ubuntu 22.04 / glibc 2.35, not {version}')
    jobs = str(min(os.cpu_count() or 2, 4))
    work = ROOT / '.runtime-work/linux-build'
    work.mkdir(parents=True, exist_ok=True)
    source_output = ROOT / '.runtime-work/linux-sources'
    source_output.mkdir(parents=True, exist_ok=True)
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
            if version.startswith('8.0.'):
                # PHP 8.0 predates OpenSSL 3 in Ubuntu 22.04. Apply the
                # checksummed backports used by Homebrew's final 8.0 formula.
                patches = (
                    ('openssl3', '0002-Add-minimal-OpenSSL-3.0-patch-PHP8.0.patch'),
                    ('openssl-eof', '0003-Fix-bug-79589-ssl3_read_n-unexpected-eof-while-reading-PHP8.0.patch'),
                )
                for key, filename in patches:
                    url = f'https://raw.githubusercontent.com/shivammathur/php-src-backports/2bcb0b/patches/{filename}'
                    patch = lock_download(f'php-8.0-linux-{key}-patch', url, refresh=refresh, lock=lock)
                    copy(patch, source_output / filename)
                    run(['patch', '-p1', '-i', str(patch)], cwd=source)
            prefix = work / f'{name}-install'
            run(['./configure', f'--prefix={prefix}', '--disable-all', '--enable-cli', '--enable-phar', '--with-iconv', '--enable-fpm', '--disable-cgi', '--enable-bcmath', '--enable-ctype', '--with-curl', '--enable-dom', '--enable-fileinfo', '--enable-filter', '--with-libxml', '--enable-gd', '--with-jpeg', '--with-webp', '--with-freetype', '--enable-intl', '--enable-mbstring', '--enable-mysqlnd', '--with-mysqli=mysqlnd', '--with-openssl', '--enable-pdo', '--with-pdo-mysql=mysqlnd', '--with-pdo-sqlite', '--enable-session', '--enable-simplexml', '--with-sodium', '--with-sqlite3', '--enable-tokenizer', '--enable-xml', '--enable-xmlreader', '--enable-xmlwriter', '--with-zip', '--with-zlib', '--without-pear'], cwd=source)
            run(['make', '-j' + jobs], cwd=source)
            run(['make', 'install'], cwd=source)
            copy(prefix / 'bin/php', target / name / 'bin/php')
            copy(prefix / 'sbin/php-fpm', target / name / 'bin/php-fpm')
            (target / name / 'ext').mkdir(parents=True, exist_ok=True)
            for notice in ('LICENSE', 'NEWS'): copy(source / notice, target / 'licenses' / name / notice)
        else:
            prefix = target / 'nginx'
            run(['./configure', '--prefix=/', '--sbin-path=bin/nginx', '--conf-path=conf/nginx.conf', '--pid-path=nginx.pid', '--error-log-path=logs/nginx.log', '--http-log-path=logs/access.log', '--with-http_ssl_module', '--with-http_gzip_static_module', '--with-http_realip_module', '--with-http_stub_status_module'], cwd=source)
            run(['make', '-j' + jobs], cwd=source)
            run(['make', 'install', f'DESTDIR={prefix}'], cwd=source)
            copy(source / 'LICENSE', target / 'licenses/nginx/LICENSE')
    series = '.'.join(MANIFEST['mysql'].split('.')[:2])
    url = f'https://cdn.mysql.com/Downloads/MySQL-{series}/mysql-{MANIFEST["mysql"]}-linux-glibc2.28-x86_64-minimal.tar.xz'
    archive = lock_download('mysql-linux-x86_64', url, refresh=refresh, lock=lock)
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
    for name in ('composer', 'ca-certificates'):
        entry = lock[name + '-linux-source']
        archive = lock_download(name + '-linux-source', entry['url'], refresh=refresh, lock=lock)
        copy(archive, source_output / (name + '.tar.gz'))
    entry = lock['phpmyadmin']
    archive = lock_download('phpmyadmin', entry['url'], refresh=refresh, lock=lock)
    copy(archive, source_output / 'phpmyadmin-source.zip')
    (source_output / 'library-sources.json').write_text(json.dumps(sources, indent=2) + '\n')
