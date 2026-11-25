"""Compile-time pin isolation and sharing identity checks without personal data."""
import json
import hashlib
import io
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tarfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from package_macos_sharing import verify_identity
import package_macos_sharing as sharing


class SharingIdentityTests(unittest.TestCase):
    def test_source_archives_must_match_commit_runtime_pin_and_official_postgresql_source(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            pg_source = b'unchanged source fixture'
            lock = {'version': '18.6', 'sha256': hashlib.sha256(pg_source).hexdigest()}
            (root / 'postgresql-source.lock.json').write_text(json.dumps(lock))
            (root / 'runtime-manifest.json').write_text('{"php":"fixture"}')
            pin = {'manifestSha256': 'a' * 64}
            entries = {'README.md': b'Fairway XMP source commit: reviewed.', 'runtime-manifest.json': b'{"php":"fixture"}', 'postgresql/runtime-pin.json': json.dumps(pin).encode(), 'postgresql/postgresql-source.lock.json': json.dumps(lock).encode(), 'postgresql/postgresql-18.6-source.tar.bz2': pg_source, 'postgresql/PostgreSQL-COPYRIGHT': b'notice', 'postgresql/timezone-README': b'notice', 'fairway-xmp-source.tar.gz': b'app sources'}
            archive = root / 'sources.tar.gz'
            with tarfile.open(archive, 'w:gz') as bundle:
                for name, data in entries.items():
                    member = tarfile.TarInfo('fairway-xmp-0.2.0-rc.1-sources/' + name); member.size = len(data)
                    bundle.addfile(member, io.BytesIO(data))
            with mock.patch.object(sharing, 'ROOT', root):
                sharing.verify_sources(archive, pin, 'reviewed', '0.2.0-rc.1')
                with self.assertRaisesRegex(ValueError, 'another commit'):
                    sharing.verify_sources(archive, pin, 'other', '0.2.0-rc.1')
                with self.assertRaisesRegex(ValueError, 'another PostgreSQL payload'):
                    sharing.verify_sources(archive, {}, 'reviewed', '0.2.0-rc.1')
                (root / 'postgresql-source.lock.json').write_text(json.dumps({**lock, 'sha256': '0' * 64}))
                with self.assertRaises(ValueError):
                    sharing.verify_sources(archive, pin, 'reviewed', '0.2.0-rc.1')

    def test_free_workflow_uses_no_credentials_or_publication(self):
        text = (ROOT / '.github/workflows/free-sharing-builds.yml').read_text()
        for forbidden in ('${{ secrets.', 'check_release_setup.py', 'publish_release.py', 'configure_release.py', '--features native-macos-trust,native-macos-hosts'):
            self.assertNotIn(forbidden, text)
        self.assertIn('signed_release: false', text)
        self.assertIn('authorize_postgresql_source_build', text)
        self.assertIn('FAIRWAY_DISTRIBUTION: free-sharing', text)
        self.assertLess(text.index('configure_free_sharing.py'), text.index('npm run tauri -- build'))
        self.assertIn('scripts/package_sources.py --locked', text)

    def test_distribution_platform_features_and_exact_pin_must_match(self):
        pin = {'version': '18.6', 'platform': 'macos-arm64', 'manifestSha256': 'a' * 64}
        identity = {'distribution': 'free-sharing', 'commit': 'b' * 40, 'version': '0.2.0-rc.1', 'os': 'macos', 'architecture': 'aarch64', 'postgresqlRuntimePin': pin, 'features': {'nativeMacosHosts': False, 'nativeMacosTrust': True}}
        verify_identity(identity, pin, 'b' * 40, '0.2.0-rc.1')
        for key, value in [('distribution', 'standard'), ('commit', 'c' * 40), ('architecture', 'x86_64'), ('postgresqlRuntimePin', {**pin, 'manifestSha256': '0' * 64}), ('features', {'nativeMacosHosts': True, 'nativeMacosTrust': True})]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                verify_identity({**identity, key: value}, pin, 'b' * 40, '0.2.0-rc.1')


class CompileTimePinTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = shutil.which('rustc') or str(Path.home() / '.cargo/bin/rustc')
        if not Path(compiler).is_file():
            raise unittest.SkipTest('Rust compiler unavailable for build-script fixtures')
        cls.scratch = tempfile.TemporaryDirectory()
        cls.binary = Path(cls.scratch.name) / ('build-script.exe' if os.name == 'nt' else 'build-script')
        subprocess.run([compiler, '--edition=2021', str(ROOT / 'src-tauri/build.rs'), '-o', str(cls.binary)], check=True, capture_output=True)

    @classmethod
    def tearDownClass(cls):
        cls.scratch.cleanup()

    def test_standard_and_windows_builds_ignore_free_pin_and_free_mac_fails_without_it(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            default = root / 'src-tauri/src/postgresql/managed/runtime-pin.json'
            default.parent.mkdir(parents=True); default.write_text('committed pin fixture')
            free = root / 'artifacts/free-sharing/runtime-pin.json'
            free.parent.mkdir(parents=True); free.write_text('free pin fixture')
            output = root / 'out'; output.mkdir()
            env = {**os.environ, 'CARGO_MANIFEST_DIR': str(root / 'src-tauri'), 'OUT_DIR': str(output), 'CARGO_CFG_TARGET_ARCH': 'aarch64'}
            for mode, target, expected in [('standard', 'macos', 'committed pin fixture'), ('free-sharing', 'windows', 'committed pin fixture'), ('free-sharing', 'macos', 'free pin fixture')]:
                with self.subTest(mode=mode, target=target):
                    result = subprocess.run([str(self.binary)], env={**env, 'FAIRWAY_DISTRIBUTION': mode, 'CARGO_CFG_TARGET_OS': target}, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual((output / 'postgresql-runtime-pin.json').read_text(), expected)
                    self.assertEqual(default.read_text(), 'committed pin fixture')
            free.unlink()
            result = subprocess.run([str(self.binary)], env={**env, 'FAIRWAY_DISTRIBUTION': 'free-sharing', 'CARGO_CFG_TARGET_OS': 'macos'}, capture_output=True)
            self.assertNotEqual(result.returncode, 0, 'A free build must never silently fall back to the release pin')
            if os.name != 'nt':
                free.symlink_to(default)
                result = subprocess.run([str(self.binary)], env={**env, 'FAIRWAY_DISTRIBUTION': 'free-sharing', 'CARGO_CFG_TARGET_OS': 'macos'}, capture_output=True)
                self.assertNotEqual(result.returncode, 0)


if __name__ == '__main__':
    unittest.main()
