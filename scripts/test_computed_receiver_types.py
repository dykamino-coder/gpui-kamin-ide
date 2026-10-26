"""Synthetic receiver fixtures; no real engine data."""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from computed_access import aggregate, DEFINITIONS, PREFIX


def counts(source, extra=None):
    files = {DEFINITIONS: 'pub struct Computed {\n    pub width: f32,\n}\npub struct InheritedStyle {\n    pub color: f32,\n}\n',
             PREFIX+'dom/sample.rs': source}
    if extra:
        files.update(extra)
    return aggregate(files)


def escapes(source, name='borrow_mut', extra=None):
    return counts(source, extra)['@'+name, 'dom', 'escape']


class ReceiverTests(unittest.TestCase):
    def test_nominal_payloads_remain_guarded(self):
        for declaration in ('struct Payload { style: Computed }',
                            'struct Payload(Computed);',
                            'enum Payload { Style(Computed) }',
                            'struct Inner { style: Computed } struct Payload { inner: Inner }',
                            'struct Payload { state: External }'):
            for wrapper, api, call in (('RefCell', 'borrow_mut', 'x.borrow_mut()'),
                                       ('RefCell', 'get_mut', 'x.get_mut()'),
                                       ('Rc', 'make_mut', 'Rc::make_mut(&mut x)')):
                source = declaration + f' fn f(mut x: {wrapper}<Payload>) {{ {call}; }}'
                with self.subTest(source=source):
                    self.assertEqual(1, escapes(source, api))

    def test_trait_default_and_blanket_methods_remain_guarded(self):
        for implementation in ('impl Styles for Cache {}', 'impl<T> Styles for T {}'):
            code = 'trait Styles { fn borrow_mut(&self) -> RefMut<Computed> { source() } } struct Cache; ' + implementation + ' fn f(x: Cache) { x.borrow_mut(); }'
            self.assertEqual(1, escapes(code))

    def test_unknown_ufcs_provider_remains_guarded(self):
        for api in ('borrow_mut', 'get_mut', 'make_mut'):
            self.assertEqual(1, escapes(f'fn f(x: Rc<Vec<u8>>) {{ Provider::{api}(&x); }}', api))

    def test_qualified_unknown_ufcs_provider_remains_guarded(self):
        for provider, api in (('Rc', 'make_mut'), ('Arc', 'get_mut'), ('RefCell', 'borrow_mut')):
            self.assertEqual(1, escapes(f'fn f(mut x: {provider}<Vec<u8>>) {{ external::{provider}::{api}(&mut x); }}', api))
            module = {'Rc': 'rc', 'Arc': 'sync', 'RefCell': 'cell'}[provider]
            self.assertEqual(0, escapes(f'fn f(mut x: {provider}<Vec<u8>>) {{ std::{module}::{provider}::{api}(&mut x); }}', api))

    def test_standard_import_alias_cannot_spoof_inherent_provider(self):
        source = 'use std::cell::RefCell as Rc; trait Styles { fn make_mut(this: &mut Self) -> &mut Computed { panic!() } } impl Styles for Rc<Vec<u8>> {} fn f(mut x: Rc<Vec<u8>>) { Rc::make_mut(&mut x); }'
        self.assertEqual(1, escapes(source, 'make_mut'))

    def test_shadowed_standard_name_remains_guarded(self):
        self.assertEqual(1, escapes('struct RefCell<T>(T); trait Styles { fn borrow_mut(&self) -> RefMut<Computed> { source() } } impl<T> Styles for RefCell<T> {} fn f(x: RefCell<u8>) { x.borrow_mut(); }'))

    def test_explicit_non_style_parameters(self):
        for kind in ('u8', 'Vec<u8>', 'String'):
            for api in ('borrow_mut', 'get_mut', 'make_mut'):
                wrapper = 'Rc' if api == 'make_mut' else 'RefCell'
                call = f'Rc::{api}(&mut x)' if api == 'make_mut' else f'x.{api}()'
                with self.subTest(kind=kind, api=api):
                    self.assertEqual(0, escapes(f'fn f(mut x: {wrapper}<{kind}>) {{ {call}; }}', api))

    def test_all_style_receivers_remain_guarded(self):
        for kind in ('Computed', 'InheritedStyle', 'SharedStyle', 'BoxStyle'):
            for api in ('borrow_mut', 'get_mut', 'make_mut'):
                wrapper = 'Rc' if api == 'make_mut' else 'RefCell'
                call = f'Rc::{api}(&mut x)' if api == 'make_mut' else f'x.{api}()'
                with self.subTest(kind=kind, api=api):
                    self.assertEqual(1, escapes(f'fn f(mut x: {wrapper}<{kind}>) {{ {call}; }}', api))

    def test_style_alias_and_import(self):
        self.assertEqual(1, escapes('type C = Computed; type S = Rc<C>; fn f(x: S) { x.borrow_mut(); }'))
        self.assertEqual(1, escapes('use crate::style::Computed as C; fn f(x: RefCell<C>) { x.borrow_mut(); }'))

    def test_unresolved_receivers_fail_closed(self):
        for text in ('fn f(x: External) { x.borrow_mut(); }',
                     'fn f() { let x = source(); x.borrow_mut(); }',
                     'fn f(x: Vec<u8>) { x[index()].borrow_mut(); }',
                     'fn f(x: Vec<u8>) { x.style().borrow_mut(); }',
                     'fn f(x: S::Output) { x.borrow_mut(); }'):
            with self.subTest(text=text):
                self.assertEqual(1, escapes(text))

    def test_shadowed_and_generic_bindings_fail_closed(self):
        for text in ('fn f(x: Vec<u8>) { let x = style(); x.borrow_mut(); }',
                     'fn f(x: Vec<u8>) { match v { Some(x) => x.borrow_mut(), _ => {} } }',
                     'struct Node; fn f<Node>(x: Rc<Node>) { x.borrow_mut(); }',
                     'fn f(x: Vec<u8>) { for x in styles { x.borrow_mut(); } }',
                     'fn f(x: Vec<u8>) { let closure = |x| x.borrow_mut(); }',
                     'fn f(x: Vec<u8>) { let closure = async |x| x.borrow_mut(); }',
                     'fn f(x: Vec<u8>) { return |x| x.borrow_mut(); }',
                     'fn f(x: Vec<u8>) {} fn g((x,): (RefCell<Computed>,)) { x.borrow_mut(); }'):
            with self.subTest(text=text):
                self.assertEqual(1, escapes(text))

    def test_named_and_tuple_field_receivers(self):
        self.assertEqual(0, escapes('struct State { cache: RefCell<Vec<u8>> } impl State { fn f(&self) { self.cache.borrow_mut(); } }'))
        self.assertEqual(0, escapes('struct State(Rc<Vec<u8>>); impl State { fn f(&mut self) { Rc::make_mut(&mut self.0); } }', 'make_mut'))
        self.assertEqual(1, escapes('struct State { style: RefCell<Computed> } impl State { fn f(&self) { self.style.borrow_mut(); } }'))

    def test_ambiguous_same_name_field_is_guarded(self):
        extra = {PREFIX+'dom/other.rs': 'struct State { cache: RefCell<Computed> }'}
        self.assertEqual(1, escapes('struct State { cache: RefCell<Vec<u8>> } impl State { fn f(&self) { self.cache.borrow_mut(); } }', extra=extra))

    def test_standard_imports_do_not_hide_proven_non_style_receivers(self):
        self.assertEqual(0, escapes('use std::cell::RefCell; struct Cache; fn f(x: RefCell<Cache>) { x.borrow_mut(); }'))

    def test_foreign_nominals_and_inline_modules_cannot_prove_receivers(self):
        extra = {PREFIX+'resources/other.rs': 'struct Cache; type C = u8;'}
        for receiver in ('Cache', 'C'):
            with self.subTest(receiver=receiver):
                self.assertEqual(1, escapes(f'use external::*; fn f(x: RefCell<{receiver}>) {{ x.borrow_mut(); }}', extra=extra))
        self.assertEqual(1, escapes('use external::*; fn f(x: RefCell<Vec<u8>>) { x.borrow_mut(); }'))
        self.assertEqual(1, escapes('mod unrelated { struct Cache; } mod consumer { use external::*; fn f(x: RefCell<Cache>) { x.borrow_mut(); } }'))
        self.assertEqual(1, escapes('mod unrelated { struct Cache; } fn f(x: RefCell<crate::dom::sample::Cache>) { x.borrow_mut(); }'))

    def test_local_items_and_associated_aliases_do_not_prove_foreign_types(self):
        for text in (
            'fn unrelated() { struct Cache; } use external::*; fn f(x: RefCell<Cache>) { x.borrow_mut(); }',
            'fn unrelated() { type C = u8; } use external::*; fn f(x: RefCell<C>) { x.borrow_mut(); }',
            'struct Host; impl Trait for Host { type C = u8; } use external::*; fn f(x: RefCell<C>) { x.borrow_mut(); }',
        ):
            with self.subTest(text=text): self.assertEqual(1, escapes(text))

    def test_custom_escape_methods_remain_guarded(self):
        for api in ('borrow_mut', 'get_mut', 'make_mut'):
            text = f'struct Cache; impl Cache {{ fn {api}(&self) -> RefMut<Computed> {{ source() }} }} fn f(x: Cache) {{ x.{api}(); }}'
            with self.subTest(api=api): self.assertEqual(1, escapes(text, api))

    def test_group_import_aliases_and_renamed_deref_remain_guarded(self):
        self.assertEqual(1, escapes('struct C; use external::{Computed, Style as C}; fn f(x: C) { x.borrow_mut(); }'))
        self.assertEqual(1, escapes('struct Node; use std::ops::Deref as D; impl D for Node { type Target = RefCell<Computed>; } fn f(x: Node) { x.borrow_mut(); }'))

    def test_opaque_import_and_deref_are_guarded(self):
        fixtures = (
            'struct Node; use opaque::StyleState as Node; fn f(x: Node) { x.borrow_mut(); }',
            'struct Node; impl Deref for Node { type Target = RefCell<Computed>; } fn f(x: Node) { x.borrow_mut(); }',
            'struct Node; impl BorrowMut<Computed> for Node {} fn f(x: Node) { x.borrow_mut(); }',
        )
        for text in fixtures:
            with self.subTest(text=text):
                self.assertEqual(1, escapes(text))

    def test_qualified_nominals_and_lifetime_projections(self):
        code = "struct Ctx<'a> { cache: &'a Cache } struct Cache { values: RefCell<Vec<u8>> } fn f(cx: Ctx<'_>) { cx.cache.values.borrow_mut(); }"
        self.assertEqual(0, escapes(code))
        extra = {PREFIX+'resources/cache.rs': 'pub struct Cache;'}
        self.assertEqual(0, escapes('fn f(x: RefCell<crate::resources::cache::Cache>) { x.borrow_mut(); }', extra=extra))
        self.assertEqual(1, escapes('fn f(x: RefCell<other::Cache>) { x.borrow_mut(); }', extra=extra))

    def test_removed_non_style_escape_cannot_fund_style_growth(self):
        from computed_access import growth
        from check_computed import owner_changes, transferred_growth
        for api in ('borrow_mut', 'get_mut', 'make_mut'):
            wrapper = 'Rc' if api == 'make_mut' else 'RefCell'
            call = 'Rc::make_mut(&mut x)' if api == 'make_mut' else f'x.{api}()'
            old = counts(f'fn f(mut x: {wrapper}<Vec<u8>>) {{ {call}; }}')
            new = counts('', {PREFIX+'paint/new.rs': f'fn f(mut x: {wrapper}<Computed>) {{ {call}; }}'})
            self.assertTrue(growth(new, old, 'base source counters'))
            self.assertTrue(transferred_growth(new, old, 'fixture', owner_changes(new, old))[0])

    def test_old_blanket_ceiling_cannot_hide_new_style_escape(self):
        import tempfile
        from check_computed import check, documents
        from test_check_transitional import commit, git
        for api in ('borrow_mut', 'get_mut', 'make_mut'):
            with self.subTest(api=api), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                git(root, 'init', '-q')
                wrapper = 'Rc' if api == 'make_mut' else 'RefCell'
                call = 'Rc::make_mut(&mut x)' if api == 'make_mut' else f'x.{api}()'
                source = f'fn f(mut x: {wrapper}<Vec<u8>>) {{ {call}; read(s.width); }}'
                files = {DEFINITIONS: 'pub struct Computed {\n    pub width: f32,\n}\n',
                         PREFIX+'dom/sample.rs': source}
                for name, data in files.items():
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(data, encoding='utf-8')
                directory = root / 'scripts/computed_manifest'
                directory.mkdir(parents=True)
                old = aggregate(files)
                old['@'+api, 'dom', 'escape'] = 1
                for name, data in documents(old).items():
                    (directory/name).write_text(data, encoding='utf-8')
                base = commit(root)
                (root/(PREFIX+'dom/sample.rs')).write_text(
                    source.replace(f'{wrapper}<Vec<u8>>', f'{wrapper}<Computed>'), encoding='utf-8')
                original = {p.name: p.read_bytes() for p in directory.glob('access-*.json')}
                errors, _ = check(root, base, write=True)
                self.assertTrue(any('base source counters' in error for error in errors), errors)
                self.assertEqual(original, {p.name: p.read_bytes()
                                            for p in directory.glob('access-*.json')})

    def test_used_api_and_field_writes_unchanged(self):
        result = counts('fn f(x: Vec<u8>, s: &mut Computed) { x.used_mut(); s.width = 1.; }')
        self.assertEqual(1, result['@used_mut', 'dom', 'escape'])
        self.assertEqual(1, result['Computed.width', 'dom', 'write'])
        self.assertEqual(1, result['@mut-Computed', 'dom', 'escape'])


if __name__ == '__main__':
    unittest.main()
