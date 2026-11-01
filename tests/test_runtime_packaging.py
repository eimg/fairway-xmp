import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import tarfile
import unittest
import zipfile
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('runtime', Path(__file__).resolve().parents[1] / 'scripts/prepare_runtime.py')
runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runtime)


class ArchiveTests(unittest.TestCase):
    def test_windows_php_toolsets_cover_extra_and_default_8_6_without_changing_old_urls(self):
        manifest = {**runtime.MANIFEST, 'phpVersions': {**runtime.MANIFEST['phpVersions'], '8.6': '8.6.0'}}
        sources = runtime.windows_php_sources(manifest)
        for minor, toolset in [('8.2', 'vs16'), ('8.3', 'vs16'), ('8.4', 'vs17'), ('8.5', 'vs17'), ('8.6', 'vs18')]:
            key = 'php-windows' if manifest['phpVersions'][minor] == manifest['php'] else f'php-{minor}-windows'
            self.assertIn(f'-nts-Win32-{toolset}-x64.zip', sources[key][0])
        self.assertEqual(sources['php-8.6-windows'][1], 'php-8.6')
        sources = runtime.windows_php_sources({**manifest, 'php': '8.6.0'})
        self.assertIn('php-8.6.0-nts-Win32-vs18-x64.zip', sources['php-windows'][0])
        self.assertNotIn('php-8.6-windows', sources)
        self.assertIn('php-8.5-windows', sources)
        with self.assertRaisesRegex(RuntimeError, 'No reviewed Windows toolset'):
            runtime.windows_php_url('8.7.0')

    def test_macos_default_formula_preserves_locked_bottles_and_handles_major_rollover(self):
        lock = {'brew-php-arm64-sequoia': {'version': runtime.MANIFEST['php']}}
        with patch.object(runtime, 'RESUME_LOCK', False):
            self.assertEqual(runtime.macos_default_php_formula(lock), 'php')
            self.assertEqual(runtime.macos_default_php_formula(lock, refresh=True), 'php@8.5')
        with patch.object(runtime, 'RESUME_LOCK', True):
            self.assertEqual(runtime.macos_default_php_formula(lock, refresh=True), 'php')
        self.assertEqual(runtime.macos_default_php_formula({}), 'php@8.5')
        self.assertEqual(runtime.macos_default_php_formula({'brew-php-arm64-sequoia': {'version': '8.6.0'}}), 'php@8.5')

    def test_windows_prerequisite_installs_the_verified_pin_and_preserves_historical_installer(self):
        installer = Path('/verified/cache/installer')
        lock = {'vc-runtime': {'url': 'https://aka.ms/vs/17/release/vc_redist.x64.exe', 'sha256': 'a' * 64}}
        with patch.object(runtime.platform, 'system', return_value='Windows'), \
                patch.object(runtime, 'lock_download', return_value=installer) as download, \
                patch.object(runtime, 'copy') as copy, \
                patch.object(runtime.subprocess, 'run') as run:
            runtime.install_windows_prerequisites(lock)
            download.assert_called_once_with('vc-runtime', lock['vc-runtime']['url'], refresh=False, lock=lock)
            staged = copy.call_args.args[1]
            self.assertEqual(copy.call_args.args[0], installer)
            self.assertEqual(staged.name, 'vc_redist.x64.exe')
            self.assertEqual(run.call_args.args[0][-2:], ['-Installer', str(staged)])
            self.assertTrue(run.call_args.kwargs['check'])
        with patch.object(runtime, 'lock_download', return_value=installer) as download:
            runtime.windows_vc_runtime({}, True)
            self.assertEqual(download.call_args.args[1], 'https://aka.ms/vc14/vc_redist.x64.exe')
        with patch.object(runtime.platform, 'system', return_value='Darwin'), \
                patch.object(runtime, 'lock_download') as download:
            with self.assertRaisesRegex(RuntimeError, 'only be installed on Windows'):
                runtime.install_windows_prerequisites(lock)
            download.assert_not_called()

    def test_fnm_stages_unchanged_executable_and_matching_license_on_each_platform(self):
        for system in ('windows', 'macos', 'linux'):
            with self.subTest(system=system), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                name = 'fnm.exe' if system == 'windows' else 'fnm'
                binary = root / f'fnm-{system}.zip'
                with zipfile.ZipFile(binary, 'w') as archive:
                    archive.writestr(name, b'upstream executable unchanged')
                source = root / 'fnm-source.tar.gz'
                with tarfile.open(source, 'w:gz') as archive:
                    content = b'GPL version 3'
                    entry = tarfile.TarInfo(f'fnm-{runtime.MANIFEST["fnm"]}/LICENSE')
                    entry.size = len(content)
                    archive.addfile(entry, io.BytesIO(content))
                def download(key, *args, **kwargs):
                    return source if key == 'fnm-source' else binary
                target = root / 'runtime'
                with patch.object(runtime, 'lock_download', side_effect=download):
                    runtime.fnm_runtime(target, {}, False, system)
                self.assertEqual((target / 'fnm/bin' / name).read_bytes(), b'upstream executable unchanged')
                self.assertEqual((target / 'licenses/fnm/LICENSE').read_bytes(), b'GPL version 3')
                self.assertFalse((target / 'node').exists())

    def test_verification_rejects_missing_client_or_worker_extension_on_each_php_version(self):
        # Exercise the next branch without advertising unreleased downloads.
        manifest = {**runtime.MANIFEST, 'phpVersions': {**runtime.MANIFEST['phpVersions'], '8.6': '8.6.0'}}
        with patch.object(runtime, 'MANIFEST', manifest):
            self.check_missing_extensions()

    def check_missing_extensions(self):
        folders = ['php', *(f'php-{minor}' for minor, version in runtime.MANIFEST['phpVersions'].items() if version != runtime.MANIFEST['php'])]
        for folder in folders:
            for extension in ('pdo_pgsql', 'pgsql', 'sockets', 'soap', 'ldap', 'pcntl', 'posix'):
                def output(command, **kwargs):
                    php_folder = Path(command[0]).parts[-3]
                    version = runtime.MANIFEST['php'] if php_folder == 'php' else runtime.MANIFEST['phpVersions'][php_folder[4:]]
                    missing = extension if php_folder == folder else None
                    return json.dumps({'version': version, 'extensions': [name for name in runtime.required_php_extensions(runtime.MANIFEST, 'Darwin') if name != missing]})
                with self.subTest(folder=folder, extension=extension), tempfile.TemporaryDirectory() as temporary, \
                        patch.object(runtime.platform, 'system', return_value='Darwin'), \
                        patch.object(runtime.subprocess, 'check_output', side_effect=output):
                    target = Path(temporary)
                    (target / 'manifest.json').write_text(json.dumps(runtime.MANIFEST))
                    with self.assertRaisesRegex(RuntimeError, extension):
                        runtime.verify(target)

    def test_worker_extensions_are_required_only_on_unix(self):
        for system in ('Darwin', 'Linux', 'Windows'):
            with self.subTest(system=system):
                required = runtime.required_php_extensions(runtime.MANIFEST, system)
                self.assertTrue({'sockets', 'soap', 'ldap'}.issubset(required))
                self.assertEqual({'pcntl', 'posix'} & required,
                                 set() if system == 'Windows' else {'pcntl', 'posix'})

    def test_windows_verifier_loads_shipped_clients_without_unix_extensions(self):
        def output(command, **kwargs):
            if Path(command[0]).name == 'fnm.exe': return f'fnm {runtime.MANIFEST["fnm"]}'
            php_folder = Path(command[0]).parts[-3]
            if '-r' in command:
                ini = Path(command[command.index('-c') + 1]).read_text()
                for extension in ('sockets', 'soap', 'ldap'):
                    self.assertIn(f'extension={extension}', ini)
                self.assertNotIn('extension=pcntl', ini)
                self.assertNotIn('extension=posix', ini)
                version = runtime.MANIFEST['php'] if php_folder == 'php' else runtime.MANIFEST['phpVersions'][php_folder[4:]]
                return json.dumps({'version': version, 'extensions': runtime.MANIFEST['extensions']})
            if any('composer.phar' in str(part) for part in command):
                return f'Composer version {runtime.MANIFEST["composer"]} bundled'
            if '-v' in command: return f'nginx/{runtime.MANIFEST["nginx"]}'
            if 'version' in command: return f'Mailpit v{runtime.MANIFEST["mailpit"]} bundled'
            return f'mysqld Ver {runtime.MANIFEST["mysql"]} bundled'

        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(runtime.platform, 'system', return_value='Windows'), \
                patch.object(runtime.subprocess, 'check_output', side_effect=output), \
                patch('sys.stdout', new=io.StringIO()):
            target = Path(temporary)
            (target / 'manifest.json').write_text(json.dumps(runtime.MANIFEST))
            for folder in ['php', *(f'php-{minor}' for minor, version in runtime.MANIFEST['phpVersions'].items() if version != runtime.MANIFEST['php'])]:
                ext = target / folder / 'ext'
                ext.mkdir(parents=True)
                for name in ('sockets', 'soap', 'ldap'):
                    (ext / f'php_{name}.dll').touch()
            runtime.verify(target)

    def test_runtime_verification_is_offline_and_still_rejects_wrong_mailpit_version(self):
        manifest = {'php': '8.5.11', 'extensions': [], 'composer': '2.10.3',
                    'nginx': '1.30.5', 'mysql': '9.7.2', 'mailpit': '1.31.6'}
        for mailpit_version in ('1.31.6', '0.0.0'):
            def offline_output(command, **kwargs):
                if '-r' in command:
                    return json.dumps({'version': manifest['php'], 'extensions': []})
                if 'version' in command:
                    if '--no-release-check' not in command:
                        raise OSError('Release lookup is unavailable offline')
                    return f'mailpit v{mailpit_version} compiled with Go'
                if any('composer.phar' in str(part) for part in command):
                    return f'Composer version {manifest["composer"]} built'
                if '-v' in command:
                    return f'nginx/{manifest["nginx"]}'
                return f'mysqld Ver {manifest["mysql"]} bundled'

            with self.subTest(version=mailpit_version), tempfile.TemporaryDirectory() as folder, \
                    patch.object(runtime, 'MANIFEST', manifest), \
                    patch.object(runtime.platform, 'system', return_value='Darwin'), \
                    patch.object(runtime.subprocess, 'check_output', side_effect=offline_output), \
                    patch('sys.stdout', new=io.StringIO()):
                (Path(folder) / 'manifest.json').write_text(json.dumps(manifest))
                if mailpit_version == manifest['mailpit']:
                    runtime.verify(Path(folder))
                else:
                    with self.assertRaisesRegex(RuntimeError, 'Runtime version mismatch'):
                        runtime.verify(Path(folder))

    def test_verification_rejects_missing_or_stale_staged_manifest_before_running_binaries(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(runtime.subprocess, 'check_output') as execute:
            target = Path(folder)
            with self.assertRaisesRegex(RuntimeError, 'manifest is missing'):
                runtime.verify(target)
            stale = {**runtime.MANIFEST, 'phpVersions': {**runtime.MANIFEST['phpVersions'], '8.0': '8.0.30'}}
            (target / 'manifest.json').write_text(json.dumps(stale))
            with self.assertRaisesRegex(RuntimeError, 'manifest is out of date'):
                runtime.verify(target)
            execute.assert_not_called()

    def test_verification_rejects_removed_php_binaries_even_with_a_current_manifest(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(runtime.subprocess, 'check_output') as execute:
            target = Path(folder)
            (target / 'manifest.json').write_text(json.dumps(runtime.MANIFEST))
            (target / 'php-8.0').mkdir()
            (target / 'php-8.1').mkdir()
            with self.assertRaisesRegex(RuntimeError, 'unexpected PHP folders: php-8.0, php-8.1'):
                runtime.verify(target)
            execute.assert_not_called()

    def test_php_8_0_and_8_1_are_not_bundled(self):
        root = Path(__file__).resolve().parents[1]
        manifest = json.loads((root / 'runtime-manifest.json').read_text())
        self.assertTrue(manifest['phpVersions'])
        self.assertEqual(manifest['phpVersions']['.'.join(manifest['php'].split('.')[:2])], manifest['php'])
        for minor, version in manifest['phpVersions'].items():
            self.assertTrue(version.startswith(minor + '.'))
        self.assertFalse((root / 'scripts/windows_legacy_sources.json').exists())
        downloads = json.loads((root / 'runtime-downloads.lock.json').read_text())
        linux = json.loads((root / 'runtime-linux-downloads.lock.json').read_text())
        sources = json.loads((root / 'runtime-sources.lock.json').read_text())
        for name in (*downloads, *linux, *(item['name'] for item in sources)):
            self.assertNotRegex(name, r'(?:^|[-@])php[-@]8\.[01](?:\D|$)')
        self.assertIn('brew-icu4c@78-arm64-sequoia', downloads)
        for item in sources:
            versions = set(item.get('phpVersions') or [])
            self.assertFalse(versions & {'8.0', '8.1'})

    def test_windows_archive_cannot_escape_runtime_folder(self):
        for malicious in ('../outside.txt', '/outside.txt', 'C:/outside.txt', '..\\outside.txt'):
            with self.subTest(path=malicious), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                archive = root / 'archive.zip'
                with zipfile.ZipFile(archive, 'w') as bundle: bundle.writestr(malicious, 'unsafe')
                with self.assertRaises(ValueError): runtime.safe_extract(archive, root / 'out', 'zip')
                self.assertFalse((root / 'outside.txt').exists())

    def test_valid_archive_extracts(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            archive = root / 'archive.zip'
            with zipfile.ZipFile(archive, 'w') as bundle: bundle.writestr('php/bin/php.exe', 'binary')
            runtime.safe_extract(archive, root / 'out', 'zip')
            self.assertEqual((root / 'out/php/bin/php.exe').read_text(), 'binary')

    def test_bottle_external_link_is_skipped_without_writing_outside(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            archive = root / 'bottle.tar'
            with tarfile.open(archive, 'w') as bundle:
                link = tarfile.TarInfo('nginx/html')
                link.type = tarfile.SYMTYPE
                link.linkname = '../../outside'
                bundle.addfile(link)
            with self.assertRaises(tarfile.LinkOutsideDestinationError):
                runtime.safe_extract(archive, root / 'strict', 'tar')
            runtime.safe_extract(archive, root / 'bottle', 'tar', skip_external_links=True)
            self.assertFalse((root / 'bottle/nginx/html').exists())
            self.assertFalse((root / 'outside').exists())

    def test_checksum_mismatch_refuses_cached_download(self):
        with tempfile.TemporaryDirectory() as folder:
            previous = runtime.CACHE
            runtime.CACHE = Path(folder)
            try:
                url = 'https://example.invalid/runtime.zip'
                cached = runtime.CACHE / hashlib.sha256(url.encode()).hexdigest()
                cached.write_bytes(b'tampered')
                with self.assertRaises(RuntimeError): runtime.download(url, '0' * 64)
                self.assertFalse(cached.exists())
            finally: runtime.CACHE = previous


if __name__ == '__main__': unittest.main()
