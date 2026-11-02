"""Regression checks for CLI smoke capture and bounded process cleanup."""
import os
import ctypes
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

from smoke_process import SmokeCommands


class SmokeProcessTests(unittest.TestCase):
    def test_utf8_output_and_nonzero_exit_are_preserved(self):
        with tempfile.TemporaryDirectory() as folder, SmokeCommands(folder) as commands:
            result = commands.run([sys.executable, '-c',
                "import os,sys; os.write(1, '日本'.encode()); os.write(2, b'problem'); sys.exit(7)"])
            self.assertEqual((result.returncode, result.stdout, result.stderr), (7, '日本', 'problem'))
            self.assertTrue((Path(folder) / 'command-001.json').is_file())

    def test_parent_exit_does_not_wait_for_descendant_output_handle(self):
        child = None
        child_handle = None
        try:
            with tempfile.TemporaryDirectory() as folder, SmokeCommands(folder) as commands:
                result = commands.run([sys.executable, '-c',
                    "import subprocess,sys,time; time.sleep(.2); "
                    "p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); print(p.pid)"], timeout=5)
                child = int(result.stdout.strip())
                self.assertEqual(result.returncode, 0)
                if os.name == 'nt':
                    from ctypes import wintypes as w
                    api = commands.job.api
                    api.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
                    api.OpenProcess.restype = w.HANDLE
                    api.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
                    api.WaitForSingleObject.restype = w.DWORD
                    child_handle = api.OpenProcess(0x00100000, False, child)  # SYNCHRONIZE
                    self.assertTrue(child_handle)
                    self.assertEqual(api.WaitForSingleObject(child_handle, 0), 258)  # still running
                # It remains available between commands in the same smoke session.
                commands.run([sys.executable, '-c', 'pass'], timeout=5)
        finally:
            # WindowsJob cleans its test descendants on context exit.
            if child_handle:
                try:
                    self.assertEqual(api.WaitForSingleObject(child_handle, 5000), 0)
                finally:
                    api.CloseHandle(child_handle)
            if child and os.name != 'nt':
                try:
                    os.kill(child, signal.SIGKILL)
                except ProcessLookupError:
                    pass

    def test_timeout_is_bounded_with_descendant_output_handle_open(self):
        with tempfile.TemporaryDirectory() as folder, SmokeCommands(folder) as commands:
            started = time.monotonic()
            with self.assertRaises(subprocess.TimeoutExpired):
                commands.run([sys.executable, '-c',
                    "import subprocess,sys,time; "
                    "subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); time.sleep(60)"], timeout=1)
            self.assertLess(time.monotonic() - started, 8)
            self.assertTrue((Path(folder) / 'command-001.stderr').is_file())


if __name__ == '__main__':
    unittest.main()
