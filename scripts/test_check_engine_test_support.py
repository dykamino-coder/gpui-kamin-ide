"""Regressions for the narrow dev-only lower HTML helper contract."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from check_engine_test_support import check, graph_errors, manifest_errors


def document(owner, **entries):
    return {'package': {'name': owner}, **entries}


class ManifestContract(unittest.TestCase):
    def test_only_upper_dev_dependency_can_activate_support(self):
        allowed = {'features': {'engine-test-support': ['gpui/test-support']}}
        documents = {
            'lower': document('kamin-layout', **allowed),
            'upper': document('kamin-html', **{'dev-dependencies': {
                'layout': {'package': 'kamin-layout', 'features': ['engine-test-support']}}})}
        self.assertEqual(manifest_errors(documents), [])
        for owner, kind in [('kamin-html', 'dependencies'),
                            ('kamin-html', 'build-dependencies'),
                            ('kamin-paint', 'dev-dependencies')]:
            with self.subTest(owner=owner, kind=kind):
                invalid = {'upper': document(owner, **{kind: {
                    'kamin-layout': {'optional': True, 'features': ['engine-test-support']}}})}
                self.assertTrue(manifest_errors(invalid))

    def test_default_aliases_and_feature_forwarding_are_rejected(self):
        for expression in ('engine-test-support', 'kamin-layout/engine-test-support',
                           'kamin-layout?/engine-test-support'):
            self.assertTrue(manifest_errors({'lower': document('kamin-layout',
                features={'default': [expression]})}))
        self.assertTrue(manifest_errors({'upper': document('kamin-html',
            features={'engine-test-support': []})}))

    def test_target_and_workspace_dependency_tables_are_checked(self):
        dependency = {'kamin-layout': {'features': ['engine-test-support']}}
        for data in ({'target': {'cfg(windows)': {'dependencies': dependency}}},
                     {'workspace': {'dependencies': dependency}}):
            self.assertTrue(manifest_errors({'Cargo.toml': document('kamin-html', **data)}))

    def test_resolved_root_features_and_feature_edges_are_checked(self):
        self.assertTrue(graph_errors('kamin-layout v1.0|default,engine-test-support'))
        self.assertTrue(graph_errors('kamin-layout feature "engine-test-support"'))
        self.assertFalse(graph_errors('engine-test-support-tool v1.0|default\n'
                                      'kamin-layout v1.0|not-engine-test-support'))


@unittest.skipUnless(shutil.which('cargo'), 'Cargo is required for resolved graph regression')
class ResolvedContract(unittest.TestCase):
    def test_dev_feature_does_not_leak_but_normal_activation_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            # Graph resolution queries rustc; isolate this fixture from any
            # machine-wide compiler cache without building or changing a profile.
            (root / '.cargo').mkdir()
            (root / '.cargo/config.toml').write_text(
                '[build]\nrustc-wrapper = ""\n', encoding='utf-8')
            (root / 'Cargo.toml').write_text(
                '[workspace]\nresolver = "2"\nmembers = ["upper", "lower"]\n', encoding='utf-8')
            for name in ('upper', 'lower'):
                (root / name / 'src').mkdir(parents=True)
                (root / name / 'src/lib.rs').write_text('', encoding='utf-8')
            (root / 'lower/Cargo.toml').write_text('''[package]
name = "kamin-layout"
version = "0.0.0"
edition = "2021"
[features]
engine-test-support = []
''', encoding='utf-8')
            manifest = root / 'upper/Cargo.toml'
            source = '''[package]
name = "kamin-html"
version = "0.0.0"
edition = "2021"
[dependencies]
kamin-layout = { path = "../lower" }
[dev-dependencies]
kamin-layout = { path = "../lower", features = ["engine-test-support"] }
'''
            manifest.write_text(source, encoding='utf-8')
            subprocess.run(['cargo', 'generate-lockfile', '--offline'], cwd=root,
                           check=True, capture_output=True)
            self.assertEqual(check(root), [])
            dev = subprocess.check_output(['cargo', 'tree', '--workspace', '--edges',
                'normal,dev,features', '--prefix', 'none', '--format', '{p}|{f}',
                '--offline', '--locked'], cwd=root, text=True)
            self.assertTrue(graph_errors(dev), 'fixture must actually enable dev helpers')
            manifest.write_text(source.replace('path = "../lower" }',
                'path = "../lower", features = ["engine-test-support"] }'), encoding='utf-8')
            self.assertTrue(any('normal Cargo graph' in error for error in check(root)))


if __name__ == '__main__':
    unittest.main()
