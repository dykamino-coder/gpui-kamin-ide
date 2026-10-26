"""Only the approved lower-crate test-support cfg can own test-only debt."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from computed_access import aggregate

PREFIX = 'crates/html/src/'
FIELDS = 'pub struct Computed {\n    pub width: f32,\n}\n'


def counts(code, extras=None):
    return aggregate({PREFIX + 'style/computed/fields.rs': FIELDS,
                      PREFIX + 'layout/mod.rs': code, **(extras or {})})


class TestSupportScopes(unittest.TestCase):
    def test_exact_support_cfg_is_test_owned_with_whitespace_and_comments(self):
        result = counts('''#[cfg(any(test, /* exact feature */
            feature = "engine-test-support"))]
            mod cases { fn run() { s.width = 1; } }
            s.width = 2;''')
        self.assertEqual(result['Computed.width', 'layout:test', 'write'], 1)
        self.assertEqual(result['Computed.width', 'layout', 'write'], 1)

    def test_external_module_descendants_inherit_test_scope(self):
        result = counts('#[cfg(any(test, feature = "engine-test-support"))] mod cases;', {
            PREFIX + 'layout/cases.rs': 's.width = 1; mod nested;',
            PREFIX + 'layout/cases/nested.rs': 's.width = 2;'})
        self.assertEqual(result['Computed.width', 'layout:test', 'write'], 2)
        self.assertEqual(result['Computed.width', 'layout', 'write'], 0)

    def test_other_cfg_contracts_remain_production(self):
        for expression in ('any(test, feature = "other")',
                           'feature = "engine-test-support"',
                           'any(feature = "engine-test-support", test)',
                           'all(test, feature = "engine-test-support")',
                           'any(test, feature = "engine-test-support", unix)',
                           'any(test, feature = r"engine-test-support")'):
            with self.subTest(expression=expression):
                result = counts(f'#[cfg({expression})] mod cases {{ s.width = 1; }}')
                self.assertEqual(result['Computed.width', 'layout', 'write'], 1)
                self.assertEqual(result['Computed.width', 'layout:test', 'write'], 0)

    def test_comment_or_literal_cannot_create_a_scope(self):
        result = counts('''/* #[cfg(any(test, feature = "engine-test-support"))] */
            let text = r##"#[cfg(any(test, feature = "engine-test-support"))]"##;
            mod cases { s.width = 1; }''')
        self.assertEqual(result['Computed.width', 'layout', 'write'], 1)

    def test_custom_path_cannot_exempt_an_unrelated_default_path(self):
        result = counts('''#[path = "other.rs"]
            #[cfg(any(test, feature = "engine-test-support"))] mod cases;''', {
                PREFIX + 'layout/cases.rs': 's.width = 1;'})
        self.assertEqual(result['Computed.width', 'layout', 'write'], 1)

    def test_existing_cfg_test_ownership_and_literal_accounting_are_unchanged(self):
        result = counts('''#[cfg(test)] mod cases { s.width = 1; }
            let a = "s.width = 2"; let b = r#"used_mut()"#;
            s.width = 3;''')
        self.assertEqual(result['Computed.width', 'layout:test', 'write'], 1)
        self.assertEqual(result['Computed.width', 'layout', 'write'], 1)


if __name__ == '__main__':
    unittest.main()
