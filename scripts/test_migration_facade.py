"""The staged planner retains facade constructors at their original DOM paths."""
import unittest

import test_migration_plan as fixtures
from migration_facade import recover
from migration_plan import ROOT_OWNERS, plan, stage


class FacadeCompatibilityTests(unittest.TestCase):
    def sources(self):
        sources = fixtures.MigrationPlanTests().sources()
        sources['lib.rs'] = sources['lib.rs'].replace(
            'pub mod dom;', 'pub use crate::document::dom;')
        sources['lib.rs'] += 'pub mod document;\n'
        sources['document/mod.rs'] = 'pub mod dom; pub struct Parsed;\n'
        sources['document/dom.rs'] = (
            'pub use kamin_style::dom::*;\n'
            'pub use super::Parsed;\n')
        return sources

    def apply(self, sources, physical, result):
        roots = {}
        reverse = {value: name for name, value in physical.items()}
        for change in result.changes:
            if change.old in reverse:
                name = reverse[change.old]
                sources[name], physical[name] = change.after, change.new
            else:
                package = 'kamin-' + change.new.split('/')[1].removeprefix('html-')
                roots[package] = change.after
        return roots

    def test_wrapper_survives_style_recheck_and_next_layout_stage(self):
        sources = self.sources()
        physical = {name: 'crates/html/src/' + name for name in sources}
        with stage('kamin-style'):
            result = plan(sources, set(physical.values()), physical)
            self.assertFalse(result.blockers)
            roots = self.apply(sources, physical, result)
            repeated = plan(sources, set(physical.values()), physical, roots)
        self.assertEqual(repeated.changes, [])
        self.assertFalse(repeated.blockers)
        self.assertIn('pub use crate::document::dom;', sources['lib.rs'])
        self.assertEqual(sources['document/dom.rs'], self.sources()['document/dom.rs'])
        self.assertEqual(physical['dom/mod.rs'], 'crates/html-style/src/dom/mod.rs')
        self.assertEqual(physical['document/dom.rs'], 'crates/html/src/document/dom.rs')
        with stage('kamin-layout'):
            layout = plan(sources, set(physical.values()), physical, roots)
            self.assertFalse(layout.blockers)
            roots.update(self.apply(sources, physical, layout))
            repeated = plan(sources, set(physical.values()), physical, roots)
        self.assertEqual(repeated.changes, [])
        self.assertFalse(repeated.blockers)
        self.assertIn('pub use crate::document::dom;', sources['lib.rs'])
        self.assertEqual(physical['layout/mod.rs'], 'crates/html-layout/src/layout/mod.rs')

    def test_wrapper_cannot_hide_an_upper_dependency_in_a_lower_module(self):
        sources = self.sources()
        sources['style/mod.rs'] += 'pub fn backwards(_: crate::document::Parsed) {}\n'
        with stage('kamin-style'):
            result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertTrue(any(blocker['source'] == 'kamin-style'
                            and blocker['target'] == 'kamin-html' for blocker in result.blockers))

    def test_wrapper_requires_a_real_public_lower_glob(self):
        sources = self.sources()
        for body in ('// pub use kamin_style::dom::*;\n',
                     'const S: &str = "pub use kamin_style::dom::*;";\n',
                     'use kamin_style::dom::*;\n',
                     'pub use kamin_style::style::*;\n'):
            sources['document/dom.rs'] = body
            with self.subTest(body=body), self.assertRaisesRegex(ValueError, 'forward its lower root'):
                recover(sources['lib.rs'], sources, ROOT_OWNERS)

    def test_ambiguous_or_undeclared_wrapper_is_rejected(self):
        sources = self.sources()
        sources['document/dom/mod.rs'] = sources['document/dom.rs']
        with self.assertRaisesRegex(ValueError, 'needs one module'):
            recover(sources['lib.rs'], sources, ROOT_OWNERS)
        del sources['document/dom/mod.rs']
        sources['document/mod.rs'] = 'pub struct Parsed;\n'
        with stage('kamin-style'), self.assertRaisesRegex(ValueError, 'Undeclared implementation'):
            plan(sources, {'crates/html/src/' + p for p in sources})

    def test_commented_and_duplicate_aliases_need_explicit_review(self):
        sources = self.sources()
        source = sources['lib.rs'].replace('crate::document', 'crate /* keep */ ::document')
        with self.assertRaisesRegex(ValueError, 'Comment inside facade'):
            recover(source, sources, ROOT_OWNERS)
        with self.assertRaisesRegex(ValueError, 'Duplicate facade'):
            recover(sources['lib.rs'] + 'pub use crate::document::dom;', sources, ROOT_OWNERS)


    def path_sources(self):
        sources = self.sources()
        sources['lib.rs'] = sources['lib.rs'].replace(
            'pub(crate) mod style;',
            '#[path = "document/style/mod.rs"]\npub(crate) mod style;')
        sources['document/style/mod.rs'] = (
            'pub use kamin_style::style::*;\n'
            '#[cfg(test)] mod tests { #[test] fn same_name() {} }\n')
        return sources

    def test_path_wrapper_preserves_identity_without_emitting_virtual_modules(self):
        sources = self.path_sources()
        physical = {name: 'crates/html/src/' + name for name in sources}
        parent = sources['document/mod.rs']
        root = sources['lib.rs']
        with stage('kamin-style'):
            result = plan(sources, set(physical.values()), physical)
            self.assertFalse(result.blockers)
            roots = self.apply(sources, physical, result)
            repeated = plan(sources, set(physical.values()), physical, roots)
        self.assertEqual(repeated.changes, [])
        self.assertEqual(sources['document/mod.rs'], parent)
        self.assertIn('#[path = "document/style/mod.rs"]\npub(crate) mod style;',
                      sources['lib.rs'])
        self.assertNotIn('Virtual ownership', ''.join(sources.values()))
        self.assertEqual(physical['style/mod.rs'], 'crates/html-style/src/style/mod.rs')
        self.assertEqual(physical['document/style/mod.rs'],
                         'crates/html/src/document/style/mod.rs')
        with stage('kamin-layout'):
            layout = plan(sources, set(physical.values()), physical, roots)
            self.assertFalse(layout.blockers)
            roots.update(self.apply(sources, physical, layout))
            self.assertEqual(plan(sources, set(physical.values()), physical, roots).changes, [])

    def test_path_wrapper_still_exposes_forbidden_upward_edges(self):
        sources = self.path_sources()
        sources['style/mod.rs'] += 'pub fn backwards(_: crate::document::Parsed) {}\n'
        with stage('kamin-style'):
            result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertTrue(any(blocker['source'] == 'kamin-style'
                            and blocker['target'] == 'kamin-html' for blocker in result.blockers))

    def test_path_wrapper_rejects_unreviewed_destinations_and_private_globs(self):
        sources = self.path_sources()
        with self.assertRaisesRegex(ValueError, 'Unsupported facade module path'):
            recover(sources['lib.rs'].replace('document/style/mod.rs', 'other/style.rs'),
                    sources, ROOT_OWNERS)
        sources['document/style/mod.rs'] = 'use kamin_style::style::*;\n'
        with self.assertRaisesRegex(ValueError, 'forward its lower root'):
            recover(sources['lib.rs'], sources, ROOT_OWNERS)


if __name__ == '__main__':
    unittest.main()
