import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from module_sources import modules


class ModuleSourceTests(unittest.TestCase):
    def test_declared_identity_wins_over_physical_filename(self):
        _, files = modules({'lib.rs': 'mod fonts;', 'fonts.rs':
                           '#[path = "math_constants.rs"] mod constants;',
                           'math_constants.rs': 'pub struct Constants;'})
        self.assertEqual(files['math_constants.rs'], ('fonts', 'constants'))

    def test_inline_scope_and_external_file(self):
        decl, files = modules({'lib.rs': 'mod a { mod b; }', 'a/b.rs': 'mod c;',
                              'a/b/c.rs': ''})
        self.assertTrue(decl[('a',)].inline)
        self.assertEqual(files['a/b/c.rs'], ('a', 'b', 'c'))

    def test_all_cfgs_comments_and_strings(self):
        _, files = modules({'lib.rs': '#[cfg(test)] mod tests; // mod fake;\n'
                           'const S: &str = "mod fake;"; fn f(){ let a = "mod fake;"; }',
                           'tests.rs': ''})
        self.assertEqual(set(files), {'lib.rs', 'tests.rs'})

    def test_fail_closed_on_ambiguous_duplicate_and_missing(self):
        for sources in [{'lib.rs': 'mod a;', 'a.rs': '', 'a/mod.rs': ''},
                        {'lib.rs': 'mod a;'},
                        {'lib.rs': 'mod a; mod a;', 'a.rs': ''},
                        {'lib.rs': '#[path = "../a.rs"] mod a;', '../a.rs': ''}]:
            with self.subTest(sources=sources), self.assertRaises(ValueError):
                modules(sources)

    def test_repository_resolves_every_source(self):
        from html_source_inventory import PACKAGES
        root = Path(__file__).resolve().parents[1]
        for prefix in PACKAGES.values():
            folder = root / prefix
            if not folder.exists():
                continue
            sources = {p.relative_to(folder).as_posix(): p.read_text(encoding='utf-8')
                       for p in folder.rglob('*.rs')}
            _, files = modules(sources)
            self.assertEqual(set(files), set(sources))

    def test_rewrite_uses_declared_path_for_relative_imports(self):
        from rewrite_paths import rewrite
        owners = {'style': 'kamin-style', 'text': 'kamin-layout'}
        source = 'use super::super::super::super::style::Computed; use self::Value;'
        changed = rewrite(source, 'text/fonts/math_constants.rs',
                          'kamin-layout', owners,
                          module_path=('text', 'fonts', 'math', 'constants'))
        self.assertEqual(changed, 'use kamin_style::style::Computed; '
                         'use crate::text::fonts::math::constants::Value;')

    def test_unknown_path_expression_is_rejected(self):
        with self.assertRaises(ValueError):
            modules({'lib.rs': '#[path = concat!("a", ".rs")] mod a;', 'a.rs': ''})

    def test_export_resolution_uses_declared_module_identity(self):
        from canonical_exports import CanonicalExports
        sources = {'lib.rs': 'mod text; pub use text::fonts::Fact;',
                   'text.rs': '#[path = "font_facts.rs"] mod fonts;',
                   'font_facts.rs': 'pub use crate::style::Value as Fact;',
                   'style.rs': 'pub struct Value;'}
        sources['lib.rs'] += ' mod style;'
        _, file_modules = modules(sources)
        exports = CanonicalExports(sources, file_modules)
        self.assertEqual(exports.resolve(('Fact',)), {('style', 'Value')})


if __name__ == '__main__':
    unittest.main()
