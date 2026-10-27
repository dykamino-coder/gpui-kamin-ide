"""Check that moving physical crates cannot drop guarded source files."""

import unittest
from unittest.mock import patch
import io
import tarfile

from html_source_inventory import PREFIX, inventory, read


class Inventory(unittest.TestCase):
    def test_phase_names_survive_moves(self):
        before = inventory({'crates/html/src/layout/flow.rs': 'old'})
        after = inventory({'crates/html-layout/src/layout/flow.rs': 'new'})
        self.assertEqual(list(before), list(after))
        self.assertEqual(after[PREFIX + 'layout/flow.rs'].package, 'kamin-layout')

    def test_root_code_is_not_dropped_or_overwritten(self):
        sources = inventory({
            'crates/html/src/lib.rs': 'facade',
            'crates/html-style/src/lib.rs': 'style-root',
            'crates/html-layout/src/lib.rs': 'layout-root',
        })
        self.assertEqual(len(sources), 3)
        self.assertEqual({item.text for item in sources.values()},
                         {'facade', 'style-root', 'layout-root'})

    def test_duplicate_phase_file_fails(self):
        with self.assertRaisesRegex(ValueError, 'duplicate logical'):
            inventory({'crates/html/src/style/x.rs': 'x',
                       'crates/html-style/src/style/x.rs': 'x'})

    def test_unknown_sibling_is_not_engine_source(self):
        self.assertFalse(inventory({'crates/shell/src/style/x.rs': 'x'}))

    def test_parent_escape_fails(self):
        with self.assertRaisesRegex(ValueError, 'unsafe'):
            inventory({'crates/html-layout/src/../outside.rs': 'x'})

    def test_large_revision_inventory_uses_bounded_archive_arguments(self):
        names = [f'crates/html/src/layout/long_domain_name/case_{n}.rs'
                 for n in range(2000)]
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode='w') as archive:
            for name in (names[0], 'crates/html/src/layout/fixture.txt'):
                item = tarfile.TarInfo(name)
                item.size = 1
                archive.addfile(item, io.BytesIO(b'x'))
        with patch('html_source_inventory.subprocess.check_output',
                   side_effect=['\n'.join(names), output.getvalue()]) as command:
            self.assertEqual(list(read('.', 'HEAD')), [names[0]])
        arguments = command.call_args_list[1].args[0]
        self.assertLess(sum(map(len, arguments)), 1000)
        self.assertEqual(arguments[-1], 'crates/html/src/')


if __name__ == '__main__':
    unittest.main()
