from pathlib import Path
import json
import plistlib
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
import sign_macos
import check_signed_php


class SigningTests(unittest.TestCase):
    def test_xpc_helper_has_its_exact_peer_identifier(self):
        app = Path('/build/Fairway XMP.app')
        helper_id = 'dev.fairway.environment.hosts-helper'
        helper = app / 'Contents/Library/HelperTools' / helper_id
        command = sign_macos.signing_command(helper, 'Developer ID', app)
        self.assertEqual(command[command.index('--identifier') + 1], helper_id)
        self.assertNotIn('--entitlements', command)
        self.assertNotIn('--identifier', sign_macos.signing_command(app, 'Developer ID', app))

    def test_only_php_interpreters_receive_the_jit_entitlement(self):
        app = Path('/build/Fairway XMP.app')
        runtime = app / 'Contents/Resources/runtime'
        for version in ('php', 'php-8.2', 'php-8.5', 'php-8.6'):
            for binary in ('php', 'php-cgi', 'php-fpm'):
                command = sign_macos.signing_command(runtime / version / 'bin' / binary, 'Developer ID', app)
                self.assertIn('--entitlements', command)
        for path in (app, app / 'Contents/MacOS/fairway', runtime / 'mysql/bin/mysqld', runtime / 'nginx/bin/nginx', runtime / 'lib/libpcre2.dylib', runtime / 'php-8.3/ext/opcache.so'):
            self.assertNotIn('--entitlements', sign_macos.signing_command(path, 'Developer ID', app))
        entitlements = plistlib.loads(sign_macos.ENTITLEMENTS.read_bytes())
        self.assertEqual(entitlements, {'com.apple.security.cs.allow-jit': True})

    def test_main_executable_is_signed_only_by_the_final_bundle_step(self):
        with tempfile.TemporaryDirectory() as temporary:
            app = Path(temporary) / 'Fairway XMP.app'
            main = app / 'Contents/MacOS/fairway'
            library = app / 'Contents/Resources/runtime/lib/libexample.dylib'
            for file in (main, library):
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(bytes.fromhex('cffaedfe'))
            (app / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable': 'fairway'}))
            self.assertEqual(sign_macos.signing_targets(app), [library])


class SignedPhpTests(unittest.TestCase):
    def runtime(self, app, include_86=True, default='8.5'):
        versions = {'8.2': '8.2.36', '8.3': '8.3.37', '8.4': '8.4.28', '8.5': '8.5.13'}
        if include_86:
            versions['8.6'] = '8.6.0'
        runtime = app / 'Contents/Resources/runtime'
        for minor in versions:
            php = runtime / ('php' if minor == default else f'php-{minor}') / 'bin/php'
            php.parent.mkdir(parents=True)
            php.touch()
        (runtime / 'manifest.json').write_text(json.dumps({'php': versions[default], 'phpVersions': versions}))
        return runtime, versions

    def test_verifier_exercises_every_manifest_branch_with_four_or_five_versions(self):
        for include_86, default in [(False, '8.5'), (True, '8.5'), (True, '8.6')]:
            with self.subTest(include_86=include_86, default=default), tempfile.TemporaryDirectory() as folder:
                app = Path(folder) / 'Fairway XMP.app'
                runtime, versions = self.runtime(app, include_86, default)
                with patch.object(sys, 'argv', ['check_signed_php.py', str(app)]), \
                        patch.object(check_signed_php.subprocess, 'run') as run:
                    check_signed_php.main()
                self.assertEqual(run.call_count, 4 * len(versions))
                checked = {Path(call.args[0][0]): call.args[0][-1]
                           for call in run.call_args_list if '-r' in call.args[0] and 'PHP_VERSION !==' in call.args[0][3]}
                self.assertEqual(checked, dict(check_signed_php.bundled_interpreters(runtime)))
                self.assertEqual(set(checked.values()), set(versions.values()))
                self.assertTrue(all(call.kwargs['check'] for call in run.call_args_list))

    def test_verifier_rejects_missing_and_unexpected_branches_before_execution(self):
        with tempfile.TemporaryDirectory() as folder:
            app = Path(folder) / 'Fairway XMP.app'
            runtime, _ = self.runtime(app)
            (runtime / 'php-8.6/bin/php').unlink()
            for extra in (False, True):
                if extra:
                    unexpected = runtime / 'php-9.0/bin/php'
                    unexpected.parent.mkdir(parents=True)
                    unexpected.touch()
                with patch.object(sys, 'argv', ['check_signed_php.py', str(app)]), \
                        patch.object(check_signed_php.subprocess, 'run') as run:
                    with self.assertRaisesRegex(RuntimeError, 'missing:.*php-8.6'):
                        check_signed_php.main()
                    run.assert_not_called()
