import sys
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from migration_manifests import generate, original_contract

ORIGINAL = '''[package]
name = "kamin-html"
version.workspace = true
edition.workspace = true
[features]
wpt-harness = []
[[example]]
name = "wptrun"
required-features = ["wpt-harness"]
[dependencies]
gpui.workspace = true
xml-rs = "0.8.29"
html5ever = {path = "../../vendor/html5ever"}
image = {version = "0.25", default-features = false, features = ["png"]}
[dev-dependencies]
gpui = {workspace = true, features = ["test-support"]}
taffy = "0.14.0"
[lints]
workspace = true
'''


class ManifestTests(unittest.TestCase):
    def build(self):
        return generate(ORIGINAL, {
            'kamin-style': ['use html5ever::Parser; use xml::reader;'],
            'kamin-layout': ['use gpui::Div; #[test] fn case() { let _: taffy::NodeId; }'],
            'kamin-paint': ['use image::Rgba;'],
            'kamin-html': ['gpui::div();']}, {
                ('kamin-html', 'kamin-paint'): [],
                ('kamin-paint', 'kamin-layout'): [],
                ('kamin-layout', 'kamin-style'): []})

    def test_preserve_exact_external_contracts_and_direct_internal_edges(self):
        manifests = self.build()
        style = tomllib.loads(manifests['crates/html-style/Cargo.toml'])
        self.assertEqual(style['dependencies'], {
            'xml-rs': '0.8.29', 'html5ever': {'path': '../../vendor/html5ever'}})
        layout = tomllib.loads(manifests['crates/html-layout/Cargo.toml'])
        self.assertEqual(layout['dependencies']['gpui'], {'workspace': True})
        self.assertEqual(layout['dependencies']['kamin-style'], {'path': '../html-style'})
        self.assertEqual(layout['dev-dependencies']['taffy'], '0.14.0')
        paint = tomllib.loads(manifests['crates/html-paint/Cargo.toml'])
        self.assertEqual(paint['dependencies']['image'], {
            'version': '0.25', 'default-features': False, 'features': ['png']})
        self.assertNotIn('example', layout)

    def test_features_forward_without_cyclic_dependency_or_duplicate_types(self):
        manifests = self.build()
        layout = tomllib.loads(manifests['crates/html-layout/Cargo.toml'])
        self.assertEqual(layout['features']['wpt-harness'], ['kamin-style/wpt-harness'])
        self.assertIn('gpui/test-support', layout['features']['engine-test-support'])
        self.assertEqual(layout['features']['engine-test-support'], ['gpui/test-support', 'dep:taffy'])
        self.assertEqual(layout['dependencies']['taffy'], {'version': '0.14.0', 'optional': True})
        upper = tomllib.loads(manifests['crates/html/Cargo.toml'])
        self.assertNotIn('engine-test-support', upper['features'])
        self.assertEqual(upper['dev-dependencies']['kamin-paint'], {
            'path': '../html-paint', 'features': ['engine-test-support']})
        self.assertEqual(upper['example'][0]['name'], 'wptrun')
        self.assertEqual(set(upper['dev-dependencies']) & {
            'kamin-font-facts', 'kamin-style', 'kamin-layout', 'kamin-paint'},
            {'kamin-font-facts', 'kamin-style', 'kamin-layout', 'kamin-paint'})

    def test_font_contract_has_no_upper_dependencies(self):
        manifests = generate(ORIGINAL, {
            'kamin-font-facts': ['pub trait Provider {}'],
            'kamin-style': ['use kamin_font_facts::Provider;']}, {
                ('kamin-style', 'kamin-font-facts'): []})
        fonts = tomllib.loads(manifests['crates/font-facts/Cargo.toml'])
        style = tomllib.loads(manifests['crates/html-style/Cargo.toml'])
        self.assertEqual(fonts['dependencies'], {})
        self.assertEqual(fonts['dev-dependencies'], {})
        self.assertEqual(style['dependencies'], {'kamin-font-facts': {'path': '../font-facts'}})

    def test_support_feature_never_uses_normal_forwarding(self):
        manifests = self.build()
        sys.path.insert(0, str(Path(str(Path(__file__).resolve().parent))))
        from check_engine_test_support import manifest_errors
        documents = {path: tomllib.loads(text) for path, text in manifests.items()}
        self.assertEqual(manifest_errors(documents), [])

    def test_repeated_generation_is_identical(self):
        self.assertEqual(self.build(), self.build())

    def test_recover_contract_after_migration_without_a_saved_snapshot(self):
        first = self.build()
        recovered = tomllib.loads(original_contract(first))
        original = tomllib.loads(ORIGINAL)
        self.assertEqual(recovered, original)
        self.assertEqual(tomllib.loads(original_contract({'crates/html/Cargo.toml': ORIGINAL})), original)

    def test_conflicting_dependency_versions_are_rejected(self):
        documents = self.build()
        documents['crates/html-style/Cargo.toml'] += '\n[dev-dependencies.gpui]\nversion = "999"\n'
        with self.assertRaisesRegex(ValueError, 'Conflicting external'):
            original_contract(documents)

    def test_generated_optional_copy_preserves_every_dev_dependency_option(self):
        source = ORIGINAL.replace('taffy = "0.14.0"',
            'taffy = {path = "../../vendor/taffy", default-features = false, features = ["std"]}')
        manifests = generate(source, {'kamin-layout': ['taffy::NodeId'],
            'kamin-html': ['gpui::Div; xml::reader; html5ever::Parser; image::Rgba;']}, {})
        layout = tomllib.loads(manifests['crates/html-layout/Cargo.toml'])
        self.assertEqual(layout['dependencies']['taffy'], {
            'path': '../../vendor/taffy', 'default-features': False,
            'features': ['std'], 'optional': True})
        self.assertEqual(tomllib.loads(original_contract(manifests)),
                         tomllib.loads(source))

    def test_generated_test_dependency_mutation_cannot_hide_from_recovery(self):
        manifests = self.build()
        name = 'crates/html-layout/Cargo.toml'
        manifests[name] = manifests[name].replace(
            'taffy = { optional = true, version = "0.14.0" }',
            'taffy = { optional = true, version = "999" }')
        with self.assertRaisesRegex(ValueError, 'Conflicting external test dependency'):
            original_contract(manifests)

    def test_upper_support_feature_rejects_instead_of_normalizing_it_away(self):
        invalid = ORIGINAL.replace('[features]', '[features]\nengine-test-support = []')
        with self.assertRaisesRegex(ValueError, 'must not define'):
            generate(invalid, {}, {})
        with self.assertRaisesRegex(ValueError, 'Inconsistent phase features'):
            original_contract({'crates/html/Cargo.toml': invalid})

    def test_original_optional_dependency_cannot_be_confused_with_a_generated_copy(self):
        source = ORIGINAL.replace('[dependencies]',
            '[dependencies]\ntaffy = {version = "0.14.0", optional = true}')
        with self.assertRaisesRegex(ValueError, 'explicit recovery policy'):
            generate(source, {}, {})

    @unittest.skipUnless(shutil.which('cargo'), 'Cargo is required for boundary regression')
    def test_generated_test_helpers_compile_without_normal_feature_activation(self):
        from check_engine_test_support import check, graph_errors
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def write(name, text):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text, encoding='utf-8')

            write('.cargo/config.toml', '[build]\nrustc-wrapper = ""\n'
                  '[target.x86_64-pc-windows-msvc]\nlinker = "rust-lld"\n')
            write('Cargo.toml', '[workspace]\nresolver = "2"\nmembers = ["crates/*", "vendor/taffy"]\n'
                  '[workspace.package]\nversion = "0.0.0"\nedition = "2021"\n'
                  '[workspace.lints.rust]\nunsafe_code = "forbid"\n')
            write('vendor/taffy/Cargo.toml', '[package]\nname = "taffy"\nversion = "0.0.0"\nedition = "2021"\n')
            write('vendor/taffy/src/lib.rs', 'pub fn identity(x: u32) -> u32 { x }\n')
            original = ('[package]\nname = "kamin-html"\nversion.workspace = true\nedition.workspace = true\n'
                        '[features]\nwpt-harness = []\n[dependencies]\n[dev-dependencies]\n'
                        'taffy = {path = "../../vendor/taffy"}\n[lints]\nworkspace = true\n')
            sources = {
                'kamin-font-facts': ['#![deny(warnings)]\n'],
                'kamin-style': ['#![deny(warnings)]\npub struct Style(pub u32);\n'],
                'kamin-layout': ['#![deny(warnings)]\n'
                    'pub fn identity(s: kamin_style::Style) -> kamin_style::Style { s }\n'
                    '#[cfg(any(test, feature = "engine-test-support"))]\n'
                    'pub fn helper(s: kamin_style::Style) -> kamin_style::Style {\n'
                    '    kamin_style::Style(taffy::identity(s.0))\n}\n'],
                'kamin-paint': ['#![deny(warnings)]\n'],
                'kamin-html': ['#![deny(warnings)]\n'
                    'pub fn ordinary(s: kamin_style::Style) -> kamin_style::Style { kamin_layout::identity(s) }\n'
                    '#[cfg(test)] mod tests { #[test] fn real_shared_identity() {\n'
                    ' let s = super::ordinary(kamin_style::Style(42));\n'
                    ' assert_eq!(kamin_layout::helper(s).0, 42);\n} }\n']}
            edges = {('kamin-html', 'kamin-layout'): [], ('kamin-html', 'kamin-style'): [],
                     ('kamin-layout', 'kamin-style'): []}
            manifests = generate(original, sources, edges)
            for name, text in manifests.items():
                write(name, text)
                package = tomllib.loads(text)['package']['name']
                write(str(Path(name).parent / 'src/lib.rs'), sources[package][0])
            def cargo(*args):
                try:
                    return subprocess.run(['cargo', *args, '--offline'], cwd=root, text=True,
                        capture_output=True, check=True, timeout=90)
                except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
                    self.fail(f'{args}: {error}\n{error.stdout}\n{error.stderr}')
            cargo('generate-lockfile')
            self.assertEqual(check(root), [])
            cargo('check', '-p', 'kamin-html', '--lib', '--locked', '-j', '1')
            result = cargo('test', '-p', 'kamin-html', '--lib', '--locked', '-j', '1')
            self.assertIn('1 passed; 0 failed', result.stdout)
            dev = cargo('tree', '--workspace', '--edges', 'normal,dev,features',
                        '--prefix', 'none', '--format', '{p}|{f}', '--locked')
            self.assertTrue(graph_errors(dev.stdout))
            self.assertEqual(tomllib.loads(original_contract(manifests)), tomllib.loads(original))

    def test_unreviewed_features_and_manifest_sections_and_cycles_reject(self):
        with self.assertRaisesRegex(ValueError, 'Feature'):
            generate(ORIGINAL.replace('wpt-harness = []', 'wpt-harness = ["image/png"]'), {}, {})
        with self.assertRaisesRegex(ValueError, 'sections'):
            generate(ORIGINAL + '\n[build-dependencies]\ncc = "1"\n', {}, {})
        with self.assertRaisesRegex(ValueError, 'direction'):
            generate(ORIGINAL, {}, {('kamin-style', 'kamin-layout'): []})


if __name__ == '__main__':
    unittest.main()
