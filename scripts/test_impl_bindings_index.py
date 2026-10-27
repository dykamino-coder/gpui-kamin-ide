"""Compare indexed impl bindings with the original per-round full scan."""
import unittest

from canonical_exports import CanonicalExports
from impl_bindings import ImplBindings
from module_sources import modules


class ReferenceBindings(ImplBindings):
    def expand_globs(self):
        changed = True
        while changed:
            changed = False
            public = set(self.exported)
            local = self.declared | set(self.aliases)
            for module, targets in self.globs.items():
                for target in targets:
                    for resolved in self.resolve(target):
                        names = public | local if module[:len(resolved)] == resolved else public
                        for name in names:
                            if len(name) != len(resolved) + 1 or name[:-1] != resolved:
                                continue
                            alias = module + name[-1:]
                            if alias in self.explicit or alias == name:
                                continue
                            if name not in self.aliases[alias]:
                                self.aliases[alias].add(name)
                                if (module, target) in self.public_globs:
                                    self.exported.add(alias)
                                changed = True


class IndexedBindingTests(unittest.TestCase):
    def compare(self, sources, declared, paths):
        _, files = modules(sources)
        exports = CanonicalExports(sources, files)
        original = {k: set(v) for k, v in exports.aliases.items()}
        actual = ImplBindings(exports, declared)
        reference = ReferenceBindings(exports, declared)
        for attribute in ('aliases', 'globs', 'public_globs', 'exported', 'modules',
                          'explicit', 'declared', 'ambiguous'):
            self.assertEqual(getattr(actual, attribute), getattr(reference, attribute), attribute)
        self.assertEqual(dict(exports.aliases), original)
        for path in paths:
            self.assertEqual(actual.resolve(path), reference.resolve(path), path)
        return actual

    def test_descendant_sees_private_parent_but_sibling_does_not(self):
        result = self.compare({
            'lib.rs': 'mod a; mod sibling;',
            'a.rs': 'struct Hidden; pub struct Visible; mod child;',
            'a/child.rs': 'use super::*;',
            'sibling.rs': 'use crate::a::*;',
        }, {('a', 'Hidden'), ('a', 'Visible')},
            [('a', 'child', 'Hidden'), ('sibling', 'Hidden'), ('sibling', 'Visible')])
        self.assertEqual(result.resolve(('a', 'child', 'Hidden')), {('a', 'Hidden')})
        self.assertNotIn(('sibling', 'Hidden'), result.aliases)

    def test_private_declaration_shadows_glob(self):
        result = self.compare({
            'lib.rs': 'mod a; mod b;',
            'a.rs': 'pub struct Value;',
            'b.rs': 'use crate::a::*; struct Value;',
        }, {('a', 'Value'), ('b', 'Value')}, [('b', 'Value')])
        self.assertEqual(result.resolve(('b', 'Value')), {('b', 'Value')})

    def test_public_chain_and_ambiguous_donors(self):
        result = self.compare({
            'lib.rs': 'mod a; mod b; mod c; mod d;',
            'a.rs': 'pub struct Value;',
            'b.rs': 'pub struct Value;',
            'c.rs': 'pub use crate::a::*; pub use crate::b::*;',
            'd.rs': 'pub use crate::c::*;',
        }, {('a', 'Value'), ('b', 'Value')}, [('c', 'Value'), ('d', 'Value')])
        self.assertEqual(result.resolve(('d', 'Value')), {('a', 'Value'), ('b', 'Value')})

    def test_cfg_ambiguous_explicit_binding_stays_unresolved(self):
        result = self.compare({
            'lib.rs': 'mod a; mod b;',
            'a.rs': 'pub struct Value;',
            'b.rs': 'pub use crate::a::Value; struct Value;',
        }, {('a', 'Value'), ('b', 'Value')}, [('b', 'Value')])
        self.assertIn(('__impl_binding_requires_cfg_review__', 'b', 'Value'),
                      result.resolve(('b', 'Value')))

    def test_parent_glob_cycle(self):
        result = self.compare({
            'lib.rs': 'mod a;',
            'a.rs': 'mod child; pub use child::*;',
            'a/child.rs': 'use super::*; pub struct Value;',
        }, {('a', 'child', 'Value')}, [('a', 'Value'), ('a', 'child', 'Value')])
        self.assertEqual(result.resolve(('a', 'Value')), {('a', 'child', 'Value')})


if __name__ == '__main__':
    unittest.main()
