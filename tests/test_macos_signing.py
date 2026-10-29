from pathlib import Path
import plistlib
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
import sign_macos


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
        for version in ('php', 'php-8.2', 'php-8.5'):
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
