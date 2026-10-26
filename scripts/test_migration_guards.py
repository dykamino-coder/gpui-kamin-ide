import ast
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

OUT=Path(__file__).resolve().parent
ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/'scripts'),str(OUT)]
from migration_guards import GUARDS, HELPERS, MARKER, transform, plan
from migration_write import apply

class GuardPlanTests(unittest.TestCase):
    def setUp(self):
        self.inputs={name:(ROOT/'scripts'/name).read_bytes().decode('utf-8') for name in GUARDS}

    def test_transform_preserves_limits_and_uses_phase_inventory(self):
        result=transform(self.inputs)
        self.assertEqual(set(result),set(GUARDS))
        for name,text in result.items():
            with self.subTest(name=name):
                ast.parse(text)
                self.assertIn('html_source_inventory',text)
                self.assertTrue(text.endswith(MARKER))
        self.assertIn('transferred_growth', result['check_computed.py'])
        self.assertIn('installation', result['check_transitional.py'])
        layer_ast = ast.parse(result['check_layer_imports.py'])
        stage = next(ast.literal_eval(node.value) for node in layer_ast.body
                     if isinstance(node, ast.Assign)
                     and any(isinstance(target, ast.Name) and target.id == 'STAGE'
                             for target in node.targets))
        self.assertEqual(stage['font_facts'], 0)

    def test_transform_is_exactly_idempotent(self):
        after=transform(self.inputs)
        self.assertEqual(transform(after),after)

    def test_partial_adaptation_is_rejected(self):
        self.inputs[GUARDS[0]]=transform(self.inputs)[GUARDS[0]]
        with self.assertRaisesRegex(ValueError,'Partially adapted'):
            transform(self.inputs)

    def test_conventions_drift_is_rejected(self):
        self.inputs['check_conventions.py']=self.inputs['check_conventions.py'].replace(
            'from collections import defaultdict','from collections import defaultdict as other')
        # Remove the entire anchor; a prefix extension still matches by design.
        self.inputs['check_conventions.py']=self.inputs['check_conventions.py'].replace(
            'from collections import defaultdict','from collections import Counter')
        with self.assertRaisesRegex(ValueError,'expected one occurrence'):
            transform(self.inputs)

    def test_guard_drift_is_rejected(self):
        self.inputs['check_layer_imports.py']=self.inputs['check_layer_imports.py'].replace(
            'from rust_layer_exports import Exports','from rust_layer_exports import Renamed')
        with self.assertRaisesRegex(ValueError,'expected one exact replacement'):
            transform(self.inputs)

    def test_import_alias_drift_is_rejected(self):
        self.inputs['check_conventions.py']=self.inputs['check_conventions.py'].replace(
            'from collections import defaultdict','from collections import defaultdict as alias')
        with self.assertRaisesRegex(ValueError,'import source drift'):
            transform(self.inputs)

    def test_extra_definition_is_not_silently_removed(self):
        self.inputs['check_conventions.py']=self.inputs['check_conventions.py'].replace(
            '\n\nRULES =','\n\ndef upstream_helper():\n    return 7\n\nRULES =')
        with self.assertRaisesRegex(ValueError,'structure changed'):
            transform(self.inputs)

    def test_all_helpers_share_the_same_preflight_and_repeat_is_empty(self):
        with tempfile.TemporaryDirectory(prefix='guards-plan-') as folder:
            root=Path(folder)
            (root/'scripts').mkdir()
            for name,text in self.inputs.items():
                (root/'scripts'/name).write_bytes(text.encode('utf-8'))
            changes=plan(root,OUT)
            self.assertEqual(len(changes),len(GUARDS)+len(HELPERS))
            apply(root,changes)
            self.assertEqual(plan(root,OUT),[])
            for name in HELPERS:
                self.assertEqual((root/'scripts'/name).read_bytes(),(OUT/name).read_bytes())

if __name__=='__main__':
    unittest.main(verbosity=2)
