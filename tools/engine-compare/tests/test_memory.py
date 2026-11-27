"""Memory: private bytes from tree samples and marks, empty-page subtraction, the best-other-engine table."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from ec import memory
from ec import metrics as M

MB = 1 << 20


class TreePrivate(unittest.TestCase):
    def test_peak_steady_and_after_phase(self):
        # samples (t, cpu, ws, private); marks (t, cpu, ws, private, cycles)
        samples = [(0.0, 0, 50 * MB, 40 * MB), (1.0, 0, 90 * MB, 80 * MB), (1.6, 0, 70 * MB, 60 * MB),
                   (2.0, 0, 72 * MB, 62 * MB), (3.0, 0, 75 * MB, 66 * MB)]
        marks = {'nav': (0.5, 0, 0, 0, 0), 'loaded': (1.0, 0, 0, 0, 0), 'steady_end': (2.1, 0, 0, 0, 0),
                 'cpu_C_0': (2.2, 0, 0, 0, 0), 'cpu_C_1': (3.0, 0, 75 * MB, 66 * MB, 0)}
        p = M.tree_private(samples, marks)
        self.assertEqual(p['peak_private_bytes'], 80 * MB)  # the sample before navigation is not the page
        self.assertEqual(p['steady_private_bytes'], 61 * MB)  # median of the samples in [loaded + 0.5, steady_end]
        self.assertEqual(p['after_phase'], {'C': {'ws_bytes': 75 * MB, 'private_bytes': 66 * MB}})

    def test_no_marks(self):
        p = M.tree_private([(0.0, 0, 1, 2)], {})
        self.assertEqual((p['peak_private_bytes'], p['steady_private_bytes'], p['after_phase']), (None, None, {}))


class OursPrivate(unittest.TestCase):
    def test_audit_fields(self):
        j = {'memory_audit': {'page': {'median': {'private_commit_bytes': 30}, 'end': {'process_lifetime_peak_commit_bytes': 90}}}}
        self.assertEqual(M.ours_private(j), {'peak_private_bytes': 90, 'steady_private_bytes': 30})

    def test_falls_back_to_report_end(self):
        j = {'memory_audit': {'error': 'x'}, 'end': {'process_lifetime_peak_commit_bytes': 7}}
        self.assertEqual(M.ours_private(j), {'peak_private_bytes': 7, 'steady_private_bytes': None})


class MinusBlank(unittest.TestCase):
    def test_keys_and_after_phase(self):
        m = {'peak_bytes': 100, 'steady_bytes': 80, 'peak_private_bytes': 60, 'steady_private_bytes': None,
             'after_phase': {'S': {'ws_bytes': 90, 'private_bytes': 70}}}
        b = {'peak_bytes': 30, 'steady_bytes': 20, 'peak_private_bytes': 10, 'steady_private_bytes': 5}
        self.assertEqual(M.minus_blank(m, b), {'peak_bytes': 70, 'steady_bytes': 60, 'peak_private_bytes': 50,
                                               'after_phase': {'S': {'ws_bytes': 70, 'private_bytes': 65}}})

    def test_no_blank(self):
        self.assertEqual(M.minus_blank({'peak_bytes': 1}, None), {})


class Table(unittest.TestCase):
    MEM = {('gpu4', 'p', 'ours'): {'peak_private_bytes': 300, 'blank': {'peak_private_bytes': 100}},
           ('gpu4', 'p', 'chrome'): {'peak_private_bytes': 900, 'blank': {'peak_private_bytes': 800}},
           ('gpu4', 'p', 'blitz'): {'peak_private_bytes': 250}}

    def test_rows_keep_raw_net_and_blank_pages(self):
        rs = memory.rows(self.MEM)
        ours = next(r for r in rs if r['engine'] == 'ours' and r['page'] == 'p')
        self.assertEqual((ours['raw'], ours['blank'], ours['net']), (300, 100, 200))
        blanks = sorted((r['engine'], r['raw']) for r in rs if r['page'] == memory.BLANK_PAGE)
        self.assertEqual(blanks, [('chrome', 800), ('ours', 100)])

    def test_best_uses_net_where_both_have_it(self):
        bs = {(r['page'], r['metric']): r for r in memory.best(memory.rows(self.MEM))}
        r = bs[('p', 'peak_private_bytes')]
        self.assertEqual((r['basis'], r['ours'], r['best'], r['best_engine'], r['ratio']), ('net', 200, 100, 'chrome', 2.0))
        b = bs[(memory.BLANK_PAGE, 'peak_private_bytes')]  # the constant price: raw against raw
        self.assertEqual((b['basis'], b['best_engine'], b['ratio']), ('raw', 'chrome', 0.125))

    def test_write(self):
        with tempfile.TemporaryDirectory() as d:
            n, nb = memory.write(Path(d) / 'r.memory.csv', self.MEM)
            self.assertTrue((Path(d) / 'r.memory.best.csv').exists())
            self.assertEqual((n, nb), (5, 2))


class NativeBlank(unittest.TestCase):
    def test_empty_page_measured_once_and_failures_not_cached(self):
        from types import SimpleNamespace
        from ec.adapters.native import Native
        with tempfile.TemporaryDirectory() as d:
            cfg = SimpleNamespace(store=Path(d) / 'store')
            lane = SimpleNamespace(work=Path(d) / 'work'); lane.work.mkdir()
            n = Native('kite', cfg, {'exe': 'x.exe', 'version': 'v1'}); calls = []

            def launch(lane, cls, page, phases, hold, input_file=None):
                calls.append(input_file)
                return ({}, 'crashed' if len(calls) == 1 else None, (5 * MB, 3 * MB), (4 * MB, 2 * MB))
            n.launch = launch
            self.assertEqual(n._blank(lane, 'soft2')['empty'], 'crashed')  # a failed empty page is retried
            b = n._blank(lane, 'soft2'); n._blank(lane, 'soft2')
            self.assertEqual(len(calls), 2)
            self.assertEqual(b, {'peak_bytes': 5 * MB, 'steady_bytes': 4 * MB,
                                 'peak_private_bytes': 3 * MB, 'steady_private_bytes': 2 * MB})
            self.assertTrue(str(calls[-1]).endswith('kite-blank.html'))

if __name__ == '__main__':
    unittest.main()
