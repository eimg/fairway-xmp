"""Signing policy and credential cleanup without real certificates or host keychains."""
import base64
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from verify_signing_source import verify


class SigningSourceTests(unittest.TestCase):
    def test_only_main_history_and_the_exact_build_commit_can_be_signed(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)

            def git(*args):
                return subprocess.check_output([
                    'git', '-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false',
                    '-c', 'user.name=Signing fixture', '-c', 'user.email=fixture@example.invalid', *args,
                ], cwd=root, text=True).strip()

            git('init', '--quiet', '--initial-branch=main')
            git('commit', '--quiet', '--allow-empty', '-m', 'Reviewed base')
            base = git('rev-parse', 'HEAD')
            git('commit', '--quiet', '--allow-empty', '-m', 'Reviewed main')
            main = git('rev-parse', 'HEAD')
            git('update-ref', 'refs/remotes/origin/main', main)
            git('tag', '-a', 'reviewed-comparison', '-m', 'Historic comparison', base)
            self.assertEqual(verify(root, main), main)
            self.assertEqual(verify(root, source='reviewed-comparison'), base)
            self.assertEqual(git('rev-parse', 'HEAD'), main, 'Resolving a comparison must not execute/checkout it')
            with self.assertRaisesRegex(ValueError, 'exact build commit'):
                verify(root, base)
            for invalid in ('main', '--help', '0' * 40):
                with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                    verify(root, invalid)
            git('checkout', '--quiet', '-b', 'unreviewed', base)
            git('commit', '--quiet', '--allow-empty', '-m', 'Unreviewed side branch')
            with self.assertRaisesRegex(ValueError, 'merged into origin/main'):
                verify(root)
            git('checkout', '--quiet', 'main')
            with self.assertRaisesRegex(ValueError, 'merged into origin/main'):
                verify(root, source='unreviewed')
            git('update-ref', '-d', 'refs/remotes/origin/main')
            with self.assertRaises(subprocess.CalledProcessError):
                verify(root)



@unittest.skipIf(sys.platform == 'win32', 'Bash/keychain fixture runs on Unix')
class CredentialCleanupTests(unittest.TestCase):
    def fixture(self, root):
        binary = root / 'bin'; binary.mkdir()
        tool = binary / 'security'
        tool.write_text('''#!/usr/bin/env python3
import os
from pathlib import Path
import sys
args = sys.argv[1:]
root = Path(os.environ['RUNNER_TEMP'])
with (root / 'events').open('a') as output:
    output.write(args[0] + (' restore' if '-s' in args else '') + '\\n')
if args[0] == 'list-keychains':
    if '-s' not in args:
        print('"/Users/runner/Library/Keychains/login.keychain-db"')
    elif os.environ.get('FAIL_RESTORE') == '1':
        sys.exit(29)
elif args[0] == 'create-keychain':
    Path(args[-1]).touch(mode=0o600)
elif args[0] == 'delete-keychain':
    Path(args[-1]).unlink(missing_ok=True)
elif args[0] == 'import' and os.environ.get('FAIL_IMPORT') == '1':
    sys.exit(23)
''')
        tool.chmod(0o755)
        xcrun = binary / 'xcrun'
        xcrun.write_text('#!/bin/sh\n[ "${FAIL_NOTARY:-0}" != 1 ] || exit 31\n')
        xcrun.chmod(0o755)
        return {**os.environ, 'PATH': str(binary) + os.pathsep + os.environ['PATH'],
                'RUNNER_TEMP': str(root), 'GITHUB_ENV': str(root / 'github-env'),
                'APPLE_CERTIFICATES_P12': base64.b64encode(b'fixture p12 bytes').decode(),
                'APPLE_CERTIFICATES_PASSWORD': 'fixture certificate password',
                'APPLE_API_PRIVATE_KEY': 'fixture notarization key',
                'APPLE_API_KEY': 'fixture key id', 'APPLE_API_ISSUER': 'fixture issuer'}

    def invoke(self, script, env):
        return subprocess.run(['bash', str(ROOT / 'scripts' / script)], env=env,
                              capture_output=True, text=True, timeout=30)

    def assert_secrets_removed(self, root, result):
        for name in ('fairway-identities.p12', 'fairway-notary.p8'):
            self.assertFalse((root / name).exists())
        for secret in ('fixture certificate password', 'fixture notarization key', 'fixture p12 bytes'):
            self.assertNotIn(secret, result.stdout + result.stderr)

    def test_success_removes_plaintext_then_explicit_cleanup_removes_identity(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); env = self.fixture(root)
            result = self.invoke('import_apple_credentials.sh', env)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assert_secrets_removed(root, result)
            self.assertTrue((root / 'fairway-signing.keychain-db').exists())
            result = self.invoke('cleanup_apple_credentials.sh', env)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse((root / 'fairway-signing.keychain-db').exists())
            self.assertIn('list-keychains restore', (root / 'events').read_text())
            self.assertEqual(self.invoke('cleanup_apple_credentials.sh', env).returncode, 0)

    def test_failed_import_or_notarization_cleans_partial_credentials(self):
        for failure, status in [('FAIL_IMPORT', 23), ('FAIL_NOTARY', 31), ('FAIL_RESTORE', 1)]:
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as folder:
                root = Path(folder); env = self.fixture(root); env[failure] = '1'
                result = self.invoke('import_apple_credentials.sh', env)
                self.assertEqual(result.returncode, status, result.stderr)
                self.assert_secrets_removed(root, result)
                self.assertFalse((root / 'fairway-signing.keychain-db').exists())
                self.assertIn('delete-keychain', (root / 'events').read_text())

    def test_failed_search_list_restore_still_deletes_keys_and_reports_failure(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); env = self.fixture(root)
            self.assertEqual(self.invoke('import_apple_credentials.sh', env).returncode, 0)
            env['FAIL_RESTORE'] = '1'
            result = self.invoke('cleanup_apple_credentials.sh', env)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / 'fairway-signing.keychain-db').exists())
            self.assert_secrets_removed(root, result)


@unittest.skipIf(sys.platform == 'win32', 'macOS packaging shell fixture runs on Unix')
class DeferredMacPackagingTests(unittest.TestCase):
    def test_signing_only_mode_never_executes_bundled_code_or_the_updater_signer(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            scripts = root / 'scripts'; scripts.mkdir()
            shutil.copy2(ROOT / 'scripts/package_macos.sh', scripts)
            app = root / 'src-tauri/target/release/bundle/macos/Fairway XMP.app'
            (app / 'Contents/Resources').mkdir(parents=True)
            (app / 'Contents/MacOS').mkdir()
            (app / 'Contents/Resources/Uninstall Fairway XMP.command').write_text('exit 91')
            executable = app / 'Contents/MacOS/fairway'
            executable.write_text('#!/bin/sh\nexit 91\n'); executable.chmod(0o755)
            (root / 'package.json').write_text('{"version":"1.2.3"}')
            for name in ('verify_macos_permissions.py', 'check_signed_php.py'):
                (scripts / name).write_text('raise RuntimeError("Bundled code check ran while credentials were available")')
            for name in ('sign_macos.py', 'staple_macos.py'):
                (scripts / name).write_text('pass')
            binary = root / 'bin'; binary.mkdir()
            tool = binary / 'tool'
            tool.write_text('#!' + sys.executable + '\n' + 
                'from pathlib import Path\nimport shutil, sys\n'
                'name = Path(sys.argv[0]).name\n'
                'if name == "pkgbuild": Path(sys.argv[-1]).write_bytes(b"fixture package")\n'
                'if name == "productsign": shutil.copyfile(sys.argv[-2], sys.argv[-1])\n')
            tool.chmod(0o755)
            for name in ('ditto', 'pkgbuild', 'plutil', 'productsign', 'pkgutil', 'xcrun', 'codesign', 'spctl'):
                (binary / name).symlink_to(tool)
            env = {**os.environ, 'PATH': str(binary) + os.pathsep + os.environ['PATH'],
                   'FAIRWAY_SIGNED_RELEASE': '1', 'FAIRWAY_DEFER_RUNTIME_CHECKS': '1',
                   'FAIRWAY_DEFER_UPDATER_SIGNATURE': '1', 'APPLE_SIGNING_IDENTITY': 'fixture',
                   'APPLE_INSTALLER_IDENTITY': 'fixture', 'APPLE_NOTARY_PROFILE': 'fixture'}
            env.pop('TAURI_SIGNING_PRIVATE_KEY', None)
            result = subprocess.run(['sh', str(scripts / 'package_macos.sh')], env=env,
                                    capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue((root / 'artifacts/fairway-xmp-1.2.3-macos-arm64.pkg').is_file())
            archive = root / 'artifacts/fairway-xmp-1.2.3-macos-arm64.app.tar.gz'
            self.assertTrue(archive.is_file())
            self.assertFalse(Path(str(archive) + '.sig').exists())
            # Ordinary local packaging must still run its normal runtime checks.
            env['FAIRWAY_DEFER_RUNTIME_CHECKS'] = '0'
            result = subprocess.run(['sh', str(scripts / 'package_macos.sh')], env=env,
                                    capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Bundled code check ran', result.stderr)


class WorkflowSigningBoundaryTests(unittest.TestCase):
    def jobs(self, name):
        text = (ROOT / '.github/workflows' / name).read_text().split('\njobs:\n', 1)[1]
        return {match[1]: match[2] for match in re.finditer(
            r'^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)', text, re.M | re.S)}

    def test_signing_and_builds_use_different_jobs_without_build_hooks_or_rust_caches(self):
        for name, signing in [('macos-test-build.yml', ('package', 'sign-standalone')),
                              ('windows-test-build.yml', ('package',))]:
            jobs = self.jobs(name)
            self.assertRegex(jobs['build'], r'--config artifacts/tauri.release.json --no-sign')
            for job in ('build', 'standalone'):
                self.assertNotIn('${{ secrets.', jobs[job])
            for job in signing:
                body = jobs[job]
                with self.subTest(workflow=name, job=job):
                    self.assertIn('needs:', body)
                    self.assertIn('verify_signing_source.py --expected', body)
                    self.assertIn('persist-credentials: false', body)
                    self.assertNotRegex(body, r'cargo (?:build|run|test)|rust-toolchain|rust-cache|tauri -- build')
                    self.assertIn('npm ci --ignore-scripts', body)
                    self.assertNotIn('npm exec', body)

    def test_bundled_code_checks_run_after_keychain_cleanup(self):
        jobs = self.jobs('macos-test-build.yml')
        for job in ('package', 'sign-standalone'):
            body = jobs[job]
            start = body.index('run: bash scripts/import_apple_credentials.sh')
            cleanup = body.index('run: bash scripts/cleanup_apple_credentials.sh')
            self.assertLess(start, cleanup)
            credential_window = body[start:cleanup]
            self.assertNotRegex(credential_window, r'check_signed_php|verify_macos_permissions|--test-native-hosts-registration|npm ci|tauri.*build')
            self.assertIn('if: ', body[start:cleanup])  # Cleanup must run on failures too.
            for command in ('check_signed_php.py', '--test-native-hosts-registration'):
                self.assertGreater(body.index(command), cleanup)
        self.assertIn("FAIRWAY_DEFER_RUNTIME_CHECKS: '1'", jobs['package'])
        self.assertIn("FAIRWAY_DEFER_UPDATER_SIGNATURE: '1'", jobs['package'])
        self.assertNotIn('TAURI_SIGNING_PRIVATE_KEY', jobs['package'].split('Sign and notarize app and installer')[1].split('Remove temporary signing credentials')[0])
        source = jobs['standalone']
        self.assertLess(source.index('verify_signing_source.py --source'), source.index('ref: ${{ steps.source.outputs.commit }}'))

    def test_publication_builds_the_verifier_with_read_only_permissions(self):
        jobs = self.jobs('publish-release.yml')
        self.assertNotIn('contents: write', jobs['prepare-verifier'])
        self.assertIn('cargo build', jobs['prepare-verifier'])
        self.assertIn('contents: write', jobs['publish'])
        self.assertNotRegex(jobs['publish'], r'cargo |rust-toolchain|rust-cache')
        self.assertIn('FAIRWAY_UPDATE_VERIFIER:', jobs['publish'])

    def test_postgresql_is_signed_before_pinning_and_compilation_without_running_payloads_with_credentials(self):
        jobs = self.jobs('postgresql-runtime.yml')
        self.assertNotIn('${{ secrets.', jobs['build'])
        signer = jobs['sign']
        self.assertNotRegex(signer, r'cargo (?:build|run|test)|rust-cache|rust-toolchain')
        self.assertIn('verify_signing_source.py --expected', signer)
        window = signer[signer.index('run: bash scripts/import_apple_credentials.sh'):signer.index('run: bash scripts/cleanup_apple_credentials.sh')]
        self.assertIn('postgresql_release.py sign', window)
        self.assertNotRegex(window, r'/bin/(postgres|initdb|psql)|cargo |npm ')
        macos = self.jobs('macos-test-build.yml')
        self.assertLess(macos['build'].index('postgresql_release.py stage'), macos['build'].index('npm run tauri -- build'))
        self.assertGreater(macos['package'].index('artifacts/postgresql-tests postgresql'), macos['package'].index('run: bash scripts/cleanup_apple_credentials.sh'))


if __name__ == '__main__':
    unittest.main()
