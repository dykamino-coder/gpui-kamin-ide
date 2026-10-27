"""Boundary migration invariants; no engine dependencies or target writes."""
import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from rewrite_paths import rewrite

OWNERS = {'style': 'kamin-style', 'layout': 'kamin-layout', 'paint': 'kamin-paint'}


class RewriteTests(unittest.TestCase):
    def change(self, source, filename='layout/block.rs'):
        return rewrite(source, filename, 'kamin-layout', OWNERS)

    def test_strings_and_comments_are_not_paths(self):
        text = 'let a = r###"crate::style::A"###; /* nested /* crate::paint::B */ x */\n'
        text += '// crate::style::C\nlet x: crate::style::A;'
        self.assertEqual(self.change(text), text.replace('let x: crate::style::A',
                                                         'let x: kamin_style::style::A'))

    def test_nested_mixed_use_keeps_single_cfg_item(self):
        source = '#[cfg(test)]\nuse crate::{style::{A, B as Bee}, layout::{self, C}};'
        self.assertEqual(self.change(source), '#[cfg(test)]\nuse {kamin_style::style::A, '
                         'kamin_style::style::B as Bee, crate::layout, crate::layout::C};')

    def test_inline_modules_and_super_paths(self):
        source = 'mod inner { use super::Thing; let x: super::super::style::A; }'
        actual = self.change(source, 'layout/mod.rs')
        self.assertEqual(actual, 'mod inner { use crate::layout::Thing; '
                         'let x: kamin_style::style::A; }')

    def test_external_imports_preserved(self):
        source = 'use gpui::{AnyElement, self}; use std::rc::Rc;'
        self.assertEqual(self.change(source), source)

    def test_unassigned_path_rejected(self):
        with self.assertRaises(ValueError):
            self.change('use crate::document::Document;')

    def test_macro_crate_requires_explicit_decision(self):
        with self.assertRaises(ValueError):
            self.change('macro_rules! a { () => { $crate::style::A } }')

    def test_attributes_characters_and_lifetimes_preserved(self):
        source = "#[doc = \"crate::style::A\"] fn f<'a>(x: &'a str) { let c = 'c'; }"
        self.assertEqual(self.change(source), source)

    def test_alias_resolver_selects_actual_owner(self):
        result = rewrite('use crate::paint::Alias;', 'layout/block.rs',
                         'kamin-layout', OWNERS,
                         resolve=lambda path: ('style', 'Actual'))
        self.assertEqual(result, 'use kamin_style::style::Actual;')

    def test_raw_identifiers_preserved(self):
        self.assertEqual(self.change('use crate::style::r#type as r#async;'),
                         'use kamin_style::style::r#type as r#async;')

    def test_comments_in_import_and_qualified_path_survive(self):
        source = 'use crate::{style::/* a /* b */ c */A, layout::B};\n'
        source += 'let x: crate /* note */ ::style::A;'
        actual = self.change(source)
        for comment in ('/* a /* b */ c */', '/* note */'):
            self.assertEqual(actual.count(comment), 1)
        self.assertIn('kamin_style::style::A', actual)

    def test_second_pass_is_identical(self):
        source = '#[cfg(test)] use crate::{style::{A, B as Bee}, layout::{self, C}};'
        once = self.change(source)
        self.assertEqual(self.change(once), once)

    def test_precise_capture_is_not_an_import(self):
        source = 'fn f() -> impl Fn(crate::style::A) + use<> { todo!() }'
        self.assertEqual(self.change(source),
                         'fn f() -> impl Fn(kamin_style::style::A) + use<> { todo!() }')

    def test_mixed_absolute_extern_root_is_explicit_failure(self):
        with self.assertRaises(ValueError):
            self.change('use {::std::rc::Rc, crate::style::A};')


if __name__ == '__main__':
    unittest.main()
