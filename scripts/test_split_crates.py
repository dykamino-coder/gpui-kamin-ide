import contextlib
import io
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from split_crates import main
from test_migration_plan import MigrationPlanTests
from migration_guards import GUARDS

OUT = Path(__file__).resolve().parent
MANIFEST = '''[package]
name = "kamin-html"
version.workspace = true
edition.workspace = true
[features]
wpt-harness = []
[dependencies]
[dev-dependencies]
[lints]
workspace = true
'''


class EntryPointTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory(prefix='split-cli-fixture-')
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name)
        for name, text in MigrationPlanTests().sources().items():
            path = self.root / 'crates/html/src' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(text.encode())
        (self.root / 'crates/html/Cargo.toml').write_bytes(MANIFEST.encode())
        (self.root / 'scripts').mkdir()
        repository = Path(__file__).resolve().parents[1]
        for name in GUARDS:
            shutil.copyfile(repository / 'scripts' / name, self.root / 'scripts' / name)
        self.git('init', '-q')
        self.git('config', 'core.autocrlf', 'false')
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '-qm', 'fixture')

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.root), *args], text=True).strip()

    def run_cli(self, *args):
        with contextlib.redirect_stdout(io.StringIO()):
            return main(['--root', str(self.root), *args])

    def files(self):
        return {p.relative_to(self.root).as_posix(): p.read_bytes()
                for folder in ('crates', 'scripts')
                for p in (self.root / folder).rglob('*') if p.is_file()}

    def test_check_is_read_only_apply_and_repeat_are_idempotent(self):
        original = self.files()
        self.assertEqual(self.run_cli('--check'), 1)
        self.assertEqual(self.files(), original)
        self.assertEqual(self.run_cli(), 0)
        moved = self.files()
        self.assertTrue((self.root / 'crates/html-layout/src/layout/mod.rs').is_file())
        self.assertFalse((self.root / 'crates/html/src/layout/mod.rs').exists())
        self.assertEqual(self.run_cli('--check'), 0)
        self.assertEqual(self.run_cli(), 0)
        self.assertEqual(self.files(), moved)

    def test_cycle_blocks_before_all_source_and_manifest_writes(self):
        path = self.root / 'crates/html/src/dom/mod.rs'
        path.write_bytes(b'pub struct Node; fn backwards() { crate::layout::shape(Node); }\n')
        original = self.files()
        self.assertEqual(self.run_cli('--check'), 2)
        self.assertEqual(self.run_cli(), 2)
        self.assertEqual(self.files(), original)

    def test_check_rejects_generated_directory_collision_without_writes(self):
        collision = self.root / 'crates/html-layout/src/lib.rs'
        collision.mkdir(parents=True)
        original = self.files()
        error = io.StringIO()
        with contextlib.redirect_stderr(error), self.assertRaises(SystemExit) as caught:
            self.run_cli('--check')
        self.assertEqual(caught.exception.code, 2)
        self.assertIn('not a regular file', error.getvalue())
        self.assertEqual(self.files(), original)
        self.assertTrue(collision.is_dir())

    def test_guard_drift_rejects_before_source_or_manifest_writes(self):
        path = self.root / 'scripts/check_conventions.py'
        path.write_bytes(path.read_bytes().replace(b'from collections import defaultdict',
            b'from collections import defaultdict as alias'))
        original = self.files()
        error = io.StringIO()
        with contextlib.redirect_stderr(error), self.assertRaises(SystemExit) as caught:
            self.run_cli()
        self.assertEqual(caught.exception.code, 2)
        self.assertIn('Guard import source drift', error.getvalue())
        self.assertEqual(self.files(), original)

    def test_dirty_tree_is_not_applied(self):
        path = self.root / 'crates/html/src/dom/mod.rs'
        path.write_bytes(path.read_bytes() + b'// concurrent edit\n')
        original = self.files()
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            self.run_cli()
        self.assertEqual(self.files(), original)

    @unittest.skipUnless(shutil.which('cargo'), 'Cargo required for boundary acceptance')
    def test_test_contract_compiles_and_preserves_the_complete_test_inventory(self):
        from check_engine_test_support import check
        from test_migration_test_support import SupportTests
        for path in (self.root / 'crates/html/src').rglob('*.rs'):
            path.unlink()
        sources = SupportTests().sources()
        sources['lib.rs'] = 'pub mod style; pub mod layout; pub mod render; pub mod paint;\n'
        sources['style.rs'] = 'pub struct Value(pub u32);\n'
        sources['paint.rs'] = ''
        sources['layout/body.rs'] = (
            'pub fn body(make: fn() -> crate::style::Value) { assert_eq!(make().0, 42); }\n'
            '#[test] fn own_case() { assert_eq!(2 + 3, 5); }\n')
        sources['render.rs'] = sources['render.rs'].replace('crate::style::Value)', 'crate::style::Value(42))')
        # Keep a third independent local test; bodies and names must survive.
        for name, text in sources.items():
            path = self.root / 'crates/html/src' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(text.encode())
        (self.root / 'Cargo.toml').write_text(
            '[workspace]\nresolver = "2"\nmembers = ["crates/*"]\n'
            '[workspace.package]\nversion = "0.0.0"\nedition = "2021"\n'
            '[workspace.lints.rust]\nunsafe_code = "forbid"\n', encoding='utf-8')
        (self.root / '.cargo').mkdir()
        (self.root / '.cargo/config.toml').write_text('[build]\nrustc-wrapper = ""\n'
            '[target.x86_64-pc-windows-msvc]\nlinker = "rust-lld"\n', encoding='utf-8')
        (self.root / '.gitignore').write_text('/target/\n', encoding='utf-8')

        def cargo(*args):
            try:
                return subprocess.run(['cargo', '--offline', *args], cwd=self.root,
                    text=True, capture_output=True, check=True, timeout=90)
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
                self.fail(f'{args}: {error}\n{error.stdout}\n{error.stderr}')

        def inventory():
            result = cargo('test', '--workspace', '--lib', '--', '--list')
            return sorted(line for line in result.stdout.splitlines() if line.endswith(': test'))

        before = inventory()
        self.assertEqual(len(before), 3)
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '-qm', 'test contract fixture')
        self.assertEqual(self.run_cli(), 0)
        # Resolve only existing local package entries into the lockfile.
        cargo('generate-lockfile')
        self.assertEqual(check(self.root), [])
        cargo('check', '--workspace', '--lib', '--locked', '-j', '1')
        self.assertEqual(inventory(), before)
        result = cargo('test', '--workspace', '--lib', '--locked', '-j', '1')
        self.assertEqual(result.stdout.count('test result: ok.'), 5)
        self.assertEqual(self.run_cli('--check'), 0)


if __name__ == '__main__':
    unittest.main()
