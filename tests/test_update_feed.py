import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
import release_candidate as candidate
import update_feed
import release_versions


class UpdateFeedTests(unittest.TestCase):
    def fixture(self, root, version='0.1.0-beta.3'):
        manifest = {'version': version, 'commit': 'a' * 40, 'signing': {'macos-arm64': 'apple-notarized', 'windows-x64': 'updater-only', 'linux-x86_64': 'unsigned'}, 'candidates': {}}
        directories = {}
        for target, name in [('macos-arm64', f'fairway-xmp-{version}-macos-arm64.app.tar.gz'), ('windows-x64', f'fairway-xmp-{version}-windows-x64-setup.exe')]:
            directory = root / target; directory.mkdir(); directories[target] = directory
            (directory / name).write_bytes(b'exact candidate bytes')
            (directory / (name + '.sig')).write_text('test signature fixture')
            manifest['candidates'][target] = {'version': version, 'commit': manifest['commit'], 'lifecycle': {'updater': {'status': 'passed'}}, 'updater': {'artifact': name, 'signature': name + '.sig'}, 'files': {p.name: {'sha256': candidate.digest(p), 'bytes': p.stat().st_size} for p in directory.iterdir()}}
        return manifest, directories

    def test_feed_uses_verified_versioned_assets_and_both_platforms(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest, directories = self.fixture(Path(temporary))
            result = update_feed.generate(manifest, directories, 'eimg/fairway-xmp', 'Beta notes', '2026-10-16T00:00:00Z')
            self.assertEqual(set(result['platforms']), {'darwin-aarch64', 'windows-x86_64'})
            self.assertEqual(result['version'], '0.1.0-beta.3')
            for platform in result['platforms'].values():
                self.assertIn('/releases/download/v0.1.0-beta.3/', platform['url'])
                self.assertNotIn('/latest/', platform['url'])
            file = directories['windows-x64'] / manifest['candidates']['windows-x64']['updater']['artifact']
            file.write_bytes(b'tampered')
            with self.assertRaisesRegex(ValueError, 'changed or is missing'):
                update_feed.generate(manifest, directories, 'eimg/fairway-xmp', '')

    def test_feed_rejects_missing_signing_or_native_update_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest, directories = self.fixture(Path(temporary))
            for mutate in [lambda m: m['signing'].update({'macos-arm64': 'unsigned'}), lambda m: m['candidates']['windows-x64']['lifecycle']['updater'].update({'status': 'failed'}), lambda m: m['candidates']['macos-arm64'].update({'commit': 'b' * 40})]:
                bad = copy.deepcopy(manifest); mutate(bad)
                with self.assertRaises(ValueError): update_feed.generate(bad, directories, 'eimg/fairway-xmp', '')

    def test_beta_rc_stable_order_and_invalid_versions(self):
        versions = ['0.1.0-beta.9', '0.1.0-beta.10', '0.1.0-rc.1', '0.1.0-rc.10', '0.1.0', '0.1.1-beta.1', '0.2.0-rc.1', '1.0.0']
        self.assertEqual(sorted(reversed(versions), key=release_versions.version_key), versions)
        for invalid in ['v0.1.0', '0.1.0-rc', '0.1.0-rc.01', '00.1.0', '0.1.0-beta.1.extra', '0.1.0+build', '0.1.0-alpha.1']:
            with self.subTest(version=invalid), self.assertRaises(ValueError): release_versions.version_key(invalid)

    def test_rc_and_stable_feeds_use_exact_verified_versioned_artifacts(self):
        for version in ['0.1.0-rc.1', '0.1.0']:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as temporary:
                manifest, directories = self.fixture(Path(temporary), version)
                feed = update_feed.generate(manifest, directories, 'eimg/fairway-xmp', 'Reviewed notes')
                self.assertEqual(feed['version'], version)
                self.assertTrue(all(f'/releases/download/v{version}/' in entry['url'] for entry in feed['platforms'].values()))

    def test_release_overlay(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            scripts = root / 'scripts'
            scripts.mkdir()
            for name in ('configure_release.py', 'release_versions.py'):
                (scripts / name).write_bytes((SCRIPTS / name).read_bytes())
            output = root / 'overlay.json'
            for version, feed in [('0.1.0-rc.5', 'beta-feed'), ('0.1.0', 'stable-feed')]:
                with self.subTest(version=version):
                    (root / 'package.json').write_text(json.dumps({'version': version}))
                    subprocess.run([sys.executable, str(scripts / 'configure_release.py'), '--public-key', str(SCRIPTS.parent / 'packaging/updater.pub'), '--output', str(output)], check=True, capture_output=True)
                    config = json.loads(output.read_text())
                    self.assertTrue(config['bundle']['createUpdaterArtifacts'])
                    self.assertTrue(config['plugins']['updater']['requireSignedVersion'])
                    self.assertEqual(config['plugins']['updater']['endpoints'], [f'https://github.com/eimg/fairway-xmp/releases/download/{feed}/latest.json'])
            result = subprocess.run([sys.executable, str(SCRIPTS / 'configure_release.py'), '--public-key', str(SCRIPTS.parent / 'packaging/updater.pub'), '--endpoint', 'http://insecure.invalid'], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
