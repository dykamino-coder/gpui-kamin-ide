"""Checked-in dependency proofs must not depend on Cargo or its local cache."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from computed_cargo_proofs import external_crates, locked_metadata, unrelated_crates


def package(name, dependencies=(), version='1.0.0', source=None):
    text = '\n[[package]]\nname = '+json.dumps(name)+'\nversion = '+json.dumps(version)+'\n'
    if source: text += 'source = '+json.dumps(source)+'\n'
    return text+'dependencies = '+json.dumps(list(dependencies))+'\n'


class LockedGraphTests(unittest.TestCase):
    def graph(self, manifest, packages, workspace=''):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'crates/html').mkdir(parents=True)
            (root / 'Cargo.toml').write_text('[workspace]\n'+workspace, encoding='utf-8')
            (root / 'crates/html/Cargo.toml').write_text(
                '[package]\nname = "kamin-html"\nversion = "1.0.0"\n'+manifest,
                encoding='utf-8')
            (root / 'Cargo.lock').write_text('version = 4\n'+packages, encoding='utf-8')
            return unrelated_crates(locked_metadata(root))

    def test_workspace_rename_and_transitive_dependency_direction(self):
        manifest = '[dependencies]\nui.workspace = true\nback = "1"\nincomplete = "1"\n'
        workspace = '[workspace.dependencies]\nui = { package = "paint-api", version = "1" }\n'
        packages = (package('kamin-html', ['paint-api', 'back', 'incomplete'])+
                    package('paint-api', ['leaf'])+package('leaf')+
                    package('back', ['bridge'])+package('bridge', ['kamin-html'])+
                    package('incomplete', ['missing']))
        self.assertEqual({'ui'}, self.graph(manifest, packages, workspace))

    def test_all_targets_kinds_optional_dependencies_and_alias_variants(self):
        manifest = '''[dependencies]
plain = { version = "1", optional = true }
shared = { package = "safe", version = "1" }
[dev-dependencies]
test-api = "1"
[build-dependencies]
build-api = "1"
[target.'cfg(windows)'.dependencies]
shared = { package = "back", version = "1" }
platform-api = "1"
[target.'cfg(unix)'.dev-dependencies]
unix-test = "1"
'''
        names = ['plain', 'safe', 'back', 'test-api', 'build-api', 'platform-api', 'unix-test']
        packages = package('kamin-html', names)+''.join(
            package(name, ['kamin-html'] if name == 'back' else []) for name in names)
        self.assertEqual({'plain', 'test_api', 'build_api', 'platform_api', 'unix_test'},
                         self.graph(manifest, packages))

    def test_lock_versions_and_sources_distinguish_path_from_registry(self):
        registry = 'registry+https://example.invalid/index'
        manifest = '[dependencies]\nui = { package = "paint-api", version = "1" }\n'
        packages = (package('kamin-html', ['paint-api 1.0.0'])+
                    package('paint-api')+
                    package('paint-api', ['kamin-html'], source=registry)+
                    package('paint-api', ['kamin-html'], version='2.0.0', source=registry))
        self.assertEqual({'ui'}, self.graph(manifest, packages))
        packages = (package('kamin-html', ['paint-api 1.0.0 ('+registry+')'])+
                    package('paint-api', ['kamin-html'])+
                    package('paint-api', source=registry))
        self.assertEqual({'ui'}, self.graph(manifest, packages))

    def test_missing_ambiguous_and_uninherited_dependencies_stay_guarded(self):
        manifest = '[dependencies]\nambiguous = "1"\nabsent = "1"\ninherited.workspace = true\n'
        packages = (package('kamin-html', ['ambiguous', 'absent', 'inherited'])+
                    package('ambiguous', version='1.0.0', source='registry+one')+
                    package('ambiguous', version='2.0.0', source='registry+one')+
                    package('ambiguous', version='3.0.0')+
                    package('inherited'))
        self.assertEqual(set(), self.graph(manifest, packages))

    def test_unrelated_recursive_dependency_graph_terminates(self):
        manifest = '[dependencies]\na = "1"\n'
        packages = package('kamin-html', ['a'])+package('a', ['b'])+package('b', ['a'])
        self.assertEqual({'a'}, self.graph(manifest, packages))

    def test_real_repository_proofs_ignore_path_cargo_home_and_subprocesses(self):
        external_crates.cache_clear()
        expected = external_crates()
        self.assertIn('gpui', expected)
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(os.environ, {'PATH': '', 'CARGO_HOME': directory, 'RUSTUP_HOME': directory}):
                with patch('subprocess.run', side_effect=AssertionError('must not execute Cargo')):
                    external_crates.cache_clear()
                    self.assertEqual(expected, external_crates())
        external_crates.cache_clear()


if __name__ == '__main__':
    unittest.main()
