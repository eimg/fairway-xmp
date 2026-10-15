#!/usr/bin/env python3
"""Collect version-matched runtime sources and packaging recipes for a release."""
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import http.client
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tarfile
import urllib.parse
import zipfile

from prepare_runtime import CACHE, LOCK_PATH, MANIFEST, ROOT, download, read_json, request


def archive_git_tree(archive):
    """Hash a Gitiles snapshot's files and modes without extracting or executing it."""
    def object_hash(kind, body):
        return hashlib.sha1(kind + b' ' + str(len(body)).encode() + b'\0' + body).digest()

    root = {}
    directories = []
    with tarfile.open(archive) as bundle:
        for member in bundle:
            path = PurePosixPath(member.name)
            if path.is_absolute() or '..' in path.parts or '\\' in member.name or ':' in member.name:
                raise RuntimeError('Unsafe source archive path')
            permissions = member.mode & 0o7777
            if member.isdir():
                if permissions != 0o755: raise RuntimeError('Unsafe source archive directory mode')
                directories.append(path)
                continue
            parent = root
            for part in path.parts[:-1]: parent = parent.setdefault(part, {})
            if member.isfile():
                if permissions not in (0o644, 0o755): raise RuntimeError('Unsafe source archive file mode')
                body = bundle.extractfile(member).read()
                mode = b'100755' if member.mode & 0o111 else b'100644'
            elif member.issym():
                if permissions != 0o777: raise RuntimeError('Unsafe source archive symlink mode')
                body, mode = member.linkname.encode(), b'120000'
            else:
                raise RuntimeError('Unexpected source archive member')
            if path.name in parent: raise RuntimeError('Duplicate source archive path')
            parent[path.name] = (mode, object_hash(b'blob', body))
    for path in directories:
        folder = root
        for part in path.parts:
            if not isinstance(folder, dict) or part not in folder:
                raise RuntimeError('Source archive directory is outside its Git tree')
            folder = folder[part]
        if not isinstance(folder, dict): raise RuntimeError('Source archive directory conflicts with a file')

    def tree(folder):
        body = b''
        for name, value in sorted(folder.items(), key=lambda item: item[0].encode() + (b'/' if isinstance(item[1], dict) else b'')):
            mode, digest = (b'40000', tree(value)) if isinstance(value, dict) else value
            body += mode + b' ' + name.encode() + b'\0' + digest
        return object_hash(b'tree', body)
    return tree(root).hex()


def verify_gitiles_source(archive, item):
    tree = archive_git_tree(archive)
    if pinned_gitiles_source(item):
        if tree != item['gitTree']:
            raise RuntimeError(f'Source contents do not match the committed Git tree for {item["name"]}')
        return
    revision = item['revision']
    url = item['url'].split('/+archive/', 1)[0] + '/+/' + revision + '?format=JSON'
    with request(url) as response:
        data = json.loads(response.read().decode().split('\n', 1)[1])
    if data['commit'] != revision or tree != data['tree']:
        raise RuntimeError(f'Source contents do not match the pinned Git commit for {item["name"]}')
    if item.get('gitTree') and item['gitTree'] != data['tree']:
        raise RuntimeError(f'The pinned source tree changed for {item["name"]}')
    item['gitTree'] = data['tree']


def pinned_gitiles_source(item):
    url = urllib.parse.urlsplit(item['url'])
    revision = item.get('revision') or ''
    return bool(url.scheme == 'https' and (url.hostname or '').endswith('.googlesource.com')
                and re.fullmatch('[0-9a-f]{40}', revision)
                and url.path.endswith(f'/+archive/{revision}.tar.gz')
                and re.fullmatch('[0-9a-f]{40}', item.get('gitTree') or ''))


def source_input(name, url, sha256=None):
    path = urllib.parse.urlsplit(url).path
    suffix = next((suffix for suffix in ('.tar.gz', '.tar.xz', '.tar.bz2', '.tgz', '.zip') if path.endswith(suffix)), '.tar.gz')
    return {'name': name, 'url': url, 'sha256': sha256, 'file': name.replace('@', '-') + suffix}


def collect_plan(stage, refresh=False):
    lock = json.loads(LOCK_PATH.read_text())
    source_lock = ROOT / 'runtime-sources.lock.json'
    pinned = {item['name']: item for item in json.loads(source_lock.read_text())} if source_lock.exists() else {}
    pinned_by_url = {item['url']: item for item in json.loads(source_lock.read_text())} if source_lock.exists() else {}
    plan = []
    recipes = stage / 'homebrew'
    recipes.mkdir(parents=True, exist_ok=True)
    for key, entry in sorted(lock.items()):
        if not key.startswith('brew-'):
            continue
        name = key.removeprefix('brew-').removesuffix('-arm64-sequoia')
        archive = CACHE / hashlib.sha256(entry['url'].encode()).hexdigest()
        if not archive.exists() or hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest() != entry['sha256']:
            raise RuntimeError(f'Missing verified bottle for {name}; stage the locked macOS runtime first')
        with tarfile.open(archive) as bottle:
            formula, = [member for member in bottle.getmembers() if '/.brew/' in member.name and member.name.endswith('.rb')]
            text = bottle.extractfile(formula).read().decode()
            (recipes / f'{name}.rb').write_text(text)
            metadata = [member for member in bottle.getmembers() if member.name.endswith(('/INSTALL_RECEIPT.json', '/sbom.spdx.json'))]
            for member in metadata:
                (recipes / f'{name}.{Path(member.name).name}').write_bytes(bottle.extractfile(member).read())
        url = re.search(r'^  url "([^"]+)"', text, re.M)
        sha = re.search(r'^  sha256 "([0-9a-f]{64})"', text, re.M)
        revision = re.search(r'^\s+revision: "([0-9a-f]{40})"', text, re.M)
        if url and url.group(1).endswith('.git') and revision:
            item = source_input(name, url.group(1).removesuffix('.git') + '/+archive/' + revision.group(1) + '.tar.gz')
            item['revision'] = revision.group(1)
        elif url and sha:
            item = source_input(name, url.group(1), sha.group(1))
        else:
            raise RuntimeError(f'Review the source recipe for {name}: cannot identify stable source')
        item['binary'] = {'key': key, **entry}
        item['mirrors'] = re.findall(r'^  mirror "([^"]+)"', text.split('  license', 1)[0], re.M)
        plan.append(item)
        tap = None
        for index, block in enumerate(re.findall(r'^  (?:patch[^\n]*|resource "[^"]+") do\n(.*?)^  end', text, re.M | re.S)):
            patch_url = re.search(r'^\s+url "([^"]+)"', block, re.M)
            patch_sha = re.search(r'^\s+sha256 "([0-9a-f]{64})"', block, re.M)
            local_patch = re.search(r'^\s+file "([^"]+)"', block, re.M)
            if local_patch:
                previous = pinned.get(f'{name}-resource-{index}')
                previous_main = pinned.get(name, {})
                if previous and previous_main.get('binary', {}).get('sha256') == entry['sha256']:
                    # An unchanged bottle keeps its historical patch URL even
                    # when Homebrew's current tap head moves forward.
                    plan.append(dict(previous))
                    continue
                if tap is None:
                    tap = read_json(f'https://formulae.brew.sh/api/formula/{urllib.parse.quote(name, safe="")}.json')
                    if tap['bottle']['stable']['files']['arm64_sequoia']['sha256'] != entry['sha256']:
                        raise RuntimeError(f'Homebrew recipe changed for {name}; locate its historical patch commit')
                extra = source_input(f'{name}-resource-{index}', f'https://raw.githubusercontent.com/Homebrew/homebrew-core/{tap["tap_git_head"]}/{local_patch.group(1)}')
                extra['file'] = f'{name.replace("@", "-")}-patch-{index}.patch'
                plan.append(extra)
                continue
            if not patch_url or not patch_sha:
                raise RuntimeError(f'Review external source/patch {index} for {name}')
            extra = source_input(f'{name}-resource-{index}', patch_url.group(1), patch_sha.group(1))
            if 'patch' in extra['url']:
                extra['file'] = f'{name.replace("@", "-")}-patch-{index}.patch'
            plan.append(extra)
    if 'nginx-macos-source' in lock:
        nginx = lock['nginx-macos-source']
        plan.append(source_input('nginx', nginx['url'], nginx['sha256']))
        shutil.copyfile(ROOT / 'src-tauri/resources/runtime/nginx/build.json', stage / 'nginx-build.json')
    # The release distribution includes readable PHP/JavaScript and vendor files;
    # the Git tag also supplies the preferred development sources/build inputs.
    pma = lock['phpmyadmin']
    plan.append(source_input('phpmyadmin-distribution', pma['url'], pma['sha256']))
    plan.append(source_input('phpmyadmin-development', f'https://codeload.github.com/phpmyadmin/phpmyadmin/tar.gz/refs/tags/RELEASE_{MANIFEST["phpmyadmin"].replace(".", "_")}'))
    plan.append(source_input('composer', f'https://codeload.github.com/composer/composer/tar.gz/refs/tags/{MANIFEST["composer"]}'))
    certifi_version = re.search(r'certifi-([\d.]+)-', lock['ca-certificates']['url']).group(1)
    certifi = read_json(f'https://pypi.org/pypi/certifi/{certifi_version}/json')
    sdist, = [file for file in certifi['urls'] if file['packagetype'] == 'sdist']
    plan.append(source_input('ca-certificates', sdist['url'], sdist['digests']['sha256']))
    # Windows PHP's SBOM identifies the exact dependency repositories, including
    # LGPL libraries and their Windows build recipes/patches. Older releases
    # predate the SBOM, so their versioned source refs are pinned separately.
    legacy = json.loads((ROOT / 'scripts/windows_legacy_sources.json').read_text())
    shutil.copyfile(ROOT / 'scripts/windows_legacy_sources.json', stage / 'windows_legacy_sources.json')
    seen = set()
    for key in ('php-windows', *(f'php-{minor}-windows' for minor, version in MANIFEST.get('phpVersions', {}).items() if version != MANIFEST['php'])):
        php_windows = lock[key]
        windows_archive, _ = download(php_windows['url'], php_windows['sha256'])
        windows_metadata = stage / key
        windows_metadata.mkdir(exist_ok=True)
        with zipfile.ZipFile(windows_archive) as php:
            for filename in ['extras/sbom/php.spdx.json', 'extras/sbom/php.cdx.json', 'readme-redist-bins.txt', 'license.txt']:
                if filename in php.namelist():
                    (windows_metadata / Path(filename).name).write_bytes(php.read(filename))
            has_sbom = 'extras/sbom/php.spdx.json' in php.namelist()
            sbom = json.loads(php.read('extras/sbom/php.spdx.json')) if has_sbom else {'packages': []}
        if not has_sbom:
            minor = key.removeprefix('php-').removesuffix('-windows')
            sources = [entry for entry in legacy if minor in entry['php']]
            if minor not in ('8.0', '8.1') or not sources:
                raise RuntimeError(f'No Windows dependency source manifest for {key}')
            for entry in sources:
                ref = f'git+https://github.com/{entry["repo"]}.git@{entry["revision"]}'
                if ref in seen:
                    continue
                seen.add(ref)
                extra = source_input('windows-' + entry['repo'].replace('/', '-') + '-' + entry['revision'][:12],
                                     f'https://codeload.github.com/{entry["repo"]}/tar.gz/{entry["revision"]}')
                extra.update(revision=entry['revision'], binaryPackage=entry['package'],
                             sourceTag=entry['tag'], phpVersions=entry['php'])
                plan.append(extra)
        for package in sbom['packages']:
            for ref in package.get('externalRefs') or []:
                match = re.fullmatch(r'git\+https://github.com/([^/]+/[^/]+)\.git@([0-9a-f]{40})', ref.get('referenceLocator', ''))
                if not match or match.group(0) in seen:
                    continue
                seen.add(match.group(0))
                repo, revision = match.groups()
                extra = source_input('windows-' + repo.replace('/', '-') + '-' + revision[:12], f'https://codeload.github.com/{repo}/tar.gz/{revision}')
                extra.update(revision=revision, binaryPackage=package['name'], binaryVersion=package.get('versionInfo'))
                plan.append(extra)
    if pinned:
        for item in plan:
            previous = pinned.get(item['name']) or pinned_by_url.get(item['url'])
            if previous:
                if refresh and item.get('revision') and '.googlesource.com/' in item['url']:
                    # Gitiles may recompress an unchanged snapshot. A refreshed
                    # archive must reproduce the pinned commit's full Git tree.
                    if item.get('revision') == previous.get('revision') and previous.get('gitTree'):
                        item['gitTree'] = previous['gitTree']
                    continue
                if item['url'] != previous['url']:
                    if refresh: continue
                    raise RuntimeError(f'Source URL changed for {item["name"]}; review the source lock')
                if item.get('sha256') and item['sha256'] != previous['sha256']:
                    raise RuntimeError(f'Source checksum changed for {item["name"]}; refused to replace its pin')
                item['sha256'] = previous['sha256']
                if previous.get('gitTree') and item.get('revision') == previous.get('revision'):
                    item['gitTree'] = previous['gitTree']
    if len({item['file'] for item in plan}) != len(plan):
        raise RuntimeError('Source archive filenames must be unique')
    return plan


def require_locked_plan(plan, locked):
    expected = {item['name']: item for item in locked}
    actual = {item['name']: item for item in plan}
    if len(expected) != len(locked) or len(actual) != len(plan) or set(actual) != set(expected):
        raise RuntimeError('Source inputs changed; review and commit the source lock before building a candidate')
    for name, item in actual.items():
        pin = expected[name]
        if (not item.get('sha256')
                or any(item.get(key) != pin.get(key) for key in ('url', 'revision', 'gitTree'))
                or (item['sha256'] != pin['sha256'] and not pinned_gitiles_source(pin))):
            raise RuntimeError(f'Unreviewed source input for {name}; candidate builds require the committed source lock')


def fetch_source(item, sources):
    failure = None
    archive = None
    # Gitiles can recompress a snapshot without changing its files or modes.
    # Its committed Git tree is authoritative; record the transport SHA below.
    expected_sha = None if pinned_gitiles_source(item) else item['sha256']
    for url in [item['url'], *item.get('mirrors', [])]:
        for attempt in range(3):
            try:
                archive, sha = download(url, expected_sha)
                break
            except RuntimeError as error:
                if 'Checksum mismatch' not in str(error) or attempt == 2:
                    raise
            except (OSError, http.client.HTTPException) as error:
                failure = error
        if archive is not None:
            break
    if archive is None:
        raise RuntimeError(f'Cannot obtain source for {item["name"]}') from failure
    if item.get('revision') and '.googlesource.com/' in item['url']:
        verify_gitiles_source(archive, item)
    destination = sources / item['file']
    shutil.copyfile(archive, destination)
    print(f'Verified {item["name"]}: {destination.stat().st_size / 1024**2:.1f} MiB', flush=True)
    return {**item, 'sha256': sha}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan-only', action='store_true', help='Write recipes and download plan without fetching source archives')
    parser.add_argument('--source-ref', default='HEAD', help='Application source revision matching the installers')
    parser.add_argument('--refresh-lock', action='store_true', help='Allow reviewed version changes while retaining unchanged source pins')
    parser.add_argument('--locked', action='store_true', help='Require committed source pins and never rewrite the source lock')
    args = parser.parse_args()
    if args.locked and args.refresh_lock: parser.error('--locked cannot be combined with --refresh-lock')
    version = json.loads((ROOT / 'package.json').read_text())['version']
    stage = ROOT / '.runtime-work' / f'sources-{version}'
    stage.mkdir(parents=True, exist_ok=True)
    plan = collect_plan(stage, refresh=args.refresh_lock)
    locked = json.loads((ROOT / 'runtime-sources.lock.json').read_text()) if args.locked else None
    if args.locked: require_locked_plan(plan, locked)
    plan_path = stage / 'source-inputs.json'
    plan_path.write_text(json.dumps(plan, indent=2) + '\n')
    if args.plan_only:
        print(f'{len(plan)} source inputs; plan at {plan_path}')
        return
    sources = stage / 'upstream'
    sources.mkdir(exist_ok=True)

    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        verified = list(pool.map(lambda item: fetch_source(item, sources), plan))
    expected_files = {item['file'] for item in verified}
    for obsolete in sources.iterdir():
        if obsolete.is_file() and obsolete.name not in expected_files:
            obsolete.unlink()
    plan_path.write_text(json.dumps(verified, indent=2) + '\n')
    if args.locked:
        require_locked_plan(verified, locked)
    else:
        (ROOT / 'runtime-sources.lock.json').write_text(json.dumps(verified, indent=2) + '\n')
    commit = subprocess.check_output(['git', 'rev-parse', args.source_ref], cwd=ROOT, text=True).strip()
    subprocess.run(['git', 'archive', '--format=tar.gz', f'--output={stage / "fairway-xmp-source.tar.gz"}', commit], cwd=ROOT, check=True)
    shutil.copyfile(LOCK_PATH, stage / LOCK_PATH.name)
    shutil.copyfile(ROOT / 'runtime-manifest.json', stage / 'runtime-manifest.json')
    shutil.copyfile(Path(__file__), stage / 'package_sources.py')
    (stage / 'README.md').write_text(f'''# Fairway XMP {version}: bundled-tool source

This archive accompanies the Windows x64 and macOS ARM64 installers.
Fairway XMP source commit: {commit}.

`upstream/` contains source distributions for every bundled Homebrew component,
MySQL (also used by the official Windows distribution), Nginx, phpMyAdmin, Composer,
and the certificate bundle. `source-inputs.json` maps each archive to its URL,
SHA-256, and matching macOS bottle when applicable.
Gitiles snapshots also record their pinned revision and full Git tree. Their
archive compression may change; every file, symlink and executable mode must
still reproduce the committed tree. SHA-256 records the actual shipped archive.

`homebrew/` contains the exact recipes and available build metadata from the pinned
bottles. Recipes include inline patches; externally downloaded patches and
resource sources are in `upstream/`. Use the recorded recipes to build native
components with Homebrew and their listed build prerequisites. These source
archives retain the components' own licenses.

`nginx-build.json` records the stable Nginx source and configure options for the
Mac build. The header/library paths in that record point to temporary extracted
bottle folders on the builder; recreate them using the pinned PCRE2/OpenSSL
inputs. The Mac build uses SDK zlib, stages the executable and MIME types, and
relocates library load paths with `scripts/prepare_runtime.py`.

`windows-php/` retains the official PHP distribution's notices and, where
provided, software bill of materials. PHP 8.0 and 8.1 predate the SBOM;
`windows_legacy_sources.json` maps their dependency packages to pinned,
version-matched source trees under `upstream/windows-*`. The newer SBOMs'
immutable repository references supply their dependency source trees there too.

`fairway-xmp-source.tar.gz` contains the application, configuration templates,
and scripts that stage, relocate, and package the runtimes. Extract it and see
CONTRIBUTING.md for platform build instructions. `scripts/prepare_runtime.py`
describes staging and macOS library load-path relocation, followed by ad-hoc
signing, and the stable Nginx source build. Windows uses the
unmodified upstream executables and DLLs. No MySQL or PHP source patches are
applied by Fairway; upstream Homebrew patches are recorded in the recipes.

macOS libraries are dynamically linked under the app's `Contents/Resources/runtime/lib`.
You may replace a library with a compatible build. Apply the same relative load
paths and ad-hoc signing described by the relocation script before running it.
''')
    assets = ROOT / 'artifacts' / f'alpha-{version}'
    assets.mkdir(parents=True, exist_ok=True)
    output = assets / f'Fairway-XMP-{version}-bundled-tool-sources.tar.gz'
    with tarfile.open(output, 'w:gz', compresslevel=1) as bundle:
        bundle.add(stage, arcname=f'Fairway-XMP-{version}-sources')
    print(f'Source archive ready: {output} ({output.stat().st_size / 1024**2:.1f} MiB)', flush=True)


if __name__ == '__main__':
    main()
