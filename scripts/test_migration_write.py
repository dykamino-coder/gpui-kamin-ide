import sys
from pathlib import Path
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from migration_plan import Change
from migration_write import apply, preflight

OUT = Path(__file__).resolve().parent


class MigrationWriteTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory(prefix='writer-fixture-')
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name)
        (self.root / 'a.rs').write_bytes(b'original\n')
        self.changes = [Change('a.rs', 'lower/a.rs', 'original\n', 'rewritten\n'),
                        Change('lower/lib.rs', 'lower/lib.rs', '', 'pub mod a;\n')]

    def test_move_exact_bytes_and_generated_roots(self):
        apply(self.root, self.changes)
        self.assertFalse((self.root / 'a.rs').exists())
        self.assertEqual((self.root / 'lower/a.rs').read_bytes(), b'rewritten\n')
        self.assertEqual((self.root / 'lower/lib.rs').read_bytes(), b'pub mod a;\n')

    def test_stale_input_aborts_before_any_writes(self):
        (self.root / 'a.rs').write_bytes(b'changed concurrently')
        with self.assertRaisesRegex(ValueError, 'Source changed'):
            apply(self.root, self.changes)
        self.assertFalse((self.root / 'lower').exists())

    def test_collision_is_rejected_without_overwriting_other_work(self):
        (self.root / 'lower').mkdir()
        (self.root / 'lower/a.rs').write_bytes(b'unrelated')
        with self.assertRaisesRegex(ValueError, 'Destination already exists'):
            apply(self.root, self.changes)
        self.assertEqual((self.root / 'lower/a.rs').read_bytes(), b'unrelated')

    def test_generated_file_directory_collision_rejects_before_writes(self):
        (self.root / 'lower/lib.rs').mkdir(parents=True)
        with self.assertRaisesRegex(ValueError, 'not a regular file'):
            apply(self.root, self.changes)
        self.assertEqual((self.root / 'a.rs').read_bytes(), b'original\n')
        self.assertFalse((self.root / 'lower/a.rs').exists())
        self.assertTrue((self.root / 'lower/lib.rs').is_dir())

    def test_parent_file_collision_rejects_the_entire_plan_before_writes(self):
        (self.root / 'lower').write_bytes(b'unrelated parent file')
        changes = [Change('a.rs', 'upper/a.rs', 'original\n', 'rewritten\n'),
                   self.changes[1]]
        with self.assertRaisesRegex(ValueError, 'parent is not a directory'):
            apply(self.root, changes)
        self.assertEqual((self.root / 'a.rs').read_bytes(), b'original\n')
        self.assertEqual((self.root / 'lower').read_bytes(), b'unrelated parent file')
        self.assertFalse((self.root / 'upper').exists())

    def test_partial_write_failure_restores_exact_inputs(self):
        def broken_writer(path, data):
            path.write_bytes(data[:2])
            if path.name == 'lib.rs':
                raise OSError('synthetic write failure')
        with self.assertRaisesRegex(OSError, 'synthetic'):
            apply(self.root, self.changes, write=broken_writer)
        self.assertEqual((self.root / 'a.rs').read_bytes(), b'original\n')
        self.assertFalse((self.root / 'lower/a.rs').exists())
        self.assertFalse((self.root / 'lower/lib.rs').exists())

    def test_in_place_failure_restores_existing_manifest(self):
        changes = [Change('a.rs', 'a.rs', 'original\n', 'new\n')]
        def broken_writer(path, data):
            path.write_bytes(b'partial')
            raise OSError('synthetic')
        with self.assertRaises(OSError):
            apply(self.root, changes, write=broken_writer)
        self.assertEqual((self.root / 'a.rs').read_bytes(), b'original\n')

    def test_path_escape_and_duplicate_rewrites_reject(self):
        with self.assertRaisesRegex(ValueError, 'escapes'):
            preflight(self.root, [Change('../outside', 'a.rs', '', '')])
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            preflight(self.root, self.changes + self.changes[:1])


if __name__ == '__main__':
    unittest.main()
