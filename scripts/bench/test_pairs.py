"""Synthetic runner checks: no GPU, launcher, timers or verdict changes."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch


class FirstFailureTests(unittest.TestCase):
    def test_retry_keeps_first_stderr_pixels_and_failed_stdout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            calls = {}

            def run(args, stdout, stderr, label):
                attempt = calls.get(label, 0)
                calls[label] = attempt + 1
                stdout.write_text(json.dumps({'attempt': attempt}))
                stderr.write_text(f'failure {attempt}\n' if not attempt else 'retry succeeded\n')
                if '--frame-diff-dir' in args:
                    directory = Path(args[args.index('--frame-diff-dir') + 1])
                    directory.mkdir(parents=True)
                    (directory / 'first.png').write_bytes(b'synthetic RGB evidence')
                return (1 if not attempt else 0), b''

            fake = types.ModuleType('hb')
            fake.H, fake.PAGES, fake.run_hidden = root, {'fixture': root / 'fixture.html'}, run
            source = Path(os.environ.get('PAIRS_UNDER_TEST', Path(__file__).with_name('pairs.py')))
            spec = importlib.util.spec_from_file_location('pairs_under_test', source)
            module = importlib.util.module_from_spec(spec)
            with patch.dict(sys.modules, {'hb': fake}):
                spec.loader.exec_module(module)
            exe = root / 'synthetic.exe'
            exe.write_bytes(b'not executed')
            out = root / 'results'
            argv = [str(out), str(exe), str(exe), 'fixture', '1', 'both', '0', '0']
            module.main(argv)
            stem = 'fixture-01-base-both'
            self.assertEqual((out / f'{stem}.err').read_text(), 'failure 0\n')
            failures = json.loads((out / 'failures.json').read_text())
            first = Path(next(item for item in failures if item['stem'] == stem)['artifacts'])
            self.assertEqual(json.loads((first / 'stdout.json').read_text()), {'attempt': 0})
            self.assertEqual((first / 'frames/first.png').read_bytes(), b'synthetic RGB evidence')
            self.assertEqual((first.parent / '001/stderr.err').read_text(), 'retry succeeded\n')
            self.assertEqual(json.loads((out / f'{stem}.json').read_text()), {'attempt': 1})
            # A resumed missing sample must not reuse previous attempt paths or error text.
            (out / f'{stem}.json').unlink()
            module.main(argv)
            self.assertEqual((out / f'{stem}.err').read_text(), 'failure 0\n')
            self.assertTrue((first.parent / '002/stdout.json').is_file())


if __name__ == '__main__':
    unittest.main()
