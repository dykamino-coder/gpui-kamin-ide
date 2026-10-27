"""Contract widening follows current modules/exports and preserves test groups."""
import unittest

from canonical_exports import CanonicalExports
from migration_test_support import widen, SUPPORT
from module_sources import modules

OWNERS = {'style': 'kamin-style', 'layout': 'kamin-layout', 'render': 'kamin-html'}


class SupportTests(unittest.TestCase):
    def sources(self):
        return {
            'lib.rs': 'mod style; mod layout; mod render;\n',
            'style.rs': 'pub struct Value;\n',
            'layout/mod.rs': '#[cfg(test)] pub use self::cases::relay as run;\n'
                             '#[cfg(test)] mod cases;\n'
                             '#[cfg(test)] mod unrelated { #[test] fn stays() {} }\n',
            'layout/cases.rs': '#[cfg(test)] pub use self::inner::body as relay;\n'
                               '#[cfg(test)] #[path = "body.rs"] mod inner;\n',
            'layout/body.rs': 'pub fn body(_: fn() -> crate::style::Value) {}\n'
                              '#[test] fn own_case() {}\n',
            'render.rs': '#[cfg(test)] mod tests { #[test] fn run() {\n'
                         'crate::layout::run(|| crate::style::Value); } }\n',
        }

    def widen(self, sources):
        declarations, files = modules(sources)
        return widen(sources, declarations, files, CanonicalExports(sources, files), OWNERS)

    def test_reexport_chain_and_module_ancestry_only(self):
        sources = self.sources()
        after = self.widen(sources)
        self.assertEqual(after['layout/cases.rs'].count(SUPPORT), 2)
        self.assertEqual(after['layout/mod.rs'].count(SUPPORT), 2)
        self.assertIn('#[cfg(test)] mod unrelated', after['layout/mod.rs'])
        for name in ('lib.rs', 'style.rs', 'render.rs', 'layout/body.rs'):
            self.assertEqual(after[name], sources[name])

    def test_idempotence_and_external_upper_references(self):
        sources = self.sources()
        first = self.widen(sources)
        self.assertEqual(self.widen(first), first)
        sources['render.rs'] = sources['render.rs'].replace('crate::layout', 'kamin_layout::layout')
        self.assertEqual(self.widen(sources)['layout/mod.rs'], first['layout/mod.rs'])

    def test_no_filename_or_symbol_prefix_exemptions(self):
        sources = self.sources()
        sources['render.rs'] = '#[cfg(test)] mod tests {}\n'
        self.assertEqual(self.widen(sources), sources)

    def test_comments_literals_and_other_cfg_are_untouched(self):
        sources = self.sources()
        sources['layout/body.rs'] += ('// #[cfg(test)] mod fabricated;\n'
            'const S: &str = "#[cfg(test)] mod fabricated;";\n'
            '#[cfg(all(test, feature = "other"))] pub fn other() {}\n')
        self.assertEqual(self.widen(sources)['layout/body.rs'], sources['layout/body.rs'])

    def test_item_gate_is_widened_without_widening_sibling_test(self):
        sources = self.sources()
        sources['layout/body.rs'] = '#[cfg(test)] pub fn body(_: fn() -> crate::style::Value) {}\n'
        sources['layout/body.rs'] += '#[cfg(test)] fn local_only() {}\n'
        after = self.widen(sources)['layout/body.rs']
        self.assertEqual(after.count(SUPPORT), 1)
        self.assertIn('#[cfg(test)] fn local_only()', after)

    def test_ambiguous_alias_rejects_before_writing(self):
        sources = self.sources()
        sources['layout/mod.rs'] += 'pub use self::other::body as run; mod other { pub fn body() {} }\n'
        with self.assertRaisesRegex(ValueError, 'Ambiguous upper test-contract'):
            self.widen(sources)

    def test_real_public_definition_shadows_parent_glob(self):
        sources = self.sources()
        sources['style.rs'] = 'mod fields; pub use self::fields::*;\n'
        sources['style/fields.rs'] = 'use super::*; pub struct Value;\n'
        after = self.widen(sources)
        self.assertEqual(after['style/fields.rs'], sources['style/fields.rs'])
        self.assertEqual(after['layout/mod.rs'].count(SUPPORT), 2)
        _, files = modules(sources)
        exports = CanonicalExports(sources, files)
        self.assertEqual(exports.resolve(('style', 'Value')), {('style', 'fields', 'Value')})

    def test_inherent_impl_gate_tracks_method_not_production_type(self):
        sources = self.sources()
        sources['layout/mod.rs'] += ('pub struct Fixture;\n'
            '#[cfg(test)] impl Fixture { pub fn helper() {} }\n')
        sources['render.rs'] += 'fn ordinary(_: crate::layout::Fixture) {}\n'
        after = self.widen(sources)
        self.assertIn('#[cfg(test)] impl Fixture', after['layout/mod.rs'])
        sources['render.rs'] += '#[cfg(test)] fn uses_helper() { crate::layout::Fixture::helper(); }\n'
        after = self.widen(sources)
        self.assertIn(SUPPORT + ' impl Fixture', after['layout/mod.rs'])

    def test_unknown_impl_contract_rejects(self):
        sources = self.sources()
        sources['layout/mod.rs'] += '#[cfg(test)] impl<T> Fixture<T> {}\n'
        with self.assertRaisesRegex(ValueError, r'layout/mod.rs:\d+: Unsupported test-contract impl header'):
            self.widen(sources)


if __name__ == '__main__':
    unittest.main()
