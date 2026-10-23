"""Regression tests for phase rules, Rust paths and the shrinking debt inventory."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from check_layer_imports import ALLOWLIST, REPO, STAGE, check, entries, forbidden, growth
from rust_layer_paths import dependencies


class RustPathsTests(unittest.TestCase):
    def paths(self, source, filename='layout/host.rs'):
        return [path for path, _ in dependencies(source, filename, STAGE)]

    def test_use_trees_aliases_and_qualified_calls(self):
        source = '''pub use crate::{paint::{self as p, effects::{a, b as c}},
                    render::*, layout::cx::LayoutCx};
                    fn f() { crate::paint::effects::grouped(); }'''
        self.assertEqual(self.paths(source), [
            'paint', 'paint::effects::a', 'paint::effects::b', 'render::*',
            'layout::cx::LayoutCx', 'paint::effects::grouped',
        ])

    def test_relative_paths_in_file_modules_and_inline_modules(self):
        source = '''use super::super::paint::Painter;
                    mod tests { use super::super::super::{render::build, text::Node}; }
                    use self::local::Item;'''
        self.assertEqual(self.paths(source), [
            'paint::Painter', 'render::build', 'text::Node', 'layout::host::local::Item',
        ])
        self.assertEqual(self.paths('use super::super::paint::P;', 'layout/host/mod.rs'),
                         ['paint::P'])

    def test_comments_literals_local_modules_and_diagnostic_lines(self):
        source = '''// use crate::paint::Ignored;
/* nested /* crate::render::Ignored */ comment */
const S: &str = r###"use crate::paint::Ignored;"###;
const B: &[u8] = br#"crate::render::Ignored"#;
const C: char = '\\'';
use paint::local;
use crate::paint::Real;'''
        result = list(dependencies(source, 'style/cascade/inherit.rs', STAGE))
        self.assertEqual(result, [('style::cascade::inherit::paint::local', 6),
                                  ('paint::Real', 7)])

    def test_cfg_and_macro_paths_are_checked(self):
        self.assertEqual(self.paths('''#[cfg(any())] use crate::render::hidden;
            macro_rules! x { () => { $crate::paint::effect() } }'''),
                         ['render::hidden', 'paint::effect'])


class BoundaryTests(unittest.TestCase):
    def test_architecture_matrix(self):
        self.assertFalse(forbidden('dom', 'style'))  # DOM construction runs the cascade.
        self.assertFalse(forbidden('style', 'dom'))
        self.assertFalse(forbidden('layout', 'text'))
        self.assertFalse(forbidden('text', 'layout'))
        self.assertFalse(forbidden('paint', 'svg'))
        self.assertFalse(forbidden('svg', 'paint'))
        self.assertFalse(forbidden('paint', 'layout'))
        self.assertFalse(forbidden('render', 'paint'))
        self.assertTrue(forbidden('dom', 'layout'))
        self.assertTrue(forbidden('style', 'layout'))
        self.assertTrue(forbidden('text', 'paint'))
        self.assertTrue(forbidden('paint', 'render'))

    def test_root_module_files_are_checked(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'crates/html/src'
            source.mkdir(parents=True)
            (root / 'scripts').mkdir()
            (root / ALLOWLIST).write_text('[]', encoding='utf-8')
            (source / 'dom.rs').write_text('use crate::layout::Box;', encoding='utf-8')
            (source / 'text.rs').write_text('use crate::paint::P;', encoding='utf-8')
            errors, references, _ = check(root)
            self.assertEqual(references, 2)
            self.assertTrue(any('layout::Box' in error for error in errors))
            self.assertTrue(any('paint::P' in error for error in errors))

    def test_intentional_reverse_import_fails_cli_then_is_removed(self):
        # Inject into a disposable fixture, never into committed engine sources.
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'crates/html/src/layout').mkdir(parents=True)
            (root / 'scripts').mkdir()
            (root / ALLOWLIST).write_text('[]', encoding='utf-8')
            injected = root / 'crates/html/src/layout/injected.rs'
            injected.write_text('use crate::{paint::Painter, render::build};', encoding='utf-8')
            result = subprocess.run([sys.executable, str(REPO / 'scripts/check_layer_imports.py'),
                                     '--root', str(root)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn('paint::Painter', result.stderr)
            self.assertIn('render::build', result.stderr)
            injected.unlink()
            self.assertEqual(check(root), ([], 0, 0))

    def test_exact_budget_metadata_and_stale_entries(self):
        item = {'file': 'layout/host.rs', 'path': 'paint::P', 'count': 1,
                'owner': 'layout team', 'task': 'move P to layout'}
        self.assertEqual(entries([item]), {('layout/host.rs', 'paint::P'): 1})
        for mutation in ({'owner': ''}, {'task': ''}, {'count': 0}, {'file': 'layout/*'}):
            with self.assertRaises(ValueError):
                entries([item | mutation])
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'scripts').mkdir()
            (root / 'crates/html/src').mkdir(parents=True)
            (root / ALLOWLIST).write_text(json.dumps([item]), encoding='utf-8')
            self.assertIn('stale exception', check(root)[0][0])
            source = root / 'crates/html/src/layout'
            source.mkdir()
            (source / 'host.rs').write_text('use crate::paint::P;\n' * 2, encoding='utf-8')
            self.assertIn('2 references, allowed 1', check(root)[0][0])

    def test_base_inventory_cannot_grow(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'scripts').mkdir()
            (root / ALLOWLIST).write_text('[]', encoding='utf-8')
            def git(*args):
                subprocess.run(['git', '-C', str(root), *args], check=True,
                               capture_output=True)
            git('init', '-q')
            git('add', ALLOWLIST)
            git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                '-c', 'commit.gpgsign=false', 'commit', '-qm', 'baseline')
            self.assertEqual(growth(root, 'HEAD', {}), [])
            self.assertIn('inventory grew', growth(root, 'HEAD', {('layout/x.rs', 'paint::P'): 1})[0])


if __name__ == '__main__':
    unittest.main()
