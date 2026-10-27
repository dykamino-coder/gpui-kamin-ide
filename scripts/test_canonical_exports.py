"""Indexed export traversal must retain shadowing and ambiguity semantics."""
import unittest

from canonical_exports import CanonicalExports
from module_sources import modules
from rust_layer_exports import Exports


class ReferenceExports(CanonicalExports):
    def expand_globs(self):
        self.explicit.update(self.exported - self.aliases.keys())
        Exports.expand_globs(self)


class CanonicalExportTests(unittest.TestCase):
    def compare(self, sources, paths):
        _, files = modules(sources)
        actual = CanonicalExports(sources, files)
        reference = ReferenceExports(sources, files)
        for attribute in ('aliases', 'globs', 'public_globs', 'exported', 'modules', 'explicit'):
            self.assertEqual(getattr(actual, attribute), getattr(reference, attribute), attribute)
        for path in paths:
            self.assertEqual(actual.resolve(path), reference.resolve(path), path)
        return actual

    def test_multiple_glob_rounds_and_private_import_visibility(self):
        sources = {
            'lib.rs': 'mod a; mod b; mod c; mod d; mod e;\n',
            'a.rs': 'pub struct Public; struct Private;\n',
            'b.rs': 'pub use crate::a::*;\n',
            'c.rs': 'pub use crate::b::*;\n',
            'd.rs': 'use crate::c::*;\n',
            'e.rs': 'pub use crate::d::*;\n',
        }
        result = self.compare(sources, [('c', 'Public'), ('d', 'Public'), ('e', 'Public')])
        self.assertEqual(result.resolve(('c', 'Public')), {('a', 'Public')})
        self.assertEqual(result.resolve(('d', 'Public')), {('a', 'Public')})
        self.assertNotIn(('e', 'Public'), result.aliases)
        self.assertNotIn(('b', 'Private'), result.aliases)

    def test_definitions_and_explicit_aliases_shadow_globs(self):
        sources = {
            'lib.rs': 'mod a; mod b; mod c;\n',
            'a.rs': 'pub struct Value; pub struct Other;\n',
            'b.rs': 'pub use crate::a::*; pub struct Value;\n',
            'c.rs': 'pub use crate::a::*; pub use crate::b::Value;\n',
        }
        result = self.compare(sources, [('b', 'Value'), ('c', 'Value'), ('c', 'Other')])
        self.assertEqual(result.resolve(('c', 'Value')), {('b', 'Value')})

    def test_ambiguous_globs_retain_both_defining_owners(self):
        sources = {
            'lib.rs': 'mod a; mod b; mod c; mod d;\n',
            'a.rs': 'pub struct Value;\n',
            'b.rs': 'pub struct Value;\n',
            'c.rs': 'pub use crate::a::*; pub use crate::b::*;\n',
            'd.rs': 'pub use crate::c::*;\n',
        }
        result = self.compare(sources, [('c', 'Value'), ('d', 'Value')])
        self.assertEqual(result.resolve(('d', 'Value')), {('a', 'Value'), ('b', 'Value')})

    def test_public_definition_breaks_parent_glob_cycle(self):
        sources = {
            'lib.rs': 'mod style;\n',
            'style/mod.rs': 'mod fields; pub use self::fields::*;\n',
            'style/fields.rs': 'use super::*; pub struct Computed;\n',
        }
        result = self.compare(sources, [('style', 'Computed'), ('style', 'fields', 'Computed')])
        self.assertEqual(result.resolve(('style', 'Computed')), {('style', 'fields', 'Computed')})


if __name__ == '__main__':
    unittest.main()
