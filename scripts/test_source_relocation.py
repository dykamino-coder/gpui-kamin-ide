import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from source_relocation import relocate_includes, source_locations, rust_literal


class IncludeRelocationTests(unittest.TestCase):
    def test_coverage_keeps_the_original_target(self):
        names = ['coverage/sources.rs', 'style/mod.rs', 'layout/mod.rs', 'lib.rs']
        locations = source_locations(names, {'coverage': 'kamin-html',
                                             'style': 'kamin-style', 'layout': 'kamin-layout'})
        source = 'const A: &str = include_str!("../style/mod.rs");'
        after, checked = relocate_includes(source, 'crates/html/src/coverage/sources.rs',
                                           locations, set(locations))
        self.assertEqual(after, 'const A: &str = include_str!("../../../html-style/src/style/mod.rs");')
        self.assertEqual(checked, [('crates/html/src/style/mod.rs', 'crates/html-style/src/style/mod.rs')])

    def test_include_inside_moved_domain_is_unchanged(self):
        locations = source_locations(['style/mod.rs', 'style/fixture.txt'], {'style': 'kamin-style'})
        source = 'let s = include_str!(r#"fixture.txt"#);'
        self.assertEqual(relocate_includes(source, 'crates/html/src/style/mod.rs',
                                           locations, set(locations))[0], source)

    def test_comments_and_ordinary_literals_are_preserved(self):
        source = '// include_str!("missing")\nlet s = "include_str!(missing)";'
        self.assertEqual(relocate_includes(source, 'crates/html/src/lib.rs', {}, set())[0], source)

    def test_missing_or_unsupported_target_fails_before_write(self):
        for operand in ['"missing.rs"', 'concat!("a", "b")', 'PATH']:
            with self.subTest(operand=operand), self.assertRaises(ValueError):
                relocate_includes('include_bytes!(' + operand + ')',
                                   'crates/html/src/lib.rs', {}, set())

    def test_manifest_sibling_asset_retains_identity(self):
        old = 'crates/html/src/resources/test.rs'
        locations = {old: 'crates/style/src/resources/test.rs'}
        source = 'include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../shell/font.ttf"))'
        after, checked = relocate_includes(source, old, locations, {'crates/shell/font.ttf'})
        self.assertEqual(after, source)
        self.assertEqual(checked, [('crates/shell/font.ttf', 'crates/shell/font.ttf')])

    def test_unknown_owners_are_rejected(self):
        with self.assertRaises(ValueError):
            source_locations(['new_domain/mod.rs'], {})

    def test_raw_literals_without_hashes_are_valid(self):
        self.assertEqual(rust_literal('r"fixture.txt"'), 'fixture.txt')
        with self.assertRaises(ValueError):
            rust_literal('r#"fixture.txt"##')


if __name__ == '__main__':
    unittest.main()
