"""Defining phases must remain visible through neutral Rust reexports."""

from pathlib import Path
import tempfile
import unittest

from check_layer_imports import ALLOWLIST, STAGE, check
from rust_layer_exports import Exports


class ReexportTests(unittest.TestCase):
    def resolve(self, sources, path):
        return Exports(sources).paths(path, STAGE)

    def test_root_intermediate_alias_and_glob_reexports(self):
        for bridge, path in [
            ('pub use text::metrics;', 'metrics::spacing_px'),
            ('pub use crate::text::metrics as neutral;', 'neutral::spacing_px'),
            ('pub mod bridge { pub use crate::text::metrics as m; }',
             'bridge::m::spacing_px'),
            ('pub mod bridge { pub use crate::text::metrics::*; }',
             'bridge::spacing_px'),
        ]:
            with self.subTest(path=path), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                source = root / 'crates/html/src'
                (source / 'style').mkdir(parents=True)
                (source / 'text').mkdir()
                (root / 'scripts').mkdir()
                (root / ALLOWLIST).write_text('[]', encoding='utf-8')
                (source / 'lib.rs').write_text(bridge, encoding='utf-8')
                (source / 'text/metrics.rs').write_text(
                    'pub fn spacing_px() {}', encoding='utf-8')
                (source / 'style/probe.rs').write_text(
                    f'use crate::{path};', encoding='utf-8')
                errors, references, _ = check(root)
                self.assertEqual(references, 1)
                self.assertIn('text::metrics::spacing_px', errors[0])

    def test_file_modules_relative_paths_nested_trees_and_multiple_hops(self):
        sources = {
            'lib.rs': 'pub use bridge::{inner::{self as api, measure as m}};',
            'bridge/mod.rs': 'pub mod inner { pub use super::super::text::metrics as measure; }',
        }
        for path in ['api::measure::spacing_px', 'm::spacing_px']:
            self.assertEqual(self.resolve(sources, path), ['text::metrics::spacing_px'])

    def test_glob_chains_and_namespace_imports_cannot_hide_symbols(self):
        sources = {
            'lib.rs': 'pub use bridge::*;',
            'bridge.rs': 'pub use crate::facade::*;',
            'facade.rs': 'pub use crate::text::metrics::spacing_px;',
        }
        for path in ['spacing_px', 'bridge::*', 'bridge']:
            self.assertEqual(self.resolve(sources, path), ['text::metrics::spacing_px'])

    def test_private_imports_do_not_escape_through_a_glob(self):
        sources = {
            'lib.rs': 'pub use bridge::*;',
            'bridge.rs': 'use crate::text::metrics; pub fn safe() {}',
        }
        self.assertEqual(self.resolve(sources, 'metrics'), [])
        self.assertEqual(self.resolve(sources, 'bridge::metrics::spacing_px'),
                         ['text::metrics::spacing_px'])

    def test_explicit_import_shadows_globs_and_cycles_terminate(self):
        sources = {
            'lib.rs': 'pub use bridge::*; pub use crate::style::safe as measured;',
            'bridge.rs': 'pub use crate::facade::*; pub use crate::text::metrics as measured;',
            'facade.rs': 'pub use crate::bridge::*;',
        }
        self.assertEqual(self.resolve(sources, 'measured::x'), ['style::safe::x'])
        self.assertEqual(self.resolve(sources, 'facade::measured::x'), ['text::metrics::x'])

    def test_cfg_branches_restricted_visibility_and_function_modifiers(self):
        sources = {
            'lib.rs': '#[cfg(any())] pub(in crate) use bridge::*;',
            'bridge.rs': '#[cfg(any())] pub(crate) use crate::text::metrics; '
                         'pub unsafe extern "C" fn safe() {}',
        }
        self.assertEqual(self.resolve(sources, 'metrics::x'), ['text::metrics::x'])

    def test_same_named_module_and_reexported_function_use_separate_namespaces(self):
        sources = {
            'lib.rs': 'pub use text::metrics;',
            'text/metrics/mod.rs': 'mod normal_px; pub use normal_px::normal_px;',
            'text/metrics/normal_px.rs': 'pub fn normal_px() {}',
        }
        self.assertEqual(self.resolve(sources, 'metrics::normal_px'),
                         ['text::metrics::normal_px::normal_px'])


if __name__ == '__main__':
    unittest.main()
