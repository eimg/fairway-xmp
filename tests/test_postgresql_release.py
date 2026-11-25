"""Release integrity and archive boundaries without certificates or payload execution."""
import hashlib
import base64
import io
import json
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import postgresql_release as release
import check_release_setup as setup
import sign_macos


class ReleaseSetupTests(unittest.TestCase):
    def test_inherited_key_and_teacher_identity_are_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); (root / 'packaging').mkdir()
            key = root / 'packaging/updater.pub'
            material = b'Ed' + b'K' * 40
            for comment in (b'untrusted comment: inherited\n', b'untrusted comment: changed comment\r\n'):
                key.write_bytes(base64.b64encode(comment + base64.b64encode(material) + b'\r\n') + b'\r\n')
                with mock.patch.object(setup, 'INHERITED_UPDATER_KEY_SHA256', hashlib.sha256(material).hexdigest()), self.assertRaisesRegex(ValueError, 'own updater public key'):
                    setup.check(updater=True, environ={}, root=root)
            setup.check(updater=True, environ={}, root=root)
        with self.assertRaisesRegex(ValueError, 'own Apple Developer'):
            setup.check(apple=True, environ={'FAIRWAY_APPLE_TEAM_ID': '4Q27NQ2UAF'})
        with self.assertRaisesRegex(ValueError, 'checkout must belong'):
            setup.check(environ={'GITHUB_REPOSITORY': 'eimg/fairway-xmp'})

    def test_matching_identities_and_reviewed_runtime_are_required(self):
        env = {'FAIRWAY_APPLE_TEAM_ID': 'ABCDEFGHIJ', 'APPLE_SIGNING_IDENTITY': 'Developer ID Application: Fixture (ABCDEFGHIJ)', 'APPLE_INSTALLER_IDENTITY': 'Developer ID Installer: Fixture (ABCDEFGHIJ)', 'FAIRWAY_POSTGRES_RUNTIME_RUN': '12345'}
        setup.check(apple=True, postgres=True, environ=env)
        for name, value in [('APPLE_INSTALLER_IDENTITY', 'Developer ID Installer: Wrong (0123456789)'), ('FAIRWAY_POSTGRES_RUNTIME_RUN', '../other'), ('APPLE_SIGNING_IDENTITY', env['APPLE_SIGNING_IDENTITY'] + '\n')]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                setup.check(apple=True, postgres=True, environ={**env, name: value})


@unittest.skipIf(os.name == 'nt', 'Private macOS payload mode and alias checks')
class PostgreSQLReleaseTests(unittest.TestCase):
    def fixture(self, root):
        payload = root / 'postgresql'
        payload.mkdir(mode=0o700)
        for name in ('bin', 'licenses'):
            (payload / name).mkdir(mode=0o700)
        for name in release.TOOLS:
            path = payload / 'bin' / name
            path.write_bytes(b'\xcf\xfa\xed\xfe' + b'fixture native bytes')
            path.chmod(0o500)
        archive = payload / 'licenses/postgresql-18.6-source.tar.bz2'
        archive.write_bytes(b'unchanged source fixture')
        for name in ('PostgreSQL-COPYRIGHT', 'timezone-README'):
            (payload / 'licenses' / name).write_bytes(b'preserved notice')
        for path in (payload / 'licenses').iterdir():
            path.chmod(0o400)
        lock = {'version': '18.6', 'platform': 'macos-arm64', 'sha256': release.digest(archive)}
        manifest = {'schemaVersion': 1, 'version': '18.6', 'platform': 'macos-arm64', 'files': release.inventory(payload)}
        release.write(payload / 'manifest.json', manifest)
        pin = {'version': '18.6', 'platform': 'macos-arm64', 'manifestSha256': release.digest(payload / 'manifest.json')}
        return payload, lock, pin

    def test_changed_bytes_pin_aliases_and_notices_fail_closed(self):
        with tempfile.TemporaryDirectory() as folder:
            payload, lock, pin = self.fixture(Path(folder))
            original_read = release.read
            with mock.patch.object(release, 'read', side_effect=lambda path: lock if path.name == 'postgresql-source.lock.json' else original_read(path)):
                release.verify_payload(payload, pin)
                with self.assertRaisesRegex(ValueError, 'reviewed application pin'):
                    release.verify_payload(payload, {**pin, 'manifestSha256': '0' * 64})
                tool = payload / 'bin/psql'
                tool.chmod(0o700); tool.write_bytes(b'changed'); tool.chmod(0o500)
                with self.assertRaisesRegex(ValueError, 'manifest/tree mismatch'):
                    release.verify_payload(payload, pin)
                (payload / 'alias').symlink_to(tool)
                with self.assertRaises(ValueError): release.verify_payload(payload, pin)
                (payload / 'alias').unlink()
                os.link(tool, payload / 'hardlink')
                with self.assertRaises(ValueError): release.verify_payload(payload, pin)

    def test_archive_validation_precedes_extraction(self):
        cases = [('../escape', tarfile.REGTYPE), ('/absolute', tarfile.REGTYPE), ('postgresql/link', tarfile.SYMTYPE), ('postgresql/hardlink', tarfile.LNKTYPE), ('unexpected', tarfile.REGTYPE), ('postgresql/../escape', tarfile.REGTYPE), ('postgresql/safe', tarfile.REGTYPE)]
        for name, kind in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as folder:
                root = Path(folder); archive = root / 'archive.tar.gz'
                with tarfile.open(archive, 'w:gz') as bundle:
                    good = tarfile.TarInfo('postgresql/safe'); good.mode = 0o400; good.size = 4
                    bundle.addfile(good, io.BytesIO(b'kept'))
                    bad = tarfile.TarInfo(name); bad.mode = 0o400; bad.type = kind
                    bundle.addfile(bad)
                stage = root / 'stage'; stage.mkdir(mode=0o700)
                with self.assertRaises(ValueError): release.unpack(archive, stage)
                self.assertEqual(list(stage.iterdir()), [])

    def test_public_bundle_modes_keep_the_pin_and_reject_shared_writes(self):
        with tempfile.TemporaryDirectory() as folder:
            payload, lock, pin = self.fixture(Path(folder))
            original_read = release.read
            with mock.patch.object(release, 'read', side_effect=lambda path: lock if path.name == 'postgresql-source.lock.json' else original_read(path)):
                for path in payload.rglob('*'):
                    path.chmod(0o755 if path.is_dir() else 0o555 if path.stat().st_mode & 0o100 else 0o444)
                payload.chmod(0o755)
                release.verify_payload(payload, pin, bundled=True)
                with self.assertRaises(ValueError): release.verify_payload(payload, pin)
                (payload / 'bin/psql').chmod(0o775)
                with self.assertRaises(ValueError): release.verify_payload(payload, pin, bundled=True)

    @unittest.skipUnless(sys.platform == 'darwin', 'macOS exclusive rename and fsync')
    def test_staging_requires_reviewed_pin_then_publishes_readable_bytes_without_overwrite(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve(); payload, lock, pin = self.fixture(root)
            release.write(root / 'runtime-pin.json', pin)
            release.write(root / 'provenance.json', {'teamId': 'ABCDEFGHIJ', 'sourceLockSha256': release.digest(ROOT / 'postgresql-source.lock.json'), 'recipeSha256': release.digest(ROOT / 'scripts/prepare_postgresql_runtime.py')})
            archive = root / 'runtime.tar.gz'
            with tarfile.open(archive, 'w:gz') as bundle:
                for name in ('postgresql', 'runtime-pin.json', 'provenance.json'):
                    bundle.add(root / name, arcname=name)
            reviewed = root / 'reviewed.json'
            release.write(reviewed, {**pin, 'manifestSha256': '0' * 64})
            original_read = release.read
            with mock.patch.object(release, 'PIN', reviewed), mock.patch.object(release, 'read', side_effect=lambda path: lock if path.name == 'postgresql-source.lock.json' else original_read(path)), mock.patch.object(release, 'verify_code') as verify_code:
                with self.assertRaisesRegex(ValueError, 'Commit the reviewed signed runtime pin'):
                    release.stage_archive(archive, root / 'output', 'ABCDEFGHIJ')
                verify_code.assert_not_called()
                reviewed.chmod(0o600); reviewed.unlink(); release.write(reviewed, pin)
                release.stage_archive(archive, root / 'output', 'ABCDEFGHIJ')
                release.verify_payload(root / 'output', pin, bundled=True)
                self.assertEqual((root / 'output').stat().st_mode & 0o777, 0o755)
                with self.assertRaisesRegex(ValueError, 'new output'):
                    release.stage_archive(archive, root / 'output', 'ABCDEFGHIJ')
                release.verify_payload(payload, pin)

    def test_signs_only_a_new_copy_and_hashes_the_signed_bytes(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve(); payload, lock, original_pin = self.fixture(root)
            output = root / 'release'
            original_read = release.read
            def sign_command(args, **kwargs):
                path = Path(args[-1])
                with path.open('ab') as stream: stream.write(b' signed fixture')
            with mock.patch.object(release, 'read', side_effect=lambda path: lock if path.name == 'postgresql-source.lock.json' else original_read(path)), mock.patch.object(release.subprocess, 'run', side_effect=sign_command), mock.patch.object(release, 'verify_code'), mock.patch.object(release, 'publish', side_effect=lambda stage, output: stage.rename(output)):
                release.sign(payload, output, 'Developer ID Application: Fixture (ABCDEFGHIJ)', 'ABCDEFGHIJ')
                pin = release.read(output / 'runtime-pin.json')
                self.assertNotEqual(pin['manifestSha256'], original_pin['manifestSha256'])
                release.verify_payload(output / 'postgresql', pin)
                release.verify_payload(payload, original_pin)
                with self.assertRaisesRegex(ValueError, 'new output'):
                    release.sign(payload, output, 'Developer ID Application: Fixture (ABCDEFGHIJ)', 'ABCDEFGHIJ')

    def test_app_signer_excludes_pinned_postgresql_but_keeps_other_nested_code(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); app = root / 'Fairway.app'
            import plistlib
            (app / 'Contents').mkdir(parents=True)
            (app / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable': 'fairway'}))
            paths = [app / 'Contents/Resources/runtime/postgresql/bin/psql', app / 'Contents/Resources/runtime/php/bin/php']
            for path in paths:
                path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b'\xcf\xfa\xed\xfe')
            self.assertEqual(sign_macos.signing_targets(app), [paths[1]])


if __name__ == '__main__':
    unittest.main()
