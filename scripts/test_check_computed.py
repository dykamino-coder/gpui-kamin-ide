"""Regressions for stable aggregates, shrinking ceilings and bootstrap checks."""

from collections import Counter
from contextlib import redirect_stdout
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from computed_access import DEFINITIONS, aggregate, growth
from check_computed import check, decode, documents, owner_changes, transferred_growth
from test_check_transitional import commit, git
from computed_f2_rebaseline import APPROVED, CENSUS_BASE, OVERFLOW_FLOAT_BASE, OVERFLOW_FLOAT_NON_STYLE, approved_base

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
    def test_overflow_float_non_style_review_is_exact_and_expires(self):
        before = Counter({(f, o, a): old for f, o, a, old, new in OVERFLOW_FLOAT_NON_STYLE})
        after = Counter({(f, o, a): new for f, o, a, old, new in OVERFLOW_FLOAT_NON_STYLE})
        self.assertEqual(approved_base(before, OVERFLOW_FLOAT_BASE), after)
        self.assertEqual(approved_base(before, 'later-main'), before)
        self.assertEqual(len(OVERFLOW_FLOAT_NON_STYLE), 2)
        for key in [*after, ('Computed.width', 'layout', 'write')]:
            self.assertTrue(growth(after + Counter({key: 1}), approved_base(before, OVERFLOW_FLOAT_BASE), 'test'))
        with self.assertRaises(ValueError):
            approved_base(after, OVERFLOW_FLOAT_BASE)

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

    def test_rebaseline_allowance_cannot_donate_but_actual_source_removal_can(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git(root, 'init', '-q')
            for name, source in {DEFINITIONS: FIELDS, LAYOUT: 's.width = 1; s.width = 2;'}.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(source, encoding='utf-8')
            directory = root / 'scripts/computed_manifest'
            directory.mkdir(parents=True)
            # Model a census source with a larger, explicitly approved ceiling.
            for name, data in documents(scan('s.width = 1; s.width = 2; s.width = 3;')).items():
                (directory / name).write_text(data, encoding='utf-8')
            base = commit(root)
            key = 'Computed.width', 'layout', 'write'

            def allowance(counts, revision):
                result = counts.copy()
                result[key] = 3
                return result

            paint = root / 'crates/html/src/paint/a.rs'
            paint.parent.mkdir(parents=True)
            with patch('check_computed.approved_base', side_effect=allowance):
                self.assertEqual(check(root, base)[0], [])
                paint.write_text('s.width = 1;', encoding='utf-8')
                original = {p.name: p.read_bytes() for p in directory.glob('access-*.json')}
                self.assertTrue(check(root, base, write=True)[0])
                self.assertEqual(original, {p.name: p.read_bytes()
                                            for p in directory.glob('access-*.json')})
                (root / LAYOUT).write_text('s.width = 1;', encoding='utf-8')
                self.assertEqual(check(root, base, write=True)[0], [])

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


class OwnerTransferTests(unittest.TestCase):
    def compare(self, old, new, limits=None):
        changes = owner_changes(new, old)
        return transferred_growth(new, old if limits is None else limits, 'fixture', changes)

    def test_exact_transfer_and_transfer_with_growth(self):
        old = scan('s.width = 1; s.width = 2;')
        new = scan('s.width = 1; s.width = 2;', 'crates/html/src/paint/a.rs')
        errors, transfers = self.compare(old, new)
        self.assertEqual(errors, [])
        self.assertEqual(transfers, ['fixture: transfer Computed.width/write: layout -> paint (2)'])
        new['Computed.width', 'paint', 'write'] += 1
        self.assertTrue(self.compare(old, new)[0])

    def test_reduction_can_accompany_a_transfer(self):
        old = scan('s.width = 1; s.width = 2;')
        new = scan('s.width = 1;', 'crates/html/src/paint/a.rs')
        self.assertEqual(self.compare(old, new)[0], [])

    def test_fields_accesses_tests_and_style_cannot_fund_production_debt(self):
        old = scan('s.width = 1;')
        for new in (scan('s.height = 1;', 'crates/html/src/paint/a.rs'),
                    scan('f(&mut s.width);', 'crates/html/src/paint/a.rs')):
            self.assertTrue(self.compare(old, new)[0])
        for donor in (scan('#[test] fn fixture() { s.width = 1; }'),
                      scan('s.width = 1;', 'crates/html/src/style/a.rs')):
            self.assertTrue(self.compare(donor, old)[0])

    def test_escape_transfer_and_multiple_donors_are_deterministic(self):
        key = '@borrow_mut'
        old = Counter({(key, 'layout', 'escape'): 2, (key, 'paint', 'escape'): 1})
        new = Counter({(key, 'resources', 'escape'): 3})
        errors, transfers = self.compare(old, new)
        self.assertEqual(errors, [])
        self.assertEqual(transfers, [
            'fixture: transfer @borrow_mut/escape: layout -> resources (2)',
            'fixture: transfer @borrow_mut/escape: paint -> resources (1)'])

    def test_historical_slack_and_unrelated_donors_cannot_fund_transfer(self):
        old = scan('s.width = 1;')
        new = scan('s.width = 1;', 'crates/html/src/paint/a.rs')
        limits = Counter({('Computed.width', 'resources', 'write'): 1})
        self.assertTrue(self.compare(old, new, limits)[0])
        # Source debt was reduced earlier: an old ceiling cannot resurrect it.
        self.assertTrue(self.compare(Counter(), new, old)[0])

    def test_registered_transfer_prints_and_refreshes_without_inflation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git(root, 'init', '-q')
            for name, text in {DEFINITIONS: FIELDS, LAYOUT: 's.width = 1;'}.items():
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text, encoding='utf-8')
            directory = root / 'scripts/computed_manifest'
            directory.mkdir(parents=True)
            base = commit(root)
            self.assertEqual(check(root, base, write=True)[0], [])
            registered = commit(root)
            (root / LAYOUT).write_text('read(s.width);', encoding='utf-8')
            paint = root / 'crates/html/src/paint/a.rs'
            paint.parent.mkdir(parents=True)
            paint.write_text('s.width = 1;', encoding='utf-8')
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(check(root, registered)[0], [])
                self.assertEqual(check(root, registered, write=True)[0], [])
            self.assertIn('transfer Computed.width/write: layout -> paint (1)', output.getvalue())
            self.assertEqual(check(root, registered)[0], [])
            ceilings = decode(p.read_text(encoding='utf-8') for p in directory.glob('access-*.json'))
            self.assertEqual(ceilings['Computed.width', 'layout', 'write'], 0)
            self.assertEqual(ceilings['Computed.width', 'paint', 'write'], 1)
            transferred = commit(root)
            self.assertEqual(check(root, transferred)[0], [])
            paint.write_text('s.width = 1; s.width = 2;', encoding='utf-8')
            self.assertTrue(check(root, transferred, write=True)[0])

    def test_transfer_with_growth_cannot_refresh_or_inflate_ceilings(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git(root, 'init', '-q')
            for name, text in {DEFINITIONS: FIELDS, LAYOUT: 's.width = 1;'}.items():
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text, encoding='utf-8')
            directory = root / 'scripts/computed_manifest'
            directory.mkdir(parents=True)
            base = commit(root)
            self.assertEqual(check(root, base, write=True)[0], [])
            registered = commit(root)
            original = {p.name: p.read_bytes() for p in directory.glob('access-*.json')}
            (root / LAYOUT).write_text('', encoding='utf-8')
            paint = root / 'crates/html/src/paint/a.rs'
            paint.parent.mkdir(parents=True)
            paint.write_text('s.width = 1; s.width = 2;', encoding='utf-8')
            self.assertTrue(check(root, registered, write=True)[0])
            self.assertEqual(original, {p.name: p.read_bytes() for p in directory.glob('access-*.json')})
            for name, data in documents(scan('s.width = 1; s.width = 2;',
                                            'crates/html/src/paint/a.rs')).items():
                (directory / name).write_text(data, encoding='utf-8')
            self.assertTrue(any('ceiling increase' in e for e in check(root, registered)[0]))


if __name__ == '__main__':
    unittest.main()
