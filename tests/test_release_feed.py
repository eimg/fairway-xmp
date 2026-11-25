import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
import publish_release_feed as publisher


class ReleaseFeedTests(unittest.TestCase):
    def test_stable_feed_rejects_prereleases_before_any_github_request(self):
        for version in ['0.1.0-beta.4', '0.1.0-rc.1']:
            with self.subTest(version=version), mock.patch.object(publisher.subprocess, 'run') as request:
                with self.assertRaisesRegex(ValueError, 'Only stable'):
                    publisher.promote_feed('RizzRioo06/fairway-xmp', 'v' + version, Path('/unused'), 'stable-feed')
                request.assert_not_called()

    def test_preview_uses_legacy_endpoint_and_stable_uses_stable_endpoint(self):
        for version, feed in [('0.1.0-beta.4', 'beta-feed'), ('0.1.0-rc.1', 'beta-feed'), ('0.1.0', 'stable-feed')]:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); (root / 'scripts').mkdir()
                for script in ['configure_release.py', 'release_versions.py']:
                    shutil.copy2(SCRIPTS / script, root / 'scripts' / script)
                (root / 'package.json').write_text(json.dumps({'version': version}))
                key = root / 'updater.pub'; key.write_text('test public key')
                output = root / 'overlay.json'
                subprocess.run([sys.executable, str(root / 'scripts/configure_release.py'), '--public-key', str(key), '--output', str(output)], check=True, capture_output=True)
                self.assertEqual(json.loads(output.read_text())['plugins']['updater']['endpoints'], [f'https://github.com/RizzRioo06/fairway-xmp/releases/download/{feed}/latest.json'])

    def test_feed_promotion_routes_rc_and_stable_and_checks_release_status(self):
        for version, prerelease, feeds in [('0.1.0-rc.1', True, ['beta-feed']), ('0.1.0', False, ['stable-feed', 'beta-feed'])]:
            def download(args):
                root = Path(args[args.index('--dir') + 1])
                (root / 'latest.json').write_text(json.dumps({'version': version, 'platforms': {
                    platform: {'url': f'https://github.com/RizzRioo06/fairway-xmp/releases/download/v{version}/{platform}', 'signature': 'test signature'}
                    for platform in ['darwin-aarch64', 'windows-x86_64']}}))
            with self.subTest(version=version), mock.patch.dict(os.environ, {'GH_REPO': 'RizzRioo06/fairway-xmp', 'FAIRWAY_TAG': 'v' + version}), \
                 mock.patch.object(publisher.subprocess, 'check_output', return_value=json.dumps({'isDraft': False, 'isPrerelease': prerelease})), \
                 mock.patch.object(publisher.subprocess, 'check_call', side_effect=download), mock.patch.object(publisher, 'promote_feed') as promote, mock.patch('builtins.print'):
                publisher.main()
                self.assertEqual([call.args[3] for call in promote.call_args_list], feeds)
            for bad in [{'isDraft': True, 'isPrerelease': prerelease}, {'isDraft': False, 'isPrerelease': not prerelease}]:
                with mock.patch.dict(os.environ, {'GH_REPO': 'RizzRioo06/fairway-xmp', 'FAIRWAY_TAG': 'v' + version}), \
                     mock.patch.object(publisher.subprocess, 'check_output', return_value=json.dumps(bad)), \
                     mock.patch.object(publisher, 'promote_feed') as promote, mock.patch('builtins.print'):
                    with self.assertRaises(ValueError): publisher.main()
                    promote.assert_not_called()

    def test_downgrades_are_blocked_and_newer_preview_survives_stable_promotion(self):
        for old, new, feed, result in [
            ('0.1.0-beta.3', '0.1.0-rc.1', 'beta-feed', 'upload'),
            ('0.1.0-rc.1', '0.1.0', 'beta-feed', 'upload'),
            ('0.1.0-rc.2', '0.1.0-rc.1', 'beta-feed', 'reject'),
            ('0.1.0', '0.1.0-rc.9', 'beta-feed', 'reject'),
            ('0.2.0-rc.1', '0.1.1', 'beta-feed', 'keep'),
            ('0.2.0', '0.1.1', 'stable-feed', 'reject'),
        ]:
            with self.subTest(old=old, new=new, feed=feed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                def download(args):
                    if args[:3] == ['gh', 'release', 'download']:
                        (Path(args[args.index('--dir') + 1]) / 'latest.json').write_text(json.dumps({'version': old}))
                with mock.patch.object(publisher.subprocess, 'run', return_value=mock.Mock(returncode=0)), \
                     mock.patch.object(publisher.subprocess, 'check_call', side_effect=download) as calls, mock.patch('builtins.print'):
                    if result == 'reject':
                        with self.assertRaisesRegex(ValueError, 'backwards'): publisher.promote_feed('RizzRioo06/fairway-xmp', 'v' + new, root, feed)
                    else:
                        publisher.promote_feed('RizzRioo06/fairway-xmp', 'v' + new, root, feed)
                    uploads = [call for call in calls.call_args_list if call.args[0][:3] == ['gh', 'release', 'upload']]
                    self.assertEqual(bool(uploads), result == 'upload')

    def test_feed_creation_does_not_mask_api_or_existing_release_errors(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for response in [subprocess.CalledProcessError(1, 'gh api'), json.dumps([{'tag_name': 'beta-feed'}])]:
                with mock.patch.object(publisher.subprocess, 'run', return_value=mock.Mock(returncode=1)), \
                     mock.patch.object(publisher.subprocess, 'check_output', side_effect=[response]), \
                     mock.patch.object(publisher.subprocess, 'check_call') as mutate:
                    with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        publisher.promote_feed('RizzRioo06/fairway-xmp', 'v0.1.0-rc.1', root, 'beta-feed')
                    mutate.assert_not_called()
            with mock.patch.object(publisher.subprocess, 'run', return_value=mock.Mock(returncode=1)), \
                 mock.patch.object(publisher.subprocess, 'check_output', return_value='[]'), \
                 mock.patch.object(publisher.subprocess, 'check_call') as mutate:
                publisher.promote_feed('RizzRioo06/fairway-xmp', 'v0.1.0', root, 'stable-feed')
                args = mutate.call_args.args[0]
                self.assertEqual(args[:4], ['gh', 'release', 'create', 'stable-feed'])
                self.assertIn('--prerelease', args)
                self.assertIn('--latest=false', args)
