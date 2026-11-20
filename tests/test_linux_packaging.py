import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
SPEC = importlib.util.spec_from_file_location('linux_runtime', SCRIPTS / 'prepare_linux_runtime.py')
runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runtime)


class LinuxCompatibilityTests(unittest.TestCase):
    def test_reused_nginx_must_match_the_requested_version(self):
        prefix = Path('/isolated/runtime/nginx')
        for actual, expected in [('1.30.6', False), ('1.30.7', True), ('1.30.70', False)]:
            with self.subTest(actual=actual), patch.object(runtime.subprocess, 'check_output',
                    return_value=f'nginx version: nginx/{actual}\n'):
                self.assertEqual(runtime.reusable_nginx(prefix, '1.30.7'), expected)
        for error in [FileNotFoundError(), subprocess.TimeoutExpired('nginx', 15),
                      subprocess.CalledProcessError(1, 'nginx')]:
            with patch.object(runtime.subprocess, 'check_output', side_effect=error):
                self.assertFalse(runtime.reusable_nginx(prefix, '1.30.7'))

    def test_reused_php_must_have_current_version_and_all_required_extensions(self):
        with tempfile.TemporaryDirectory() as folder:
            prefix = Path(folder)
            (prefix / 'sbin').mkdir()
            (prefix / 'sbin/php-fpm').touch()
            for version, missing, expected in [(runtime.MANIFEST['php'], None, True),
                                                (runtime.MANIFEST['php'], 'pdo_pgsql', False),
                                                (runtime.MANIFEST['php'], 'pgsql', False),
                                                (runtime.MANIFEST['php'], 'sockets', False),
                                                (runtime.MANIFEST['php'], 'soap', False),
                                                (runtime.MANIFEST['php'], 'ldap', False),
                                                (runtime.MANIFEST['php'], 'pcntl', False),
                                                (runtime.MANIFEST['php'], 'posix', False),
                                                ('0.0.0', None, False)]:
                output = json.dumps({'version': version, 'extensions': [name for name in runtime.required_php_extensions(runtime.MANIFEST, 'Linux') if name != missing]})
                with self.subTest(version=version, missing=missing), \
                        patch.object(runtime.subprocess, 'check_output', return_value=output):
                    self.assertEqual(runtime.reusable_php(prefix, runtime.MANIFEST['php']), expected)
            with patch.object(runtime.subprocess, 'check_output', side_effect=OSError):
                self.assertFalse(runtime.reusable_php(prefix, runtime.MANIFEST['php']))

    def test_newer_glibc_requirement_is_rejected(self):
        runtime.validate_glibc('Name: GLIBC_2.28 Name: GLIBC_2.35')
        with self.assertRaises(RuntimeError):
            runtime.validate_glibc('Name: GLIBC_2.28 Name: GLIBC_2.36')
        with self.assertRaises(RuntimeError):
            runtime.validate_glibc('Name: GLIBC_2.9 Name: GLIBC_2.40')


@unittest.skipIf(os.name == 'nt', 'Linux packaging scripts require a Unix shell')
class LinuxInstallationSafetyTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='fairway-install-safety-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.home = self.root / "student's home မြန်မာ"
        self.home.mkdir()
        self.commands = self.root / 'commands'
        self.commands.mkdir()
        self.calls = self.root / 'calls'
        self.env = {**os.environ, 'HOME': str(self.home),
                    'XDG_DATA_HOME': str(self.home / '.local/share'),
                    'PATH': str(self.commands) + os.pathsep + os.environ['PATH'],
                    'FAIRWAY_TEST_CALLS': str(self.calls)}
        self.env.pop('FAIRWAY_TEST_FAIL', None)
        for name, output in [('uname', 'Linux'), ('id', '1000'), ('getconf', 'glibc 2.35')]:
            script = '#!/bin/sh\n'
            if name == 'uname':
                script += '[ "$1" != -m ] || { echo x86_64; exit; }\n'
            self.executable(self.commands / name, script + f"echo '{output}'\n")
        # Exercise the same scripts on macOS too. Only GNU command syntax is
        # adapted here; Linux runs the real coreutils path and rename operations.
        if sys.platform != 'linux':
            self.executable(self.commands / 'realpath', f'#!{sys.executable}\n'
                            'from pathlib import Path\nimport sys\nprint(Path(sys.argv[-1]).resolve())\n')
            self.executable(self.commands / 'mv', f'#!{sys.executable}\n'
                            'import os, subprocess, sys\n'
                            'if sys.argv[1] == "-Tf": os.replace(sys.argv[2], sys.argv[3])\n'
                            'else: subprocess.run(["/bin/mv", *sys.argv[1:]], check=True)\n')
        self.source = self.root / 'unpacked'
        self.payload(self.source)

    def executable(self, path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        path.chmod(0o755)

    def payload(self, directory):
        directory.mkdir(parents=True, exist_ok=True)
        for name in ('install.sh', 'uninstall.sh'):
            shutil.copy2(SCRIPTS.parent / 'packaging/linux' / name, directory / name)
        (directory / 'BUILD').write_text('fixture-1\n')
        self.executable(directory / 'bin/fairway', '#!/bin/sh\n'
                        'printf "%s\\n" "$1" >> "$FAIRWAY_TEST_CALLS"\n'
                        '[ "${FAIRWAY_TEST_FAIL:-}" != "$1" ]\n')

    def installation(self, prefix):
        release = prefix / 'releases/fixture-1'
        self.payload(release)
        (prefix / '.fairway-installation').write_text('Fairway XMP user installation\n')
        (prefix / 'current').symlink_to('releases/fixture-1')
        return release

    def run_script(self, script, *args):
        return subprocess.run(['sh', str(script), *map(str, args)], env=self.env,
                              capture_output=True, text=True, timeout=15)

    def test_uninstall_preserves_user_files_inside_legacy_prefix(self):
        prefix = self.home / 'fairway'
        release = self.installation(prefix)
        self.env['XDG_DATA_HOME'] = str(prefix)
        files = [prefix / 'www/coursework.php',
                 prefix / 'dev.fairway.environment/mysql/student.ibd',
                 prefix / 'dev.fairway.environment/backups/student.sql',
                 prefix / 'notes.txt']
        for file in files:
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('student work')
        self.payload(prefix / 'releases/older-build')
        outside = self.root / 'outside-project.php'
        outside.write_text('outside coursework')
        (release / 'project-link').symlink_to(outside)

        result = self.run_script(prefix / 'current/uninstall.sh')

        self.assertEqual(result.returncode, 0, result.stderr)
        for file in files:
            self.assertEqual(file.read_text(), 'student work')
        self.assertEqual(outside.read_text(), 'outside coursework')
        self.assertFalse((prefix / 'releases').exists())
        self.assertFalse((prefix / 'current').is_symlink())
        self.assertTrue((prefix / '.fairway-installation').exists())
        self.assertIn('Remaining files have been kept', result.stdout)
        self.assertEqual(self.calls.read_text().splitlines(),
                         ['shutdown', 'clear-domains', 'clear-https-trust', 'remove-cli', 'shutdown'])

    def test_uninstall_removes_empty_dedicated_prefix(self):
        prefix = self.home / '.local/share/fairway-xmp'
        self.installation(prefix)
        result = self.run_script(prefix / 'current/uninstall.sh')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(prefix.exists())

    def test_failed_cleanup_preserves_installation(self):
        prefix = self.home / 'installation'
        release = self.installation(prefix)
        for command in ('shutdown', 'clear-domains', 'clear-https-trust', 'remove-cli'):
            with self.subTest(command=command):
                self.env['FAIRWAY_TEST_FAIL'] = command
                result = self.run_script(release / 'uninstall.sh')
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue((release / 'bin/fairway').exists())
                self.assertTrue((prefix / 'current').is_symlink())
                self.assertTrue((prefix / '.fairway-installation').exists())
                self.assertNotIn('Fairway XMP removed.', result.stdout)

    def test_extracted_installation_is_left_for_manual_removal(self):
        result = self.run_script(self.source / 'uninstall.sh')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.source / 'bin/fairway').exists())
        self.assertIn('Remove this extracted directory manually', result.stdout)

    def test_invalid_managed_layout_is_rejected_before_cleanup(self):
        prefix = self.home / 'installation'
        release = self.installation(prefix)
        (prefix / 'current').unlink()
        (prefix / 'current').mkdir()
        result = self.run_script(release / 'uninstall.sh')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('layout mismatch', result.stderr)
        self.assertFalse(self.calls.exists())
        self.assertTrue(release.exists())

    def test_uninstall_refuses_protected_paths_inside_payload(self):
        for protected in ('www', 'data'):
            with self.subTest(protected=protected):
                prefix = self.home / f'install-{protected}'
                release = self.installation(prefix)
                if protected == 'www':
                    (self.home / 'fairway').mkdir()
                    (self.home / 'fairway/www').symlink_to(release)
                else:
                    self.env['XDG_DATA_HOME'] = str(prefix / 'releases')
                result = self.run_script(release / 'uninstall.sh')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('overlap', result.stderr)
                self.assertFalse(self.calls.exists())
                self.assertTrue(release.exists())
                if protected == 'www':
                    (self.home / 'fairway/www').unlink()

    def test_installer_rejects_protected_path_overlaps_and_aliases(self):
        www = self.home / 'fairway/www'
        www.mkdir(parents=True)
        data = Path(self.env['XDG_DATA_HOME']) / 'dev.fairway.environment'
        data.mkdir(parents=True)
        alias = self.home / 'coursework-alias'
        alias.symlink_to(www)
        prefixes = [self.home / 'fairway', www, www / 'tools',
                    data.parent, data, data / 'tools',
                    alias / 'tools', self.home / 'missing/../fairway/www/tools',
                    self.home / 'missing/..', self.home / 'missing/../../..']
        for prefix in prefixes:
            with self.subTest(prefix=prefix):
                result = self.run_script(self.source / 'install.sh', '--prefix', prefix)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.calls.exists())
                self.assertFalse((www / 'tools').exists())
                self.assertFalse((data / 'tools').exists())
                self.assertFalse((self.home / 'missing').exists())

    def test_installer_checks_custom_xdg_data_home(self):
        self.env['XDG_DATA_HOME'] = str(self.home / 'custom data')
        prefix = Path(self.env['XDG_DATA_HOME']) / 'dev.fairway.environment/tools'
        result = self.run_script(self.source / 'install.sh', '--prefix', prefix)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('separate from your projects', result.stderr)
        self.assertFalse(prefix.parent.exists())

    def test_install_and_uninstall_refuse_node_storage_overlaps(self):
        stores = [self.home / '.local/share/fnm', self.home / '.fnm',
                  self.home / 'custom-node-store']
        self.env['FNM_DIR'] = str(stores[-1])
        for store in stores:
            with self.subTest(store=store):
                result = self.run_script(self.source / 'install.sh', '--prefix', store / 'tools')
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.calls.exists())
        prefix = self.home / 'installation'
        release = self.installation(prefix)
        self.env['FNM_DIR'] = str(release / 'student-node-versions')
        result = self.run_script(release / 'uninstall.sh')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('overlap', result.stderr)
        self.assertTrue(release.is_dir())
        self.assertFalse(self.calls.exists())

    def test_installer_rejects_symlinked_management_paths(self):
        for name in ('.fairway-installation', 'releases'):
            with self.subTest(name=name):
                prefix = self.home / f'install-{name}'
                prefix.mkdir()
                (prefix / '.fairway-installation').write_text('Fairway XMP user installation\n')
                target = self.root / f'outside-{name}'
                if name == 'releases':
                    target.mkdir()
                    (target / 'coursework.php').write_text('student work')
                else:
                    target.write_text('student work')
                    (prefix / name).unlink()
                (prefix / name).symlink_to(target)
                result = self.run_script(self.source / 'install.sh', '--prefix', prefix)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('layout mismatch', result.stderr)
                self.assertFalse(self.calls.exists())
                file = target / 'coursework.php' if target.is_dir() else target
                self.assertEqual(file.read_text(), 'student work')

    def test_safe_install_reinstall_and_uninstall(self):
        # Prefixes sharing a string prefix with a protected path remain valid.
        prefix = self.home / 'fairway/www-tools'
        for _ in range(2):
            result = self.run_script(self.source / 'install.sh', '--prefix', prefix)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((prefix / 'current').resolve(), prefix / 'releases/fixture-1')
        self.assertEqual(self.calls.read_text().splitlines(),
                         ['install-cli', '__prepare-update', 'install-cli'])
        result = self.run_script(prefix / 'current/uninstall.sh')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(prefix.exists())

    def test_reinstall_after_uninstall_preserves_files_in_prefix(self):
        prefix = self.home / 'installation'
        for _ in range(2):
            result = self.run_script(self.source / 'install.sh', '--prefix', prefix)
            self.assertEqual(result.returncode, 0, result.stderr)
            coursework = prefix / 'www/coursework.php'
            if not coursework.exists():
                coursework.parent.mkdir()
                coursework.write_text('student work')
            result = self.run_script(prefix / 'current/uninstall.sh')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(coursework.read_text(), 'student work')
            self.assertTrue((prefix / '.fairway-installation').is_file())
            self.assertFalse((prefix / 'releases').exists())


if __name__ == '__main__': unittest.main()
