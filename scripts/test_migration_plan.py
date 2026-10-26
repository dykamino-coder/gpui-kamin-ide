import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from migration_plan import plan, expose_boundary


class MigrationPlanTests(unittest.TestCase):
    def sources(self):
        return {
            'lib.rs': 'pub mod dom;\npub(crate) mod style;\npub(crate) mod layout;\n'
                      'pub(crate) mod paint;\npub mod render;\n',
            'dom/mod.rs': 'pub struct Node;\n',
            'style/mod.rs': 'pub(crate) struct Style;\n',
            'layout/mod.rs': 'pub(crate) fn shape(_: crate::dom::Node) {}\n',
            'paint/mod.rs': 'pub(crate) fn paint(node: crate::dom::Node) { crate::layout::shape(node); }\n',
            'render/mod.rs': 'pub fn render(node: crate::dom::Node) { crate::paint::paint(node); }\n',
        }

    def test_plan_has_direct_dependencies_and_preserves_facade_visibility(self):
        sources = self.sources()
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertFalse(result.blockers)
        moved = {c.new: c.after for c in result.changes}
        self.assertEqual(moved['crates/html-layout/src/layout/mod.rs'],
                         'pub fn shape(_: kamin_style::dom::Node) {}\n')
        self.assertIn('pub use kamin_style::dom;', moved['crates/html/src/lib.rs'])
        self.assertIn('pub(crate) use kamin_layout::layout;', moved['crates/html/src/lib.rs'])
        self.assertEqual(set(result.edges), {
            ('kamin-layout', 'kamin-style'), ('kamin-paint', 'kamin-style'),
            ('kamin-paint', 'kamin-layout'), ('kamin-html', 'kamin-style'),
            ('kamin-html', 'kamin-paint'), ('kamin-html', 'kamin-layout')})

    def test_lower_roots_retain_compilation_attributes_exactly(self):
        sources = self.sources()
        attributes = ('#![forbid(unsafe_code)]\n'
                      '#![cfg_attr(feature = "test", allow(dead_code))]\n')
        sources['lib.rs'] = attributes + sources['lib.rs']
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        roots = [change.after for change in result.changes
                 if change.new.endswith('/src/lib.rs') and '/html/' not in change.new]
        self.assertEqual(len(roots), 3)
        for source in roots:
            self.assertIn(attributes, source)
            self.assertEqual(source.count('#![forbid(unsafe_code)]'), 1)

    def test_inner_module_attributes_do_not_become_crate_attributes(self):
        from migration_plan import crate_attributes
        source = ('#![forbid(unsafe_code)]\n'
                  'mod child { #![allow(dead_code)] fn f() {} }\n'
                  'const TEXT: &str = r#"#![allow(unsafe_code)]"#;\n')
        self.assertEqual(crate_attributes(source), ['#![forbid(unsafe_code)]'])
        with self.assertRaisesRegex(ValueError, 'Unclosed root crate attribute'):
            crate_attributes('#![cfg(any(test)')

    def test_second_plan_does_not_move_or_rewrite_anything(self):
        sources = self.sources()
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        physical = {name: 'crates/html/src/' + name for name in sources}
        root_sources = {}
        for change in result.changes:
            if change.new.endswith('/src/lib.rs') and not change.new.startswith('crates/html/'):
                package = 'kamin-' + change.new.split('/')[1].removeprefix('html-')
                root_sources[package] = change.after
            else:
                name = next(n for n, p in physical.items() if p == change.old)
                sources[name] = change.after
                physical[name] = change.new
        repeated = plan(sources, set(physical.values()), physical, root_sources)
        self.assertEqual(repeated.changes, [])
        self.assertFalse(repeated.blockers)

    def test_cycle_is_a_blocker_with_exact_source_site(self):
        sources = self.sources()
        sources['style/mod.rs'] += 'pub fn backwards(n: crate::dom::Node) { crate::layout::shape(n); }\n'
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertEqual(result.blockers, [dict(source='kamin-style', target='kamin-layout',
                                               sites=[('style/mod.rs', 2)])])

    def test_unknown_new_root_requires_reviewed_ownership(self):
        sources = self.sources()
        sources['unreviewed.rs'] = 'pub struct X;'
        with self.assertRaisesRegex(ValueError, 'Unassigned roots'):
            plan(sources, set())

    def box_facts_sources(self):
        sources = self.sources()
        sources['lib.rs'] += 'pub(crate) mod box_facts;\n'
        sources['box_facts.rs'] = (
            'pub(crate) struct BoxContext<W, A> { pub parent_fc: bool, pub wm: W, pub align: A }\n'
            'pub(crate) struct BoxStyleInput<W, A> { pub context: Option<BoxContext<W, A>> }\n'
            'pub(crate) struct ApplicabilityInputs { pub is_flex_item: bool }\n'
            "pub(crate) struct DecorationInputs<'a, D> { pub inherited: &'a D }\n")
        sources['dom/mod.rs'] += (
            'pub struct SharedStyle(pub crate::style::BoxStyleInput);\n'
            'impl SharedStyle { pub fn layout_made(input: crate::style::BoxStyleInput) -> Self { Self(input) } }\n')
        sources['style/mod.rs'] += (
            'pub struct WritingMode; pub struct Align;\n'
            'pub type BoxStyleInput = crate::box_facts::BoxStyleInput<WritingMode, Align>;\n'
            'pub fn inherit() -> crate::box_facts::BoxContext<WritingMode, Align> { '
            'crate::box_facts::BoxContext { parent_fc: true, wm: WritingMode, align: Align } }\n')
        sources['layout/mod.rs'] += (
            'pub fn consume(input: crate::style::BoxStyleInput) -> bool { input.context.unwrap().parent_fc }\n')
        return sources

    def test_box_facts_stays_lower_with_style_producer_and_layout_reader(self):
        sources = self.box_facts_sources()
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertFalse(result.blockers)
        moved = {c.new: c.after for c in result.changes}
        self.assertIn('crates/html-style/src/box_facts.rs', moved)
        self.assertIn('impl SharedStyle', moved['crates/html-style/src/dom/mod.rs'])
        self.assertIn('crate::box_facts::BoxContext', moved['crates/html-style/src/style/mod.rs'])
        self.assertIn('kamin_style::style::BoxStyleInput', moved['crates/html-layout/src/layout/mod.rs'])
        self.assertEqual(moved['crates/html-style/src/box_facts.rs'],
                         expose_boundary(sources['box_facts.rs']))
        self.assertNotIn(('kamin-style', 'kamin-layout'), result.edges)

    def test_box_facts_cannot_reference_upper_box_style(self):
        sources = self.box_facts_sources()
        sources['layout/mod.rs'] += 'pub struct BoxStyle;\n'
        sources['box_facts.rs'] += 'pub fn backwards(_: crate::layout::BoxStyle) {}\n'
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertEqual(result.blockers, [dict(source='kamin-style', target='kamin-layout',
                                               sites=[('box_facts.rs', 5)])])

    def test_plan_preserves_exact_input_bytes_but_emits_lf(self):
        sources = {name: text.replace('\n', '\r\n') for name, text in self.sources().items()}
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        for change in result.changes:
            self.assertNotIn('\r', change.after)
            if change.old.startswith('crates/html/src/'):
                self.assertEqual(change.before, sources[change.old.removeprefix('crates/html/src/')])

    def test_visibility_keeps_implementation_details_and_literal_bytes(self):
        before = 'pub(crate) struct A;\nmod implementation {}\npub(super) fn f() {}\n'
        before += 'const S: &str = r#"pub(crate) struct Hidden;"#; // pub(crate)\n'
        after = expose_boundary(before)
        self.assertEqual(after, before.replace('pub(crate) struct A;', 'pub struct A;'))
        restricted = 'pub(in crate::layout) struct Restricted;'
        self.assertEqual(expose_boundary(restricted), restricted)
        with self.assertRaisesRegex(ValueError, 'ancestor escaped'):
            expose_boundary('pub(in kamin_layout::layout) struct Restricted;')
        with self.assertRaises(ValueError):
            expose_boundary('pub(/* keep */ crate) struct Commented;')

    def test_acyclic_foreign_inherent_impl_is_still_blocked(self):
        sources = self.sources()
        sources['paint/mod.rs'] += 'impl crate::dom::Node { pub fn value(&self) {} }\n'
        original = dict(sources)
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertEqual(len(result.blockers), 1)
        self.assertEqual(result.blockers[0]['kind'], 'inherent-foreign-owner')
        self.assertEqual(result.blockers[0]['sites'], [('paint/mod.rs', 2)])
        self.assertEqual(sources, original)

    def test_foreign_trait_and_self_is_blocked_without_upward_edges(self):
        sources = self.sources()
        sources['dom/mod.rs'] += 'pub trait Contract {}\n'
        sources['paint/mod.rs'] += 'impl crate::dom::Contract for crate::dom::Node {}\n'
        result = plan(sources, {'crates/html/src/' + p for p in sources})
        self.assertEqual([r['kind'] for r in result.blockers], ['trait-and-self-foreign-owner'])


if __name__ == '__main__':
    unittest.main()
