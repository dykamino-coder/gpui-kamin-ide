"""Regression tests for Rust comment markers and the shrinking temporary inventory."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from check_transitional import REGISTRY, REPO, check, entries, growth, markers


ITEM = {'file': 'crates/html/src/text/shape.rs', 'task': 'F1-6',
        'reason': 'shape_full remains until continuation shaping is integrated'}


def git(root, *args):
    return subprocess.run(['git', '-C', str(root), *args], check=True,
                          capture_output=True, text=True).stdout.strip()


def commit(root):
    git(root, 'add', '.')
    git(root, '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
        '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')
    return git(root, 'rev-parse', 'HEAD')


class CommentTests(unittest.TestCase):
    def test_line_doc_block_and_nested_comments(self):
        source = '''// TRANSITIONAL(F1-6): первая причина
/// TRANSITIONAL(F2-4): doc reason
//! TRANSITIONAL(F2-5): module reason
/* TRANSITIONAL(F2-6): outer
   /* TRANSITIONAL(F2-7): nested */
   TRANSITIONAL(F2-8): after nested */
fn f() {} // TRANSITIONAL(F2-9): inline
'''
        self.assertEqual(list(markers(source)), [
            ('F1-6', 'первая причина', 1), ('F2-4', 'doc reason', 2),
            ('F2-5', 'module reason', 3), ('F2-6', 'outer', 4),
            ('F2-7', 'nested', 5), ('F2-8', 'after nested', 6),
            ('F2-9', 'inline', 7),
        ])

    def test_literals_are_not_comments_and_diagnostic_lines_survive(self):
        source = '''const S: &str = "// TRANSITIONAL(F2-1): ignored \\" quote";
const R: &str = r###"/* TRANSITIONAL(F2-2): ignored */"###;
const B: &[u8] = br#"// TRANSITIONAL(F2-3): ignored"#;
const C: &CStr = cr#"// TRANSITIONAL(F2-4): ignored"#;
const T: &str = "multiline
// TRANSITIONAL(F2-5): ignored";
const CH: char = '\\'';
const SLASH: u8 = b'/';
fn f<'a>(_: &'a str) {}
// TRANSITIONAL(F1-6): real
'''
        self.assertEqual(list(markers(source)), [('F1-6', 'real', 10)])

    def test_cfg_tests_and_comment_decoration(self):
        source = '''#[cfg(any())]
mod tests {
    /*
     * TRANSITIONAL(F2-4): keep fallback until fragments carry state
     */
}'''
        self.assertEqual(list(markers(source)), [
            ('F2-4', 'keep fallback until fragments carry state', 4),
        ])

    def test_malformed_annotations_fail(self):
        for text in ('TRANSITIONAL(): reason', 'TRANSITIONAL(F2-4):   ',
                     'TRANSITIONAL(F2-4) reason', 'TRANSITIONAL(F2 4): reason',
                     'TRANSITIONAL(F2-4): first TRANSITIONAL(F2-5): second'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                list(markers('// ' + text))


class RegistryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'scripts').mkdir()
        self.registry([])

    def registry(self, items):
        (self.root / REGISTRY).write_text(json.dumps(items), encoding='utf-8')

    def source(self, file=ITEM['file'], content=None):
        path = self.root / file
        path.parent.mkdir(parents=True, exist_ok=True)
        if content is None:
            content = f"// TRANSITIONAL({ITEM['task']}): {ITEM['reason']}\n"
        path.write_text(content, encoding='utf-8')
        return path

    def init_git(self):
        git(self.root, 'init', '-q')
        return commit(self.root)

    def cli(self, *args):
        return subprocess.run([sys.executable, str(REPO / 'scripts/check_transitional.py'),
                               '--root', str(self.root), *args],
                              capture_output=True, text=True, encoding='utf-8')

    def test_empty_and_exact_registry(self):
        self.assertEqual(check(self.root), ([], 0, 0))
        self.source()
        self.registry([ITEM])
        self.assertEqual(check(self.root), ([], 1, 1))
        self.source(content='\n' * 30 + f"// TRANSITIONAL({ITEM['task']}): {ITEM['reason']}   \n")
        self.assertEqual(check(self.root), ([], 1, 1))

    def test_exact_file_task_reason_and_stale_removal(self):
        self.source()
        for mutation in ({'file': 'vendor/gpui/src/text_system.rs'},
                         {'task': 'F2-4'}, {'reason': 'different reason'}):
            with self.subTest(mutation=mutation):
                self.registry([ITEM | mutation])
                errors, found, registered = check(self.root)
                self.assertEqual((found, registered), (1, 1))
                self.assertTrue(any('unregistered' in error for error in errors))
                self.assertTrue(any('stale registry entry' in error for error in errors))
        self.registry([ITEM])
        self.source(content='fn f() {}')
        self.assertIn('shrink registry', check(self.root)[0][0])
        self.registry([])
        self.assertEqual(check(self.root), ([], 0, 0))

    def test_vendor_adapters_examples_and_scope(self):
        files = ('vendor/gpui/src/taffy.rs', 'vendor/gpui/src/taffy/adapt.rs',
                 'crates/html/examples/example.rs')
        for file in files:
            self.source(file)
        self.source('crates/shell/src/main.rs')
        self.source('vendor/gpui-kaminide.patch')
        self.registry([ITEM | {'file': file} for file in files])
        self.assertEqual(check(self.root), ([], 3, 3))

    def test_duplicate_markers_fail_instead_of_sharing_one_entry(self):
        path = self.source()
        self.source(content=path.read_text(encoding='utf-8') * 2)
        self.registry([ITEM])
        errors, found, registered = check(self.root)
        self.assertEqual((found, registered), (2, 1))
        self.assertIn('duplicate transitional identity', errors[0])

    def test_registry_schema_exact_paths_and_duplicates(self):
        key = (ITEM['file'], ITEM['task'], ITEM['reason'])
        self.assertEqual(entries([ITEM]), {key})
        invalid = ({'task': ''}, {'task': 'F2 4'}, {'reason': ' '}, {'reason': 'two\nlines'},
                   {'reason': 1}, {'file': 'crates/html/*.rs'},
                   {'file': 'crates/html/../shell/x.rs'}, {'file': 'crates/html//x.rs'},
                   {'file': 'crates/html/./x.rs'}, {'file': 'crates/html/x?.rs'},
                   {'file': 'crates/html/x\\y.rs'}, {'file': '/crates/html/x.rs'},
                   {'file': 'crates/shell/x.rs'}, {'file': 'vendor/gpui.patch'},
                   {'line': 7}, {'reason': ' trailing '})
        for mutation in invalid:
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                entries([ITEM | mutation])
        for data in ({}, [None], [ITEM, ITEM], [{'file': ITEM['file']}]):
            with self.subTest(data=data), self.assertRaises(ValueError):
                entries(data)

    def test_count_ratchet_allows_equal_count_and_reduction(self):
        self.registry([ITEM, ITEM | {'task': 'F2-4'}])
        base = self.init_git()
        for items in ([], [ITEM], [ITEM | {'reason': 'replacement reason'}, ITEM]):
            self.assertEqual(growth(self.root, base, entries(items)), [])
        errors = growth(self.root, base, entries([
            ITEM | {'task': task} for task in ('F1-6', 'F2-4', 'F2-5')]))
        self.assertIn('2 -> 3', errors[0])
        self.registry([ITEM])
        self.source()
        self.assertEqual(check(self.root, base), ([], 1, 1))

    def test_missing_base_registry_has_zero_budget(self):
        (self.root / REGISTRY).unlink()
        (self.root / 'README').write_text('fixture', encoding='utf-8')
        base = self.init_git()
        self.assertEqual(growth(self.root, base, set()), [])
        self.assertIn('0 -> 1', growth(self.root, base, entries([ITEM]))[0])

    def test_merge_base_matches_layer_check_semantics(self):
        self.registry([ITEM])
        base = self.init_git()
        self.registry([])
        advanced = commit(self.root)
        git(self.root, 'checkout', '--detach', base)
        self.assertEqual(growth(self.root, advanced, entries([ITEM])), [])

    def test_cli_injected_marker_fails_then_is_removed(self):
        self.init_git()
        git(self.root, 'update-ref', 'refs/remotes/origin/main', 'HEAD')
        injected = self.source()
        result = self.cli()
        self.assertEqual(result.returncode, 1)
        self.assertIn('unregistered TRANSITIONAL(F1-6)', result.stderr)
        self.registry([ITEM])
        result = self.cli('--base', 'HEAD')
        self.assertEqual(result.returncode, 1)
        self.assertIn('0 -> 1', result.stderr)
        injected.unlink()
        self.registry([])
        self.assertEqual(self.cli().returncode, 0)

    def test_cli_fails_closed_on_bad_inputs(self):
        self.init_git()
        result = self.cli('--base', 'nonexistent-revision')
        self.assertEqual(result.returncode, 1)
        for content in ('{', '[null]'):
            (self.root / REGISTRY).write_text(content, encoding='utf-8')
            result = self.cli('--base', 'HEAD')
            self.assertEqual(result.returncode, 1)
            self.assertNotIn('Traceback', result.stderr)
        self.registry([])
        self.source(content='// TRANSITIONAL(F2-4):\n')
        result = self.cli('--base', 'HEAD')
        self.assertEqual(result.returncode, 1)
        self.assertIn(ITEM['file'], result.stderr)
        self.assertIn('malformed', result.stderr)


if __name__ == '__main__':
    unittest.main()
