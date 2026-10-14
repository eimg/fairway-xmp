import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import tarfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location('runtime', Path(__file__).resolve().parents[1] / 'scripts/prepare_runtime.py')
runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runtime)


class ArchiveTests(unittest.TestCase):
    def test_older_windows_php_dependency_sources_are_pinned(self):
        manifest = Path(__file__).resolve().parents[1] / 'scripts/windows_legacy_sources.json'
        sources = json.loads(manifest.read_text())
        for version in ('8.0', '8.1'):
            packages = [item['package'] for item in sources if version in item['php']]
            self.assertGreaterEqual(len(packages), 35)
            self.assertEqual(len(packages), len(set(packages)))
            for library in ('glib', 'libenchant2', 'libpq', 'libsasl', 'libsodium', 'libssh2', 'nghttp2', 'openssl', 'sqlite3'):
                self.assertTrue(any(package.startswith(library + '-') for package in packages),
                                f'{library} source missing for PHP {version}')
            if version == '8.0':
                self.assertIn('glib-2.53.3-vs16-x64.zip', packages)
                self.assertIn('libiconv-1.16-5-vs16-x64.zip', packages)
        for item in sources:
            self.assertRegex(item['revision'], r'^[0-9a-f]{40}$')
            self.assertIn('/', item['repo'])

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
