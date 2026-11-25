"""Adversarial source inputs must be rejected before configure/make can execute."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/prepare_postgresql_runtime.py'
SPEC = importlib.util.spec_from_file_location('postgresql_payload', SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PostgreSQLSourceSafety(unittest.TestCase):
    def test_changed_download_rejected(self):
        with tempfile.TemporaryDirectory() as scratch:
            source = Path(scratch) / 'source'
            source.write_bytes(b'changed upstream input')
            with self.assertRaisesRegex(RuntimeError, 'checksum mismatch'):
                MODULE.verify_source(source, hashlib.sha256(b'reviewed source').hexdigest())

    def test_unsafe_archive_never_partially_extracts(self):
        for name, kind, link in [('../escape', tarfile.REGTYPE, ''), ('/absolute', tarfile.REGTYPE, ''),
                                 ('link', tarfile.SYMTYPE, '../escape'), ('hardlink', tarfile.LNKTYPE, 'safe'),
                                 ('device', tarfile.CHRTYPE, '')]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as scratch:
                root = Path(scratch)
                archive = root / 'archive.tar'
                with tarfile.open(archive, 'w') as bundle:
                    good = tarfile.TarInfo('safe')
                    good.size = 4
                    bundle.addfile(good, io.BytesIO(b'kept'))
                    bad = tarfile.TarInfo(name)
                    bad.type, bad.linkname = kind, link
                    bundle.addfile(bad)
                target = root / 'new'
                target.mkdir()
                with self.assertRaises(RuntimeError):
                    MODULE.extract_source(archive, target)
                self.assertEqual(list(target.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
