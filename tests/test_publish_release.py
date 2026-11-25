import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))


spec = importlib.util.spec_from_file_location(
    'publish_release', Path(__file__).resolve().parents[1] / 'scripts/publish_release.py',
)
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class ReleaseTagTests(unittest.TestCase):
    def test_owner_dispatched_publication_needs_no_device_acceptance_and_keeps_release_checks(self):
        for version, prerelease, prebuilt in [(version, prerelease, prebuilt) for version, prerelease in [('0.1.0-beta.3', True), ('0.1.0-rc.1', True), ('0.1.0', False)] for prebuilt in (False, True)]:
            with self.subTest(version=version, prebuilt=prebuilt), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); fixture = root / 'fixture'; fixture.mkdir()
                sha = 'a' * 40
                manifest = {'version': version, 'commit': sha, 'status': 'awaiting-human-acceptance', 'signing': {'macos-arm64': 'apple-notarized', 'windows-x64': 'updater-only', 'linux-x86_64': 'unsigned'}, 'candidates': {}}
                for target, suffix in [('macos-arm64', 'macos-arm64.app.tar.gz'), ('windows-x64', 'windows-x64-setup.exe'), ('linux-x86_64', 'linux-x86_64.run')]:
                    directory = fixture / f'fairway-xmp-{target}-{version}'; directory.mkdir()
                    installer = f'fairway-xmp-{version}-{suffix}'
                    (directory / installer).write_bytes(b'tested bytes')
                    updater = {'artifact': installer, 'signature': installer + '.sig'}
                    if target != 'linux-x86_64': (directory / updater['signature']).write_text('fixture signature')
                    files = {p.name: {'sha256': publisher.sha256(p), 'bytes': p.stat().st_size} for p in directory.iterdir()}
                    data = {'version': version, 'commit': sha, 'installer': installer, 'files': files, 'updater': updater, 'lifecycle': {'updater': {'status': 'passed'}}}
                    manifest['candidates'][target] = data
                    metadata = fixture / f'candidate-metadata-{target}'; metadata.mkdir()
                    (metadata / f'candidate-{target}.json').write_text(json.dumps(data))
                (fixture / f'fairway-xmp-linux-x86_64-{version}-sources').mkdir()
                macos = fixture / f'fairway-xmp-macos-arm64-{version}'
                for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'runtime-manifest.json', 'runtime-downloads.lock.json']:
                    (macos / name).write_text('{}')
                metadata = fixture / f'candidate-acceptance-{sha}'; metadata.mkdir()
                (metadata / 'candidate-manifest.json').write_text(json.dumps(manifest))
                notes = root / 'notes.md'; notes.write_text('Reviewed release notes')
                uploaded = {}; commands = []; build = {}; fail_signature = False
                def run_json(*args):
                    if args[:3] == ('gh', 'run', 'view'): return {'conclusion': 'success', 'headSha': sha, 'url': 'https://example.invalid/run'}
                    return [uploaded] if uploaded else []
                def execute(args, **kwargs):
                    commands.append(args)
                    if args[0] in ('cargo', str(root / 'verify_update')) and fail_signature:
                        raise subprocess.CalledProcessError(1, args)
                    if args[:3] == ['gh', 'run', 'download']:
                        shutil.copytree(fixture, Path(args[args.index('--dir') + 1]), dirs_exist_ok=True)
                    if args[:3] == ['gh', 'release', 'create']:
                        paths = [arg for arg in args if isinstance(arg, Path)]
                        build.update(json.loads(next(p for p in paths if p.name == 'BUILD-INFO.json').read_text()))
                        self.assertFalse(any(p.name == 'acceptance.json' for p in paths))
                        uploaded.update({'id': 42, 'tag_name': 'v' + version, 'draft': True, 'assets': [
                            {'name': p.name, 'digest': 'sha256:' + publisher.sha256(p), 'size': p.stat().st_size} for p in paths]})
                with mock.patch.dict(os.environ, {'GITHUB_ACTIONS': 'true' if prebuilt else 'false', 'FAIRWAY_UPDATE_VERIFIER': str(root / 'verify_update') if prebuilt else '', 'GH_REPO': 'RizzRioo06/fairway-xmp', 'FAIRWAY_RUN_ID': '123', 'FAIRWAY_TAG': 'v' + version, 'FAIRWAY_SHA': sha, 'FAIRWAY_NOTES': str(notes), 'FAIRWAY_ACCEPTANCE': ''}), \
                     mock.patch.object(publisher, 'run_json', side_effect=run_json), mock.patch.object(publisher, 'verify_tag'), \
                     mock.patch.object(publisher.subprocess, 'check_call', side_effect=execute), mock.patch.object(publisher.subprocess, 'check_output', return_value=b'{}'), mock.patch('builtins.print'):
                    publisher.main(signed=True)
                create = next(args for args in commands if args[:3] == ['gh', 'release', 'create'])
                self.assertEqual('--prerelease' in create, prerelease)
                self.assertIn('--latest=' + str(not prerelease).lower(), create)
                patch = next(args for args in commands if args[:3] == ['gh', 'api', '--method'])
                self.assertIn('prerelease=' + str(prerelease).lower(), patch)
                self.assertIn('make_latest=' + str(not prerelease).lower(), patch)
                self.assertEqual(build['prerelease'], prerelease)
                self.assertFalse(build['humanAcceptanceRequired'])
                self.assertNotIn('humanAccepted', build)
                self.assertEqual(build['publication']['method'], 'workflow-dispatch' if prebuilt else 'explicit-invocation')
                self.assertEqual(len([args for args in commands if args[0] == ('cargo' if not prebuilt else str(root / 'verify_update'))]), 2)
                if prebuilt:
                    self.assertFalse(any(args[0] == 'cargo' for args in commands))
                # Owner-authorized publication still verifies the automated drill and signatures.
                for failure in ('native-drill', 'signature'):
                    uploaded.clear(); commands.clear()
                    fail_signature = failure == 'signature'
                    if failure == 'native-drill':
                        manifest['candidates']['windows-x64']['lifecycle']['updater']['status'] = 'failed'
                    else:
                        manifest['candidates']['windows-x64']['lifecycle']['updater']['status'] = 'passed'
                    (metadata / 'candidate-manifest.json').write_text(json.dumps(manifest))
                    with self.subTest(failure=failure), mock.patch.dict(os.environ, {'GITHUB_ACTIONS': 'true' if prebuilt else 'false', 'FAIRWAY_UPDATE_VERIFIER': str(root / 'verify_update') if prebuilt else '', 'GH_REPO': 'RizzRioo06/fairway-xmp', 'FAIRWAY_RUN_ID': '123', 'FAIRWAY_TAG': 'v' + version, 'FAIRWAY_SHA': sha, 'FAIRWAY_NOTES': str(notes), 'FAIRWAY_ACCEPTANCE': ''}), \
                         mock.patch.object(publisher, 'run_json', side_effect=run_json), mock.patch.object(publisher, 'verify_tag'), \
                         mock.patch.object(publisher.subprocess, 'check_call', side_effect=execute), mock.patch.object(publisher.subprocess, 'check_output', return_value=b'{}'), \
                         self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        publisher.main(signed=True)
                    self.assertFalse(any(args[:3] == ['gh', 'release', 'create'] for args in commands))

    def test_uploaded_draft_is_found_without_the_published_tag_endpoint(self):
        release = {'id': 42, 'tag_name': 'v0.1.0-beta.3', 'draft': True}
        with mock.patch.object(publisher, 'run_json', return_value=[release]) as api:
            self.assertEqual(publisher.find_draft_release('RizzRioo06/fairway-xmp', release['tag_name']), release)
            api.assert_called_once_with('gh', 'api', 'repos/RizzRioo06/fairway-xmp/releases?per_page=100')
        for releases in ([], [release, release], [{**release, 'draft': False}]):
            with self.subTest(releases=releases), mock.patch.object(publisher, 'run_json', return_value=releases):
                with self.assertRaises(RuntimeError):
                    publisher.find_draft_release('RizzRioo06/fairway-xmp', release['tag_name'])

    def test_uploaded_assets_require_the_exact_set_and_content(self):
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            artifact = stage / 'installer.pkg'
            artifact.write_bytes(b'tested installer')
            asset = {'name': artifact.name, 'size': artifact.stat().st_size, 'digest': 'sha256:' + publisher.sha256(artifact)}
            publisher.verify_uploaded_assets({'assets': [asset]}, stage)
            for assets in ([], [asset, asset], [{**asset, 'digest': 'sha256:' + '0' * 64}],
                           [{**asset, 'size': 0}], [asset, {**asset, 'name': 'unexpected.exe'}]):
                with self.subTest(assets=assets), self.assertRaises(RuntimeError):
                    publisher.verify_uploaded_assets({'assets': assets}, stage)

    def test_release_tag_must_resolve_to_the_tested_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def git(*args):
                return subprocess.check_output([
                    'git', '-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false',
                    '-c', 'user.name=Release test', '-c', 'user.email=release-test@example.invalid',
                    *args,
                ], cwd=root, text=True).strip()

            git('init', '--quiet')
            git('commit', '--quiet', '--allow-empty', '-m', 'Tested candidate')
            candidate = git('rev-parse', 'HEAD')
            git('tag', 'v0.1.0-beta.2')
            publisher.verify_tag('v0.1.0-beta.2', candidate, root)
            git('commit', '--quiet', '--allow-empty', '-m', 'Later documentation')
            later = git('rev-parse', 'HEAD')
            with self.assertRaisesRegex(RuntimeError, 'does not identify the tested candidate'):
                publisher.verify_tag('v0.1.0-beta.2', later, root)
            git('tag', '-a', 'annotated-beta', '-m', 'Candidate', candidate)
            publisher.verify_tag('annotated-beta', candidate, root)
