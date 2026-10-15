import hashlib
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from package_sources import archive_git_tree, fetch_source, require_locked_plan


class SourceTreeTests(unittest.TestCase):
    def test_candidate_source_inputs_must_already_be_pinned(self):
        locked = [{'name': 'php', 'url': 'https://example.test/php.tar.gz', 'sha256': 'a' * 64}]
        require_locked_plan(locked, locked)
        for bad in [[], [{'name': 'extra', 'url': 'https://example.test/extra', 'sha256': 'a' * 64}],
                    [{'name': 'php', 'url': locked[0]['url'], 'sha256': None}],
                    [{'name': 'php', 'url': locked[0]['url'], 'sha256': 'b' * 64}]]:
            with self.assertRaises(RuntimeError): require_locked_plan(bad, locked)

    def archive(self, root, filename, content=b'original source', mode=0o644, mtime=0, names=None):
        path = root / filename
        with tarfile.open(path, 'w:gz') as bundle:
            for name in names or ['src/code.c']:
                member = tarfile.TarInfo(name)
                member.size, member.mode, member.mtime = len(content), mode, mtime
                bundle.addfile(member, io.BytesIO(content))
        return path

    def test_git_tree_ignores_archive_metadata_but_detects_source_and_mode_changes(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            original = archive_git_tree(self.archive(root, 'original.tar.gz'))
            self.assertEqual(original, archive_git_tree(self.archive(root, 'repacked.tar.gz', mtime=123456)))
            self.assertNotEqual(original, archive_git_tree(self.archive(root, 'changed.tar.gz', content=b'changed source')))
            self.assertNotEqual(original, archive_git_tree(self.archive(root, 'executable.tar.gz', mode=0o755)))

    def test_locked_gitiles_recompression_requires_the_original_source_tree(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            original = self.archive(root, 'original.tar.gz')
            revision = 'b' * 40
            pin = {'name': 'aom', 'url': f'https://aomedia.googlesource.com/aom/+archive/{revision}.tar.gz',
                   'revision': revision, 'gitTree': archive_git_tree(original),
                   'sha256': hashlib.sha256(original.read_bytes()).hexdigest(), 'file': 'aom.tar.gz'}
            sources = root / 'sources'; sources.mkdir()
            repacked = self.archive(root, 'repacked.tar.gz', mtime=123456)
            digest = hashlib.sha256(repacked.read_bytes()).hexdigest()
            self.assertNotEqual(digest, pin['sha256'])
            with patch('package_sources.download', return_value=(repacked, digest)), \
                    patch('package_sources.request', side_effect=AssertionError('Use the committed tree, not live metadata')):
                verified = fetch_source(dict(pin), sources)
            require_locked_plan([verified], [pin])
            self.assertEqual((sources / pin['file']).read_bytes(), repacked.read_bytes())
            for kwargs in [{'content': b'tampered source'}, {'mode': 0o755}]:
                changed = self.archive(root, 'changed.tar.gz', **kwargs)
                (sources / pin['file']).unlink(missing_ok=True)
                with patch('package_sources.download', return_value=(changed, hashlib.sha256(changed.read_bytes()).hexdigest())):
                    with self.assertRaisesRegex(RuntimeError, 'committed Git tree'):
                        fetch_source(dict(pin), sources)
                self.assertFalse((sources / pin['file']).exists())
            for changed in [{**verified, 'revision': 'c' * 40}, {**verified, 'gitTree': 'd' * 40},
                            {**verified, 'url': pin['url'].replace('aomedia.googlesource.com', 'example.test')}]:
                with self.assertRaises(RuntimeError): require_locked_plan([changed], [pin])
            for changed_pin in [{**pin, 'gitTree': None},
                                {**pin, 'url': pin['url'].replace('aomedia.googlesource.com', 'example.test')}]:
                with self.assertRaises(RuntimeError):
                    require_locked_plan([{**changed_pin, 'sha256': digest}], [changed_pin])

    def test_source_tree_refuses_duplicate_and_escaping_paths(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for names in [['../outside'], ['/outside'], ['source.c', 'source.c']]:
                with self.subTest(names=names):
                    with self.assertRaises(RuntimeError):
                        archive_git_tree(self.archive(root, 'unsafe.tar.gz', names=names))

    def test_source_tree_rejects_unpinned_directories_and_special_permissions(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name in ('../outside', '/outside', '..\\outside', 'C:/outside', 'extra-empty-directory'):
                archive = root / 'directory.tar.gz'
                with tarfile.open(archive, 'w:gz') as bundle:
                    member = tarfile.TarInfo(name); member.type = tarfile.DIRTYPE; member.mode = 0o755
                    bundle.addfile(member)
                with self.assertRaises(RuntimeError): archive_git_tree(archive)
            for mode in (0o4755, 0o666):
                with self.assertRaisesRegex(RuntimeError, 'Unsafe source archive file mode'):
                    archive_git_tree(self.archive(root, 'unsafe-mode.tar.gz', mode=mode))


if __name__ == '__main__': unittest.main()
