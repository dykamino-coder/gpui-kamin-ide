"""Regressions for stable aggregates, shrinking ceilings and bootstrap checks."""

from collections import Counter
from pathlib import Path
import tempfile
import unittest

from computed_access import DEFINITIONS, aggregate, growth
from check_computed import check, decode, documents
from test_check_transitional import commit, git
from computed_f2_rebaseline import APPROVED, CENSUS_BASE, approved_base

FIELDS = 'pub struct Computed {\n    pub width: f32,\n    pub height: f32,\n}\n'
LAYOUT = 'crates/html/src/layout/block.rs'


def scan(source, path=LAYOUT):
    return aggregate({DEFINITIONS: FIELDS, path: source})


class AggregateTests(unittest.TestCase):
    def test_whitespace_comments_literals_and_file_moves(self):
        old = scan('s.width = 1; s.used_mut();')
        new = scan('/* s.height = 2 */\n s\n .width\n = 1;\n s.used_mut();\n'
                   'let text = r#"s.width = 2"#;', 'crates/html/src/layout/moved.rs')
        self.assertEqual(old, new)

    def test_indexed_nested_compound_write_and_multiline_borrow(self):
        counts = scan('s.width[0].nested += 1; s.width.0 = 2; s.height <<= 2; '
                      'f(&mut\n s.width); f(&mut (s.width));')
        self.assertEqual(counts['Computed.width', 'layout', 'write'], 2)
        self.assertEqual(counts['Computed.height', 'layout', 'write'], 1)
        self.assertEqual(counts['Computed.width', 'layout', 'mutable'], 2)
        self.assertFalse(scan('s.width == 2; s.height >= 3;')['Computed.width', 'layout', 'write'])

    def test_no_budget_transfer_between_fields_owners_or_accesses(self):
        old = scan('s.width = 1;')
        for new in (scan('s.height = 1;'), scan('s.width = 1;', 'crates/html/src/dom/a.rs'),
                    scan('f(&mut s.width);')):
            self.assertTrue(growth(new, old, 'test'))

    def test_reads_and_style_construction_are_unrestricted(self):
        self.assertFalse(growth(scan('read(s.width);'), Counter(), 'test'))
        self.assertFalse(growth(scan('s.width = 1;', 'crates/html/src/style/build.rs'), Counter(), 'test'))

    def test_escapes_whole_value_borrows_and_calls(self):
        for source in ('s.used_mut();', 'Rc::make_mut(&mut s);', 's.get_mut();',
                       'fn f(s: &mut Computed) {}'):
            self.assertTrue(growth(scan(source), Counter(), 'test'), source)
        self.assertFalse(growth(scan('fn used_mut() {}'), Counter(), 'test'))

    def test_style_initializers_patterns_and_shorthand_are_guarded(self):
        self.assertTrue(growth(scan('Computed { width: 2, ..old }'), Counter(), 'test'))
        self.assertTrue(growth(scan('let Computed { width: x } = value;'), Counter(), 'test'))
        self.assertTrue(growth(scan('Computed { width, ..old }'), Counter(), 'test'))
        self.assertFalse(growth(scan('Other { width: 2 }; fn f(width: f32) {}'), Counter(), 'test'))
        self.assertFalse(scan('Computed::width();')['Computed.width', 'layout', 'initialize'])

    def test_roundtrip_and_duplicate_rejection(self):
        counts = scan('s.width = 1; read(s.width); s.used_rc();')
        encoded = list(documents(counts).values())
        self.assertEqual(decode(encoded), counts)
        with self.assertRaises(ValueError):
            decode(encoded + encoded)

    def test_explicit_test_ownership_and_descendant_modules(self):
        sources = {DEFINITIONS: FIELDS, LAYOUT: '''
            #[cfg(test)] mod fixtures;
            #[cfg(test)] mod inline { fn f() { s.used_mut(); } }
            #[test] fn isolated() { s.width = 1; }
            fn production() { s.width = 2; }
        ''', 'crates/html/src/layout/block/fixtures.rs': 's.used_mut(); mod nested;',
            'crates/html/src/layout/block/fixtures/nested.rs': 's.width = 1;',
            'crates/html/src/layout/tests.rs': 's.width = 1;'}
        counts = aggregate(sources)
        self.assertEqual(counts['@used_mut', 'layout:test', 'escape'], 2)
        self.assertEqual(counts['Computed.width', 'layout:test', 'write'], 2)
        self.assertEqual(counts['Computed.width', 'layout', 'write'], 2)
        only_tests = scan('#[cfg(test)] mod tests { fn f() { s.used_mut(); } }')
        self.assertFalse(growth(only_tests, Counter(), 'test'))
        self.assertTrue(growth(scan('#[cfg(not(test))] fn f() { s.used_mut(); }'), Counter(), 'test'))

    def test_custom_path_cannot_exempt_an_unrelated_default_module(self):
        for attributes in ('#[cfg(test)] #[path="elsewhere.rs"]',
                           '#[path="elsewhere.rs"] #[cfg(test)]'):
            counts = aggregate({DEFINITIONS: FIELDS, LAYOUT: attributes + ' mod fixtures;',
                                'crates/html/src/layout/block/fixtures.rs': 's.used_mut();'})
            self.assertTrue(growth(counts, Counter(), 'test'))


class GitRatchetTests(unittest.TestCase):
    def test_f2_rebaseline_is_exact_bounded_and_expires_after_integration(self):
        before = Counter({(f, o, a): old for f, o, a, old, new in APPROVED})
        after = Counter({(f, o, a): new for f, o, a, old, new in APPROVED})
        self.assertEqual(len(APPROVED), 19)
        self.assertEqual(approved_base(before, CENSUS_BASE), after)
        self.assertFalse(growth(after, approved_base(before, CENSUS_BASE), 'test'))
        for key in after:
            extra = after.copy()
            extra[key] += 1
            self.assertTrue(growth(extra, approved_base(before, CENSUS_BASE), 'test'))
        extra = after + Counter({('Computed.width', 'layout', 'write'): 1})
        self.assertTrue(growth(extra, approved_base(before, CENSUS_BASE), 'test'))
        self.assertEqual(approved_base(before, 'later-main'), before)
        with self.assertRaises(ValueError):
            approved_base(after, CENSUS_BASE)
        # Future bases use actual reduced counters, without this old allowance.
        reduced = after.copy()
        reduced['Computed.height', 'layout', 'write'] -= 1
        self.assertTrue(growth(after, approved_base(reduced, 'later-main'), 'test'))

    def test_bootstrap_growth_budget_inflation_and_reduction_without_refresh(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git(root, 'init', '-q')
            for name, source in {DEFINITIONS: FIELDS, LAYOUT: 's.width = 1;'}.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(source, encoding='utf-8')
            directory = root / 'scripts/computed_manifest'
            directory.mkdir(parents=True)
            base = commit(root)
            self.assertEqual(check(root, base, write=True)[0], [])
            registered = commit(root)
            # Bootstrap against a base without registry cannot hide new debt.
            (root / LAYOUT).write_text('s.width = 1; s.height = 2;', encoding='utf-8')
            self.assertTrue(check(root, base)[0])
            (root / LAYOUT).write_text('s.width = 1;', encoding='utf-8')
            (root / LAYOUT).write_text('read(s.height);', encoding='utf-8')
            self.assertEqual(check(root, registered)[0], [])
            reduced = commit(root)
            (root / LAYOUT).write_text('s.width = 1;', encoding='utf-8')
            self.assertTrue(any('base source counters' in e for e in check(root, reduced)[0]))
            (root / LAYOUT).write_text('read(s.height);', encoding='utf-8')
            inflated = scan('s.width = 1; s.width = 2;')
            for name, data in documents(inflated).items():
                (directory / name).write_text(data, encoding='utf-8')
            self.assertTrue(any('ceiling increase' in e for e in check(root, registered)[0]))


if __name__ == '__main__':
    unittest.main()
