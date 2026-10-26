"""Regression checks for the guard analysis view after physical crate moves."""
from pathlib import Path
import sys
import unittest

sys.path[:0] = [str(Path(__file__).resolve().parent),
               str(Path(__file__).resolve().parent)]
from html_guard_paths import logical_path, normalize
from rust_layer_paths import dependencies


class GuardPaths(unittest.TestCase):
    def test_external_and_grouped_paths_keep_dependency_inventory(self):
        old = 'use crate::{layout::A, paint::{B, C}}; crate::render::go();'
        new = 'use kamin_layout::{layout::A}; use kamin_paint::paint::{B, C}; kamin_html::render::go();'
        scan = lambda text: sorted(path for path, _ in dependencies(text, 'style/a.rs', {}))
        self.assertEqual(scan(normalize(new)), scan(old))

    def test_comments_literals_and_line_numbers_are_preserved(self):
        source = ('// kamin_paint::paint::A\n'
                  '/* outer /* kamin_paint::paint */ end */\n'
                  'let s = r###"kamin_paint::paint"###;\n'
                  'let c = \'x\'; kamin_paint /* comment */ ::paint::go();\n')
        expected = source.replace("kamin_paint /* comment */", "crate /* comment */")
        self.assertEqual(normalize(source), expected)
        self.assertEqual(normalize(expected), expected)

    def test_global_and_raw_namespaces(self):
        self.assertEqual(normalize('use ::kamin_layout::layout::A; r#kamin_paint::paint::go();'),
                         'use crate::layout::A; crate::paint::go();')
        self.assertEqual(normalize('use ::kamin_font_facts::font_facts::Provider;'),
                         'use crate::font_facts::Provider;')

    def test_member_namespace_is_not_a_crate_root(self):
        source = 'other::kamin_layout::A; foo.kamin_paint; kamin_html_other::render::A;'
        self.assertEqual(normalize(source), source)

    def test_whole_crate_alias_cannot_hide_an_engine_dependency(self):
        for source in ('use kamin_paint as p;', 'use {kamin_layout as l, std::fmt};',
                       'use ::kamin_style;', 'extern crate kamin_paint as p;',
                       'use kamin_font_facts as facts;'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                normalize(source)
        source = 'use other::{kamin_paint as p}; let kamin_paint = 1;'
        self.assertEqual(normalize(source), source)

    def test_every_physical_root_has_a_stable_guard_identity(self):
        self.assertEqual(logical_path('crates/html-paint/src/paint/a.rs'),
                         'crates/html/src/paint/a.rs')
        self.assertEqual(logical_path('crates/html-layout/src/lib.rs'),
                         'crates/html/src/__crate_roots__/kamin_layout.rs')
        self.assertEqual(logical_path('vendor/a.rs'), 'vendor/a.rs')


if __name__ == '__main__':
    unittest.main()
